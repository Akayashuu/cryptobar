use crate::config::Config;
use crate::model::Quote;
use crate::portfolio;
use crate::render::{badge, format, sparkline};

use super::markup::Markup;
use super::payload::{AlertRow, Module};
use super::rows::rows;
use super::summary::summary;
use super::tooltip::tooltip;

/// Builds the module from the active quote and the full list.
#[must_use]
pub fn render(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
    onchain: &std::collections::BTreeMap<crate::model::CoinId, f64>,
) -> Module {
    let Some(quote) = quotes.get(active).or_else(|| quotes.first()) else {
        return Module {
            text: "crypto —".to_owned(),
            tooltip: "No data available".to_owned(),
            class: "error".to_owned(),
            symbol: "—".to_owned(),
            price: String::new(),
            percent: String::new(),
            sparkline: String::new(),
            trend: "error",
            stale,
            icon: String::new(),
            glyph: "●".to_owned(),
            color: "#8d99ae".to_owned(),
            foreground: "#ffffff".to_owned(),
            coins: Vec::new(),
            alerts: Vec::new(),
            portfolio: None,
        };
    };

    let decimals = config.decimals();
    let triggers = crate::alerts::evaluate(config, quotes);
    let wallet = portfolio::compute(quotes, onchain);
    let badge = badge::of(&quote.symbol);
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
        tooltip: tooltip(
            quotes,
            active,
            config,
            stale,
            markup,
            wallet.as_ref(),
            &triggers,
        ),
        class: class(quote, stale, !triggers.is_empty()),
        symbol,
        price,
        percent,
        sparkline: spark,
        trend: quote.trend().class(),
        stale,
        icon: quote.icon.clone().unwrap_or_default(),
        glyph: badge.glyph,
        color: badge.color,
        foreground: badge.foreground,
        coins: rows(quotes, active, config, markup),
        alerts: triggers
            .iter()
            .map(|trigger| AlertRow {
                symbol: markup.escape(&trigger.symbol),
                kind: trigger.kind.slug(),
                title: markup.escape(&trigger.title),
                body: markup.escape(&trigger.body),
            })
            .collect(),
        portfolio: wallet.map(|wallet| summary(&wallet, config, markup)),
    }
}

fn class(quote: &Quote, stale: bool, alerting: bool) -> String {
    let mut class = quote.trend().class().to_owned();
    if stale {
        class.push_str(" stale");
    }
    if alerting {
        class.push_str(" alert");
    }
    class
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::render::waybar::fixtures::{config, no_onchain, quote};

    #[test]
    fn the_text_holds_symbol_price_and_change() {
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::Pango,
            &no_onchain(),
        );
        assert!(module.text.contains("BTC"));
        assert!(module.text.contains("42"));
        assert!(module.text.contains("1.5"), "text: {}", module.text);
    }

    #[test]
    fn the_class_follows_the_trend() {
        let cfg = config();
        assert_eq!(
            render(
                &[quote("btc", 1.0, Some(1.0))],
                0,
                &cfg,
                false,
                Markup::Pango,
                &no_onchain(),
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
                Markup::Pango,
                &no_onchain(),
            )
            .class,
            "down"
        );
    }

    #[test]
    fn the_stale_state_joins_the_class_and_the_tooltip() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            true,
            Markup::Pango,
            &no_onchain(),
        );
        assert!(module.class.contains("stale"));
        assert!(module.tooltip.contains("stale"));
    }

    #[test]
    fn an_empty_list_yields_an_error_module_not_a_panic() {
        let module = render(&[], 0, &config(), false, Markup::Pango, &no_onchain());
        assert_eq!(module.class, "error");
    }

    #[test]
    fn an_out_of_bounds_index_falls_back_to_the_first_quote() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            9,
            &config(),
            false,
            Markup::Pango,
            &no_onchain(),
        );
        assert!(module.text.contains("BTC"));
    }

    #[test]
    fn the_tooltip_marks_the_active_quote() {
        let quotes = vec![quote("btc", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let tooltip = render(&quotes, 1, &config(), false, Markup::Pango, &no_onchain()).tooltip;
        let second = tooltip.lines().nth(1).unwrap();
        assert!(second.starts_with('▸'));
    }

    #[test]
    fn the_output_is_json_on_a_single_line() {
        let line = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
            &no_onchain(),
        )
        .to_line()
        .unwrap();
        assert_eq!(line.lines().count(), 1);
        assert!(line.starts_with('{'));
    }
}
