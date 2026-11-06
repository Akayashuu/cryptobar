//! `portfolio` subcommand: valuation in the terminal.

use std::fmt::Write as _;
use std::io::Write as _;

use anyhow::Result;

use crate::app::App;
use crate::config::Config;
use crate::portfolio::{self, Portfolio};
use crate::render::format;

/// Prints the portfolio valued at the last known prices.
///
/// # Errors
/// Fails when writing to standard output fails. A network outage is reported in
/// the output, not through an error: the cache stays useful.
pub fn run(app: &App) -> Result<()> {
    let (mut quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        Err(_) => (Vec::new(), true),
    };
    app.attach_icons(&mut quotes);
    // The balances of the watched addresses are refreshed here: the command is
    // one-shot, the user expects an up-to-date figure.
    let onchain = crate::onchain::totals(&app.watcher().refresh(&app.config.wallets));
    let wallet = portfolio::compute(&quotes, &onchain);
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{}", render(wallet.as_ref(), &app.config, stale))?;
    stdout.flush()?;
    Ok(())
}

/// Formats the portfolio. Pure function: testable without network.
#[must_use]
pub fn render(wallet: Option<&Portfolio>, config: &Config, stale: bool) -> String {
    let Some(wallet) = wallet else {
        return "No watched address. Add a PUBLIC ADDRESS to the configuration \
                — never a private key, never a recovery phrase:\n\n    \
                [[wallets]]\n    chain = \"bitcoin\"\n    address = \"bc1q…\"\n"
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

    let share_column: Vec<String> = wallet
        .positions
        .iter()
        .map(|position| format!("{:.1} %", position.share * 100.0))
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
        // Writing into a `String` is infallible.
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
        let _ = writeln!(out, "\nstale data — network unavailable");
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
    fn without_an_address_the_message_says_what_to_do() {
        let config = Config::parse("coins = [\"bitcoin\"]").unwrap();
        let out = render(None, &config, false);
        assert!(out.contains("[[wallets]]"), "unclear message: {out}");
    }

    #[test]
    fn the_table_shows_rows_total_and_change() {
        let config = Config::parse("coins = [\"bitcoin\"]").unwrap();
        let quotes = [quote("bitcoin", "BTC", 110.0, Some(10.0))];
        let wallet = portfolio::compute(&quotes, &held(2.0)).unwrap();
        let out = render(Some(&wallet), &config, false);
        assert!(out.contains("BTC"));
        assert!(out.contains("TOTAL"));
        assert!(out.contains("24 h"));
        assert!(out.contains("100.0 %"), "missing share: {out}");
    }

    #[test]
    fn the_stale_cache_is_reported() {
        let config = Config::parse("coins = [\"bitcoin\"]").unwrap();
        let quotes = [quote("bitcoin", "BTC", 1.0, None)];
        let wallet = portfolio::compute(&quotes, &held(1.0)).unwrap();
        assert!(render(Some(&wallet), &config, true).contains("stale"));
    }
}
