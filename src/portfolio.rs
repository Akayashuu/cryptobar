//! Portfolio valuation.
//!
//! Pure functions: the quantities come from the balances read on the watched
//! public addresses, the prices from the already cached quotes. Nothing is
//! stored, nothing is sent — cryptobar knows about no exchange and no API key,
//! and the addresses never leave the machine.
//!
//! The price 24 h ago is reconstructed from the relative change rather than
//! through a historical request: `p₀ = p / (1 + v/100)`. That is exact to the
//! precision of the published change, and it costs no network call.

use std::collections::BTreeMap;

use crate::model::{CoinId, Quote};
use crate::render::badge;

/// One line of the portfolio.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub symbol: String,
    /// Cryptocurrency id, so the interface can find the line again.
    pub id: CoinId,
    pub name: String,
    pub quantity: f64,
    pub price: f64,
    /// Quantity × price, in the quote currency.
    pub value: f64,
    /// Share of the total, within `0.0..=1.0`.
    pub share: f64,
    /// Gain or loss over 24 h, in the quote currency.
    pub change_24h: Option<f64>,
    /// Brand colour, for the allocation ring.
    pub color: String,
}

/// Valued portfolio.
#[derive(Debug, Clone, PartialEq)]
pub struct Portfolio {
    /// Positions sorted by decreasing value: the eye goes to the heavyweight
    /// first, in the table as in the ring.
    pub positions: Vec<Position>,
    pub total: f64,
    /// Gain or loss over 24 h, in the quote currency.
    pub change_24h: Option<f64>,
    /// The same thing as a percentage of the portfolio 24 h ago.
    pub change_24h_pct: Option<f64>,
}

/// Values the portfolio, or `None` when no watched address holds a balance.
///
/// A declared coin without a quote is ignored rather than valued at zero: a
/// falsely collapsed total would be worse than a partial one.
#[must_use]
pub fn compute(quotes: &[Quote], onchain: &BTreeMap<CoinId, f64>) -> Option<Portfolio> {
    let mut positions: Vec<Position> = onchain
        .iter()
        .filter(|(_, quantity)| quantity.is_finite() && **quantity > 0.0)
        .filter_map(|(id, quantity)| {
            let quote = quotes.iter().find(|quote| quote.id == *id)?;
            let value = quote.price * quantity;
            if !value.is_finite() {
                return None;
            }
            Some(Position {
                symbol: quote.symbol.clone(),
                id: quote.id.clone(),
                name: quote.name.clone(),
                quantity: *quantity,
                price: quote.price,
                value,
                share: 0.0,
                change_24h: gain_24h(quote.price, quote.change_24h).map(|gain| gain * quantity),
                color: badge::of(&quote.symbol).color,
            })
        })
        .collect();

    if positions.is_empty() {
        return None;
    }

    let total: f64 = positions.iter().map(|position| position.value).sum();
    if total > 0.0 {
        for position in &mut positions {
            position.share = position.value / total;
        }
    }
    positions.sort_by(|a, b| b.value.total_cmp(&a.value));

    // A portfolio where no line publishes a change has no change: an em dash
    // beats a zero that would read as "flat".
    let change_24h = positions
        .iter()
        .any(|position| position.change_24h.is_some())
        .then(|| positions.iter().filter_map(|p| p.change_24h).sum::<f64>());
    let change_24h_pct = change_24h.and_then(|gain| {
        let before = total - gain;
        (before > 0.0).then(|| gain / before * 100.0)
    });

    Some(Portfolio {
        positions,
        total,
        change_24h,
        change_24h_pct,
    })
}

/// Gain over 24 h for one unit, from the current price and the change in
/// percent.
fn gain_24h(price: f64, change_pct: Option<f64>) -> Option<f64> {
    let change = change_pct.filter(|change| change.is_finite())?;
    let factor = 1.0 + change / 100.0;
    // A -100% change would cancel the denominator: better to show nothing
    // than an infinity.
    if factor.abs() < f64::EPSILON {
        return None;
    }
    let before = price / factor;
    let gain = price - before;
    gain.is_finite().then_some(gain)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::CoinId;

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

    fn held(entries: &[(&str, f64)]) -> BTreeMap<CoinId, f64> {
        entries
            .iter()
            .map(|(id, quantity)| (CoinId::parse(id).unwrap(), *quantity))
            .collect()
    }

    #[test]
    fn without_a_balance_there_is_no_portfolio() {
        assert!(compute(&[quote("bitcoin", "BTC", 100.0, None)], &BTreeMap::new()).is_none());
    }

    #[test]
    fn a_zero_balance_is_ignored() {
        assert!(
            compute(
                &[quote("bitcoin", "BTC", 100.0, None)],
                &held(&[("bitcoin", 0.0)])
            )
            .is_none()
        );
    }

    #[test]
    fn the_total_is_the_sum_of_the_lines() {
        let portfolio = compute(
            &[
                quote("bitcoin", "BTC", 100.0, None),
                quote("ethereum", "ETH", 5.0, None),
            ],
            &held(&[("bitcoin", 2.0), ("ethereum", 10.0)]),
        )
        .unwrap();
        assert!((portfolio.total - 250.0).abs() < 1e-9);
        assert_eq!(portfolio.positions.len(), 2);
    }

    #[test]
    fn the_lines_are_sorted_by_decreasing_value() {
        let portfolio = compute(
            &[
                quote("bitcoin", "BTC", 10.0, None),
                quote("ethereum", "ETH", 5.0, None),
            ],
            &held(&[("bitcoin", 1.0), ("ethereum", 10.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.positions.first().unwrap().symbol, "ETH");
        assert!((portfolio.positions.first().unwrap().share - 50.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn a_coin_without_a_quote_is_ignored_rather_than_valued_at_zero() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 100.0, None)],
            &held(&[("bitcoin", 1.0), ("ethereum", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.positions.len(), 1);
        assert!((portfolio.total - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_non_finite_quantity_is_ignored() {
        assert!(
            compute(
                &[quote("bitcoin", "BTC", 100.0, None)],
                &held(&[("bitcoin", f64::NAN)])
            )
            .is_none()
        );
    }

    #[test]
    fn the_24h_gain_is_derived_from_the_change() {
        // 110 today after +10%: 100 yesterday, hence +10 per unit.
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 110.0, Some(10.0))],
            &held(&[("bitcoin", 3.0)]),
        )
        .unwrap();
        assert!((portfolio.change_24h.unwrap() - 30.0).abs() < 1e-9);
        assert!((portfolio.change_24h_pct.unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_loss_is_negative() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, Some(-10.0))],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert!(portfolio.change_24h.unwrap() < 0.0);
        assert!((portfolio.change_24h_pct.unwrap() + 10.0).abs() < 1e-9);
    }

    #[test]
    fn without_any_published_change_the_gain_is_absent() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, None)],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.change_24h, None);
        assert_eq!(portfolio.change_24h_pct, None);
    }

    #[test]
    fn a_non_finite_change_does_not_produce_an_infinity() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, Some(f64::NAN))],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.change_24h, None);
    }

    #[test]
    fn a_hundred_percent_drop_does_not_divide_by_zero() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, Some(-100.0))],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.change_24h, None);
    }

    #[test]
    fn the_shares_sum_to_one() {
        let portfolio = compute(
            &[
                quote("bitcoin", "BTC", 60.0, None),
                quote("ethereum", "ETH", 10.0, None),
            ],
            &held(&[("bitcoin", 1.0), ("ethereum", 4.0)]),
        )
        .unwrap();
        let sum: f64 = portfolio.positions.iter().map(|p| p.share).sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }
}
