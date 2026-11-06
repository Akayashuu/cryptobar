use crate::config::Config;
use crate::model::Quote;
use crate::portfolio;
use crate::render::format;

use super::markup::Markup;

pub(super) fn tooltip(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
    wallet: Option<&portfolio::Portfolio>,
    triggers: &[crate::alerts::Trigger],
) -> String {
    let decimals = config.decimals();
    let mut lines = Vec::with_capacity(quotes.len().saturating_add(2));
    for (index, quote) in quotes.iter().enumerate() {
        let marker = if index == active { "▸" } else { " " };
        lines.push(format!(
            "{marker} {}  {}   1h {}  24h {}  7d {}",
            markup.bold(&markup.escape(&quote.symbol.to_uppercase())),
            markup.escape(&format::price(quote.price, &config.currency, decimals)),
            markup.escape(&format::percent(quote.change_1h)),
            markup.escape(&format::percent(quote.change_24h)),
            markup.escape(&format::percent(quote.change_7d)),
        ));
    }
    if let Some(wallet) = wallet {
        lines.push(String::new());
        let total = markup.escape(&format::price(wallet.total, &config.currency, decimals));
        let gain = markup.escape(&format::percent(wallet.change_24h_pct));
        lines.push(format!(
            "{}  {total}   24h {gain}",
            markup.bold("Portfolio")
        ));
    }
    for trigger in triggers {
        lines.push(String::new());
        lines.push(format!("⚠ {}", markup.escape(&trigger.title)));
    }
    if stale {
        lines.push(String::new());
        lines.push(markup.italic("stale data: network unavailable"));
    }
    lines.join("\n")
}
