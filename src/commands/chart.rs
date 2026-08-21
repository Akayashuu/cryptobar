//! Sous-commande `chart` : graphique braille dans le terminal.

use std::fmt::Write as _;
use std::io::Write as _;

use anyhow::{Context as _, Result, bail};

use crate::app::App;
use crate::model::{CoinId, Range, Series};
use crate::render::{braille, format};

/// Marges réservées autour du tracé : axe des ordonnées et bordures.
const AXIS_WIDTH: usize = 12;
const MIN_COLS: usize = 20;
const MIN_ROWS: usize = 4;
/// Au-delà, le graphique devient illisible plutôt que plus précis.
const MAX_ROWS: usize = 24;
const DEFAULT_COLS: usize = 80;
const DEFAULT_ROWS: usize = 24;

/// Affiche le graphique de `coin` sur la plage demandée.
///
/// # Errors
/// Échoue si l'identifiant est invalide ou si la série est indisponible et
/// absente du cache.
pub fn run(app: &App, coin: Option<&str>, range: Range) -> Result<()> {
    let id = match coin {
        Some(raw) => CoinId::parse(raw).context("identifiant de cryptomonnaie invalide")?,
        None => app
            .config
            .coins
            .get(app.active_index())
            .or_else(|| app.config.coins.first())
            .cloned()
            .context("aucune cryptomonnaie configurée")?,
    };

    let cached = app
        .source
        .series_cached(&id, range, &app.config.currency)
        .with_context(|| format!("récupération de la série de {}", id.as_str()))?;

    let (cols, rows) = viewport();
    let rendered = render(&cached.value, &app.config, cols, rows, cached.stale);
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{rendered}")?;
    stdout.flush()?;
    Ok(())
}

/// Dimensions utilisables, avec un repli quand la sortie n'est pas un terminal.
fn viewport() -> (usize, usize) {
    terminal_size::terminal_size().map_or((DEFAULT_COLS, DEFAULT_ROWS), |(w, h)| {
        (usize::from(w.0), usize::from(h.0))
    })
}

/// Compose l'en-tête, le tracé et l'axe. Fonction pure : entièrement testable.
#[must_use]
pub fn render(
    series: &Series,
    config: &crate::config::Config,
    cols: usize,
    rows: usize,
    stale: bool,
) -> String {
    let plot_cols = cols.saturating_sub(AXIS_WIDTH).max(MIN_COLS);
    // Deux lignes d'en-tête et une de pied de page sont retirées de la hauteur,
    // puis la hauteur en cellules est le quart de la hauteur en points.
    let plot_rows = rows.saturating_sub(4).clamp(MIN_ROWS, MAX_ROWS);

    let decimals = config.decimals();
    let (min, max) = braille::bounds(&series.points);
    let lines = braille::plot(&series.points, plot_cols, plot_rows);

    let last = series.points.last().copied().unwrap_or(0.0);
    let first = series.points.first().copied().unwrap_or(last);
    let change = if first.abs() > f64::EPSILON {
        Some((last - first) / first * 100.0)
    } else {
        None
    };

    let mut out = String::new();
    // L'écriture dans une `String` est infaillible ; l'erreur est ignorée
    // sciemment plutôt que remontée par toute la chaîne d'appel.
    let _ = writeln!(
        out,
        "{}  {}   {}  ({})",
        series.id.as_str().to_uppercase(),
        format::price(last, &config.currency, decimals),
        format::percent(change),
        series.range.label(),
    );
    if stale {
        out.push_str("données périmées — réseau indisponible\n");
    }
    out.push('\n');

    let label_max = format::price(max, &config.currency, decimals);
    let label_min = format::price(min, &config.currency, decimals);
    let width = label_max.chars().count().max(label_min.chars().count());
    let last_row = lines.len().saturating_sub(1);
    for (index, line) in lines.iter().enumerate() {
        let label = if index == 0 {
            label_max.clone()
        } else if index == last_row {
            label_min.clone()
        } else {
            String::new()
        };
        let _ = writeln!(out, "{label:>width$} │{line}");
    }
    out
}

/// Ouvre le graphique dans une nouvelle fenêtre de terminal.
///
/// # Errors
/// Échoue si l'émulateur configuré est introuvable.
pub fn spawn_terminal(app: &App, coin: Option<&str>, range: Range) -> Result<()> {
    let terminal = app.config.terminal.trim();
    if terminal.is_empty() {
        bail!("aucun émulateur de terminal configuré");
    }
    let exe = std::env::current_exe().context("chemin de l'exécutable introuvable")?;
    let mut command = std::process::Command::new(terminal);
    // `-e` est accepté par foot, kitty, alacritty et wezterm.
    command
        .arg("-e")
        .arg(exe)
        .arg("chart")
        .arg("--range")
        .arg(range.slug());
    if let Some(coin) = coin {
        command.arg(coin);
    }
    command
        .spawn()
        .with_context(|| format!("lancement de {terminal}"))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn config() -> Config {
        Config::parse("coins = [\"bitcoin\"]").unwrap()
    }

    fn series(points: Vec<f64>) -> Series {
        Series {
            id: CoinId::parse("bitcoin").unwrap(),
            range: Range::D7,
            points,
        }
    }

    #[test]
    fn len_tete_contient_le_symbole_le_prix_et_la_plage() {
        let out = render(&series(vec![100.0, 110.0]), &config(), 80, 20, false);
        let header = out.lines().next().unwrap();
        assert!(header.contains("BITCOIN"));
        assert!(
            header.contains("7 jours") || header.contains('7'),
            "en-tête : {header}"
        );
    }

    #[test]
    fn la_variation_est_calculee_entre_le_premier_et_le_dernier_point() {
        let out = render(&series(vec![100.0, 110.0]), &config(), 80, 20, false);
        assert!(out.lines().next().unwrap().contains("10,0"));
    }

    #[test]
    fn letat_perime_est_signale() {
        let out = render(&series(vec![1.0, 2.0]), &config(), 80, 20, true);
        assert!(out.contains("périmées"));
    }

    #[test]
    fn laxe_porte_le_maximum_en_haut_et_le_minimum_en_bas() {
        let out = render(&series(vec![10.0, 50.0]), &config(), 80, 20, false);
        let plot: Vec<&str> = out.lines().filter(|l| l.contains('│')).collect();
        assert!(plot.first().unwrap().contains("50"));
        assert!(plot.last().unwrap().contains("10"));
    }

    #[test]
    fn un_terminal_minuscule_ne_produit_pas_de_dimension_nulle() {
        let out = render(&series(vec![1.0, 2.0, 3.0]), &config(), 1, 1, false);
        assert!(out.contains('│'));
    }

    #[test]
    fn une_serie_vide_ne_panique_pas() {
        let out = render(&series(Vec::new()), &config(), 80, 20, false);
        assert!(!out.is_empty());
    }

    #[test]
    fn une_serie_partant_de_zero_nannonce_pas_de_variation() {
        let out = render(&series(vec![0.0, 5.0]), &config(), 80, 20, false);
        assert!(out.lines().next().unwrap().contains('—'));
    }
}
