//! Sérialisation du protocole des modules personnalisés Waybar.
//!
//! Waybar attend une ligne de JSON contenant `text`, `tooltip` et `class`.
//! `text` et `tooltip` sont interprétés comme du balisage Pango : toute donnée
//! venant du réseau doit donc être échappée.
//!
//! D'autres barres — Quickshell, Ags, Eww — consomment le même JSON mais
//! affichent du texte brut. [`Markup::None`] produit alors une sortie sans
//! aucune balise ni entité.

use serde::Serialize;

use crate::config::Config;
use crate::model::Quote;

use super::{format, sparkline};

/// Nombre de points conservés dans les séries transmises aux barres riches :
/// assez pour dessiner une courbe lisible, assez peu pour que la ligne JSON
/// reste courte à chaque intervalle.
const SERIES_POINTS: usize = 48;

/// Une cotation, découpée en champs prêts à afficher.
///
/// Les barres capables de composer leur propre mise en page — Quickshell,
/// Ags, Eww — s'en servent pour dessiner une liste alignée et une courbe,
/// là où `tooltip` ne leur offrirait qu'un bloc de texte à parser.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CoinRow {
    pub symbol: String,
    pub name: String,
    pub price: String,
    pub change_1h: String,
    pub change_24h: String,
    pub change_7d: String,
    /// "up", "down" ou "flat", suivant la variation sur 24 h.
    pub trend: &'static str,
    /// Cours des sept derniers jours, normalisé dans [0, 1] : le
    /// consommateur multiplie par la hauteur dont il dispose.
    pub series: Vec<f64>,
    /// Cotation actuellement mise en avant dans la barre.
    pub active: bool,
}

/// Charge utile d'un module Waybar.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Module {
    pub text: String,
    pub tooltip: String,
    pub class: String,
    /// Champs additionnels ignorés par Waybar, utiles aux barres capables
    /// de composer leur propre mise en page (Quickshell en vertical, où le
    /// texte complet ne tiendrait pas dans la largeur disponible).
    pub symbol: String,
    pub price: String,
    pub percent: String,
    pub sparkline: String,
    /// Tendance de la cotation active : "up", "down", "flat" ou "error".
    pub trend: &'static str,
    /// Données servies depuis le cache, réseau indisponible.
    pub stale: bool,
    /// Toutes les cotations suivies, dans l'ordre de la configuration.
    pub coins: Vec<CoinRow>,
}

/// Ramène une série dans [0, 1]. Une série plate se place au milieu plutôt
/// que de diviser par une amplitude nulle.
fn normalize(points: &[f64]) -> Vec<f64> {
    let finite: Vec<f64> = points.iter().copied().filter(|v| v.is_finite()).collect();
    let reduced = sparkline::downsample(&finite, SERIES_POINTS);
    let Some(min) = reduced.iter().copied().reduce(f64::min) else {
        return Vec::new();
    };
    let max = reduced.iter().copied().fold(min, f64::max);
    let span = max - min;
    if span <= 0.0 {
        return vec![0.5; reduced.len()];
    }
    // Trois décimales suffisent à un tracé au pixel près et divisent par
    // cinq la longueur de la ligne JSON émise à chaque intervalle.
    reduced
        .iter()
        .map(|value| ((value - min) / span * 1000.0).round() / 1000.0)
        .collect()
}

impl Module {
    /// Sérialise en une ligne de JSON.
    ///
    /// # Errors
    /// Impossible en pratique : la structure ne contient que des chaînes.
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Traitement du balisage dans la sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Markup {
    /// Balises Pango et entités échappées : ce qu'attend Waybar.
    #[default]
    Pango,
    /// Texte brut, pour les barres qui n'interprètent aucun balisage.
    None,
}

impl Markup {
    /// Applique l'échappement correspondant à une donnée non fiable.
    fn escape(self, raw: &str) -> String {
        match self {
            Self::Pango => escape_pango(raw),
            // Les caractères de contrôle sont retirés dans tous les cas :
            // ils casseraient la ligne JSON attendue.
            Self::None => raw
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect(),
        }
    }

    /// Ouvre et ferme une mise en gras, ou rien du tout.
    fn bold(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<b>{raw}</b>"),
            Self::None => raw.to_owned(),
        }
    }

    /// Met en forme une note discrète.
    fn italic(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<i>{raw}</i>"),
            Self::None => raw.to_owned(),
        }
    }
}

impl std::str::FromStr for Markup {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pango" => Ok(Self::Pango),
            "none" | "plain" => Ok(Self::None),
            other => Err(format!(
                "balisage inconnu : {other} ; attendu `pango` ou `none`"
            )),
        }
    }
}

/// Échappe les caractères que Pango interpréterait comme du balisage.
#[must_use]
pub fn escape_pango(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            // Les caractères de contrôle casseraient la ligne JSON attendue.
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Construit le module à partir de la cotation active et de la liste complète.
#[must_use]
pub fn render(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
) -> Module {
    let Some(quote) = quotes.get(active).or_else(|| quotes.first()) else {
        return Module {
            text: "crypto —".to_owned(),
            tooltip: "Aucune donnée disponible".to_owned(),
            class: "error".to_owned(),
            symbol: "—".to_owned(),
            price: String::new(),
            percent: String::new(),
            sparkline: String::new(),
            trend: "error",
            stale,
            coins: Vec::new(),
        };
    };

    let decimals = config.decimals();
    let symbol = markup.escape(&quote.symbol.to_uppercase());
    let price = markup.escape(&format::price(quote.price, &config.currency, decimals));
    let percent = markup.escape(&format::percent(quote.change_24h));
    let spark = if config.sparkline && !quote.sparkline.is_empty() {
        sparkline::sparkline(&quote.sparkline, config.sparkline_width)
    } else {
        String::new()
    };

    let mut text = format!("{symbol} {price}");
    if !spark.is_empty() {
        text.push(' ');
        text.push_str(&spark);
    }
    text.push(' ');
    text.push_str(&percent);

    Module {
        text,
        tooltip: tooltip(quotes, active, config, stale, markup),
        class: class(quote, stale),
        symbol,
        price,
        percent,
        sparkline: spark,
        trend: quote.trend().class(),
        stale,
        coins: rows(quotes, active, config, markup),
    }
}

fn rows(quotes: &[Quote], active: usize, config: &Config, markup: Markup) -> Vec<CoinRow> {
    let decimals = config.decimals();
    quotes
        .iter()
        .enumerate()
        .map(|(index, quote)| CoinRow {
            symbol: markup.escape(&quote.symbol.to_uppercase()),
            name: markup.escape(&quote.name),
            price: markup.escape(&format::price(quote.price, &config.currency, decimals)),
            change_1h: markup.escape(&format::percent(quote.change_1h)),
            change_24h: markup.escape(&format::percent(quote.change_24h)),
            change_7d: markup.escape(&format::percent(quote.change_7d)),
            trend: quote.trend().class(),
            series: normalize(&quote.sparkline),
            active: index == active,
        })
        .collect()
}

fn class(quote: &Quote, stale: bool) -> String {
    let trend = quote.trend().class();
    if stale {
        format!("{trend} stale")
    } else {
        trend.to_owned()
    }
}

fn tooltip(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
) -> String {
    let decimals = config.decimals();
    let mut lines = Vec::with_capacity(quotes.len().saturating_add(2));
    for (index, quote) in quotes.iter().enumerate() {
        let marker = if index == active { "▸" } else { " " };
        lines.push(format!(
            "{marker} {}  {}   1h {}  24h {}  7j {}",
            markup.bold(&markup.escape(&quote.symbol.to_uppercase())),
            markup.escape(&format::price(quote.price, &config.currency, decimals)),
            markup.escape(&format::percent(quote.change_1h)),
            markup.escape(&format::percent(quote.change_24h)),
            markup.escape(&format::percent(quote.change_7d)),
        ));
    }
    if stale {
        lines.push(String::new());
        lines.push(markup.italic("données périmées — réseau indisponible"));
    }
    lines.join("\n")
}

#[cfg(test)]
// Un test qui échoue doit s'arrêter net ; la normalisation pose exactement
// 0.0 et 1.0 sur les extrêmes, la comparaison stricte y est donc exacte.
#[allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::model::CoinId;

    fn config() -> Config {
        Config::parse("coins = [\"bitcoin\", \"ethereum\"]").unwrap()
    }

    fn quote(symbol: &str, price: f64, change: Option<f64>) -> Quote {
        Quote {
            id: CoinId::parse("bitcoin").unwrap(),
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            price,
            change_1h: Some(0.5),
            change_24h: change,
            change_7d: Some(-2.0),
            sparkline: vec![1.0, 2.0, 3.0, 2.0],
        }
    }

    #[test]
    fn le_texte_contient_symbole_prix_et_variation() {
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::Pango,
        );
        assert!(module.text.contains("BTC"));
        assert!(module.text.contains("42"));
        assert!(module.text.contains("1,5"), "texte : {}", module.text);
    }

    #[test]
    fn la_classe_suit_la_tendance() {
        let cfg = config();
        assert_eq!(
            render(
                &[quote("btc", 1.0, Some(1.0))],
                0,
                &cfg,
                false,
                Markup::Pango
            )
            .class,
            "up"
        );
        assert_eq!(
            render(
                &[quote("btc", 1.0, Some(-1.0))],
                0,
                &cfg,
                false,
                Markup::Pango
            )
            .class,
            "down"
        );
    }

    #[test]
    fn letat_perime_sajoute_a_la_classe_et_au_tooltip() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            true,
            Markup::Pango,
        );
        assert!(module.class.contains("stale"));
        assert!(module.tooltip.contains("périmées"));
    }

    #[test]
    fn une_liste_vide_donne_un_module_derreur_et_non_un_panic() {
        let module = render(&[], 0, &config(), false, Markup::Pango);
        assert_eq!(module.class, "error");
    }

    #[test]
    fn un_indice_hors_bornes_retombe_sur_la_premiere_cotation() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            9,
            &config(),
            false,
            Markup::Pango,
        );
        assert!(module.text.contains("BTC"));
    }

    #[test]
    fn le_balisage_pango_est_echappe() {
        let module = render(
            &[quote("<b>&x", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
        );
        assert!(!module.text.contains("<b>"));
        assert!(module.text.contains("&lt;"));
        assert!(module.text.contains("&amp;"));
    }

    #[test]
    fn le_tooltip_marque_la_cotation_active() {
        let quotes = vec![quote("btc", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let tooltip = render(&quotes, 1, &config(), false, Markup::Pango).tooltip;
        let second = tooltip.lines().nth(1).unwrap();
        assert!(second.starts_with('▸'));
    }

    #[test]
    fn la_sortie_est_du_json_sur_une_seule_ligne() {
        let line = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
        )
        .to_line()
        .unwrap();
        assert_eq!(line.lines().count(), 1);
        assert!(line.starts_with('{'));
    }

    #[test]
    fn le_mode_sans_balisage_ne_produit_ni_balise_ni_entite() {
        let quotes = vec![quote("<b>&x", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let module = render(&quotes, 0, &config(), true, Markup::None);
        for field in [&module.text, &module.tooltip] {
            // Aucune balise n'est ajoutée par le rendu : ni <b> ni <i>.
            assert!(!field.contains("<b>"), "balise résiduelle : {field}");
            assert!(!field.contains("<i>"), "balise résiduelle : {field}");
            assert!(!field.contains("&amp;"), "entité résiduelle : {field}");
        }
        // La donnée réseau ressort littérale : c'est au consommateur de
        // l'afficher en texte brut (Quickshell : `textFormat: Text.PlainText`).
        assert!(module.text.contains("<B>&X"), "texte : {}", module.text);
    }

    #[test]
    fn le_mode_sans_balisage_neutralise_quand_meme_les_controles() {
        let module = render(
            &[quote("a\nb", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::None,
        );
        assert!(!module.text.contains('\n'));
    }

    #[test]
    fn le_balisage_est_parse_depuis_une_chaine() {
        assert_eq!("pango".parse(), Ok(Markup::Pango));
        assert_eq!("none".parse(), Ok(Markup::None));
        assert!("gras".parse::<Markup>().is_err());
    }

    #[test]
    fn les_champs_structures_accompagnent_le_texte() {
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::None,
        );
        assert_eq!(module.symbol, "BTC");
        assert!(module.price.contains("42"));
        assert!(module.percent.contains("1,5"));
        assert!(!module.sparkline.is_empty());
        // Le texte reste la concaténation des champs : aucune divergence
        // possible entre les deux représentations.
        assert_eq!(
            module.text,
            format!(
                "{} {} {} {}",
                module.symbol, module.price, module.sparkline, module.percent
            )
        );
    }

    #[test]
    fn chaque_cotation_donne_une_ligne_structuree() {
        let quotes = vec![
            quote("btc", 42_000.0, Some(1.5)),
            quote("eth", 2_000.0, Some(-1.0)),
        ];
        let module = render(&quotes, 1, &config(), false, Markup::None);
        let [first, second] = module.coins.as_slice() else {
            panic!(
                "deux cotations attendues, {} obtenue(s)",
                module.coins.len()
            );
        };
        assert_eq!(first.symbol, "BTC");
        assert!(!first.active);
        assert!(second.active);
        assert_eq!(second.trend, "down");
        assert_eq!(module.trend, "down");
    }

    #[test]
    fn les_series_transmises_tiennent_dans_zero_un() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::None,
        );
        let Some(row) = module.coins.first() else {
            panic!("une cotation attendue");
        };
        let series = &row.series;
        assert!(!series.is_empty());
        assert!(series.iter().all(|value| (0.0..=1.0).contains(value)));
        // Les extrêmes touchent les bornes : la courbe occupe toute la hauteur.
        assert!(series.iter().copied().any(|value| value == 0.0));
        assert!(series.iter().copied().any(|value| value == 1.0));
    }

    #[test]
    fn une_serie_plate_se_place_au_milieu() {
        assert_eq!(normalize(&[3.0, 3.0, 3.0]), vec![0.5, 0.5, 0.5]);
        assert!(normalize(&[]).is_empty());
        assert!(normalize(&[f64::NAN]).is_empty());
    }

    #[test]
    fn une_serie_longue_est_reduite() {
        let long: Vec<f64> = (0..500).map(f64::from).collect();
        assert_eq!(normalize(&long).len(), SERIES_POINTS);
    }

    #[test]
    fn les_caracteres_de_controle_sont_neutralises() {
        assert_eq!(escape_pango("a\nb"), "a b");
    }
}
