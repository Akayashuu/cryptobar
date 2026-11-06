use crate::config::Config;
use crate::portfolio;
use crate::render::format;

use super::markup::Markup;
use super::payload::{HoldingRow, PortfolioSummary};

pub(super) fn summary(
    wallet: &portfolio::Portfolio,
    config: &Config,
    markup: Markup,
) -> PortfolioSummary {
    let decimals = config.decimals();
    let money = |value: f64| markup.escape(&format::price(value, &config.currency, decimals));
    PortfolioSummary {
        total: money(wallet.total),
        change_24h: match wallet.change_24h {
            Some(gain) if gain >= 0.0 => format!("+{}", money(gain)),
            Some(gain) => money(gain),
            None => "—".to_owned(),
        },
        change_24h_pct: markup.escape(&format::percent(wallet.change_24h_pct)),
        positive: wallet.change_24h.is_some_and(|gain| gain >= 0.0),
        positions: wallet
            .positions
            .iter()
            .map(|position| HoldingRow {
                symbol: markup.escape(&position.symbol.to_uppercase()),
                quantity: markup.escape(&format::quantity(position.quantity)),
                value: money(position.value),
                share: (position.share * 1000.0).round() / 1000.0,
                color: position.color.clone(),
            })
            .collect(),
    }
}
