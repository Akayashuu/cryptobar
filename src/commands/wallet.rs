//! Sous-commande `portfolio` : valorisation dans le terminal.

use std::fmt::Write as _;
use std::io::Write as _;

use anyhow::Result;

use crate::app::App;
use crate::config::Config;
use crate::portfolio::{self, Portfolio};
use crate::render::format;

/// Affiche le portefeuille valorisé aux derniers cours connus.
///
/// # Errors
/// Échoue si l'écriture sur la sortie standard échoue. Une panne réseau est
/// signalée dans la sortie, pas par une erreur : le cache reste utile.
pub fn run(app: &App) -> Result<()> {
    let (mut quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        Err(_) => (Vec::new(), true),
    };
    app.attach_icons(&mut quotes);
    // Les soldes des adresses suivies sont rafraîchis ici : la commande est
    // ponctuelle, l'utilisateur attend un chiffre à jour.
    let onchain = crate::onchain::totals(&app.watcher().refresh(&app.config.wallets));
    let wallet = portfolio::compute(&quotes, &onchain);
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{}", render(wallet.as_ref(), &app.config, stale))?;
    stdout.flush()?;
    Ok(())
}

/// Met en forme le portefeuille. Fonction pure : testable sans réseau.
#[must_use]
pub fn render(wallet: Option<&Portfolio>, config: &Config, stale: bool) -> String {
    let Some(wallet) = wallet else {
        return "Aucune adresse suivie. Ajoutez une ADRESSE PUBLIQUE à la \
                configuration — jamais une clé privée ni une phrase de \
                récupération :\n\n    [[wallets]]\n    chain = \"bitcoin\"\n    \
                address = \"bc1q…\"\n"
            .to_owned();
    };
    let decimals = config.decimals();
    let money = |value: f64| format::price(value, &config.currency, decimals);

    let symbol_width = wallet
        .positions
        .iter()
        .map(|position| position.symbol.chars().count())
        .max()
        .unwrap_or(0);
    let quantity_column: Vec<String> = wallet
        .positions
        .iter()
        .map(|position| format::quantity(position.quantity))
        .collect();
    let quantity_width = quantity_column
        .iter()
        .map(|q| q.chars().count())
        .max()
        .unwrap_or(0);
    let value_column: Vec<String> = wallet
        .positions
        .iter()
        .map(|position| money(position.value))
        .collect();
    let value_width = value_column
        .iter()
        .map(|v| v.chars().count())
        .max()
        .unwrap_or(0);

    // Virgule décimale partout : le reste de la sortie est en français.
    let share_column: Vec<String> = wallet
        .positions
        .iter()
        .map(|position| format!("{:.1} %", position.share * 100.0).replace('.', ","))
        .collect();
    let share_width = share_column
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for (((position, quantity), value), share) in wallet
        .positions
        .iter()
        .zip(quantity_column.iter())
        .zip(value_column.iter())
        .zip(share_column.iter())
    {
        // L'écriture dans une `String` est infaillible.
        let _ = writeln!(
            out,
            "{:<symbol_width$}  {:>quantity_width$}  {:>value_width$}  {:>share_width$}",
            position.symbol.to_uppercase(),
            quantity,
            value,
            share,
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{:<symbol_width$}  {:>width$}",
        "TOTAL",
        money(wallet.total),
        width = quantity_width.saturating_add(value_width).saturating_add(2),
    );
    if let Some(gain) = wallet.change_24h {
        let sign = if gain >= 0.0 { "+" } else { "" };
        let _ = writeln!(
            out,
            "24 h  {sign}{}  ({})",
            money(gain),
            format::percent(wallet.change_24h_pct)
        );
    }
    if stale {
        let _ = writeln!(out, "\ndonnées périmées — réseau indisponible");
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::{CoinId, Quote};

    fn quote(id: &str, symbol: &str, price: f64, change: Option<f64>) -> Quote {
        Quote {
            id: CoinId::parse(id).unwrap(),
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            price,
            change_1h: None,
            change_24h: change,
            change_7d: None,
            sparkline: Vec::new(),
            image: None,
            market_cap: None,
            volume_24h: None,
            rank: None,
            ath: None,
            ath_date: None,
            icon: None,
        }
    }

    fn held(quantity: f64) -> std::collections::BTreeMap<CoinId, f64> {
        std::collections::BTreeMap::from([(CoinId::parse("bitcoin").unwrap(), quantity)])
    }

    #[test]
    fn sans_adresse_le_message_explique_quoi_faire() {
        let config = Config::parse("coins = [\"bitcoin\"]").unwrap();
        let out = render(None, &config, false);
        assert!(out.contains("[[wallets]]"), "message peu clair : {out}");
    }

    #[test]
    fn le_tableau_montre_lignes_total_et_variation() {
        let config = Config::parse("coins = [\"bitcoin\"]").unwrap();
        let quotes = [quote("bitcoin", "BTC", 110.0, Some(10.0))];
        let wallet = portfolio::compute(&quotes, &held(2.0)).unwrap();
        let out = render(Some(&wallet), &config, false);
        assert!(out.contains("BTC"));
        assert!(out.contains("TOTAL"));
        assert!(out.contains("24 h"));
        assert!(out.contains("100,0 %"), "part manquante : {out}");
    }

    #[test]
    fn le_cache_perime_est_signale() {
        let config = Config::parse("coins = [\"bitcoin\"]").unwrap();
        let quotes = [quote("bitcoin", "BTC", 1.0, None)];
        let wallet = portfolio::compute(&quotes, &held(1.0)).unwrap();
        assert!(render(Some(&wallet), &config, true).contains("périmées"));
    }
}
