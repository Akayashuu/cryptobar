use crate::config::Config;
use crate::model::Quote;
use crate::render::format;

use super::markup::Markup;
use super::payload::CoinRow;
use super::series::normalize;
use crate::render::badge;

pub(super) fn rows(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    markup: Markup,
) -> Vec<CoinRow> {
    let decimals = config.decimals();
    quotes
        .iter()
        .enumerate()
        .map(|(index, quote)| {
            let badge = badge::of(&quote.symbol);
            CoinRow {
                symbol: markup.escape(&quote.symbol.to_uppercase()),
                name: markup.escape(&quote.name),
                price: markup.escape(&format::price(quote.price, &config.currency, decimals)),
                change_1h: markup.escape(&format::percent(quote.change_1h)),
                change_24h: markup.escape(&format::percent(quote.change_24h)),
                change_7d: markup.escape(&format::percent(quote.change_7d)),
                trend: quote.trend().class(),
                series: normalize(&quote.sparkline),
                active: index == active,
                icon: quote.icon.clone().unwrap_or_default(),
                glyph: badge.glyph,
                color: badge.color,
                foreground: badge.foreground,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::waybar::fixtures::{config, no_onchain, quote};
    use crate::render::waybar::render;

    #[test]
    fn the_structured_fields_come_with_the_text() {
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::None,
            &no_onchain(),
        );
        assert_eq!(module.symbol, "BTC");
        assert!(module.price.contains("42"));
        assert!(module.percent.contains("1.5"));
        assert_ne!(module.sparkline.len(), 0);
        assert_eq!(
            module.text,
            format!(
                "{} {} {} {}",
                module.symbol, module.price, module.sparkline, module.percent
            )
        );
    }

    #[test]
    fn every_quote_yields_a_structured_row() {
        let quotes = vec![
            quote("btc", 42_000.0, Some(1.5)),
            quote("eth", 2_000.0, Some(-1.0)),
        ];
        let module = render(&quotes, 1, &config(), false, Markup::None, &no_onchain());
        let [first, second] = module.coins.as_slice() else {
            panic!("expected two quotes, got {}", module.coins.len());
        };
        assert_eq!(first.symbol, "BTC");
        assert!(!first.active);
        assert!(second.active);
        assert_eq!(second.trend, "down");
        assert_eq!(module.trend, "down");
    }
}
