//! Price threshold evaluation.
//!
//! The module is deliberately pure: it turns a configuration and a batch of
//! quotes into triggers. The notification itself is a side effect isolated in
//! [`notify`], and the memory of what already fired lives in the runtime state.
//! That separation makes the anti-repeat rule testable without a network or a
//! notification daemon.
//!
//! Re-arming rule: an alert stays "fired" as long as its condition holds, and
//! re-arms as soon as it becomes false again. A price oscillating around the
//! threshold therefore only notifies on crossings, and a price that stays above
//! only notifies once.

use std::collections::BTreeSet;

use crate::config::{Alert, Config};
use crate::model::{Currency, Quote};
use crate::render::format::{self, Decimals};

/// A crossed condition, in a form comparable from one tick to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Above,
    Below,
    Change,
}

impl Kind {
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Above => "above",
            Self::Below => "below",
            Self::Change => "change",
        }
    }
}

/// A condition that holds right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Stable identity of the condition, independent of the current price: it
    /// is what gets memorised so the same thing is not notified twice.
    pub key: String,
    pub symbol: String,
    pub kind: Kind,
    /// Notification title, short and readable in a corner of the screen.
    pub title: String,
    pub body: String,
}

/// Key of a condition. The threshold is part of it: changing the threshold in
/// the configuration naturally re-arms the alert.
fn key(coin: &str, kind: Kind, threshold: f64) -> String {
    format!("{coin}:{}:{threshold}", kind.slug())
}

/// Every condition that holds for this batch of quotes.
///
/// Disabled alerts, and those whose coin has no quote, are ignored: a network
/// outage must neither notify nor re-arm anything.
#[must_use]
pub fn evaluate(config: &Config, quotes: &[Quote]) -> Vec<Trigger> {
    let decimals = config.decimals();
    config
        .alerts
        .iter()
        .filter(|alert| alert.enabled)
        .filter_map(|alert| {
            let quote = quotes.iter().find(|quote| quote.id == alert.coin)?;
            Some(triggers_for(alert, quote, &config.currency, decimals))
        })
        .flatten()
        .collect()
}

fn triggers_for(
    alert: &Alert,
    quote: &Quote,
    currency: &Currency,
    decimals: Decimals,
) -> Vec<Trigger> {
    let coin = alert.coin.as_str();
    let price = format::price(quote.price, currency, decimals);
    let mut triggers = Vec::new();

    if let Some(threshold) = alert.above.filter(|threshold| quote.price >= *threshold) {
        triggers.push(Trigger {
            key: key(coin, Kind::Above, threshold),
            symbol: quote.symbol.clone(),
            kind: Kind::Above,
            title: format!(
                "{} above {}",
                quote.symbol,
                threshold_label(threshold, currency, decimals)
            ),
            body: format!("{} is at {price}.", quote.name),
        });
    }
    if let Some(threshold) = alert.below.filter(|threshold| quote.price <= *threshold) {
        triggers.push(Trigger {
            key: key(coin, Kind::Below, threshold),
            symbol: quote.symbol.clone(),
            kind: Kind::Below,
            title: format!(
                "{} below {}",
                quote.symbol,
                threshold_label(threshold, currency, decimals)
            ),
            body: format!("{} is at {price}.", quote.name),
        });
    }
    if let Some(threshold) = alert.change_pct {
        // The amplitude is absolute: a 6% drop counts as much as a 6% rise,
        // which is what a volatility alert should do.
        let amplitude = quote
            .change_24h
            .filter(|change| change.is_finite())
            .map(f64::abs);
        if amplitude.is_some_and(|amplitude| amplitude >= threshold) {
            triggers.push(Trigger {
                key: key(coin, Kind::Change, threshold),
                symbol: quote.symbol.clone(),
                kind: Kind::Change,
                title: format!(
                    "{} moved by {}",
                    quote.symbol,
                    format::percent(quote.change_24h)
                ),
                body: format!("{} is at {price} after 24 h.", quote.name),
            });
        }
    }
    triggers
}

fn threshold_label(value: f64, currency: &Currency, decimals: Decimals) -> String {
    format::price(value, currency, decimals)
}

/// Splits the holding conditions into "new" and "already known", and returns
/// the set to memorise for the next tick.
#[must_use]
pub fn diff(triggers: &[Trigger], fired: &BTreeSet<String>) -> (Vec<Trigger>, BTreeSet<String>) {
    let current: BTreeSet<String> = triggers.iter().map(|t| t.key.clone()).collect();
    let fresh = triggers
        .iter()
        .filter(|trigger| !fired.contains(&trigger.key))
        .cloned()
        .collect();
    (fresh, current)
}

/// Sends a desktop notification. A missing `notify-send` or notification
/// daemon is not an error: the bar keeps showing the alert, only the bubble is
/// missing.
pub fn notify(trigger: &Trigger) {
    let urgency = if trigger.kind == Kind::Change {
        "normal"
    } else {
        "critical"
    };
    let _ = std::process::Command::new("notify-send")
        .arg("--app-name=cryptobar")
        .arg("--icon=network-transmit-receive")
        .arg(format!("--urgency={urgency}"))
        // One alert per coin and per condition: the bubbles replace each other
        // instead of stacking up when the price oscillates around the threshold.
        .arg(format!(
            "--hint=string:x-canonical-private-synchronous:cryptobar-{}",
            trigger.key
        ))
        .arg(&trigger.title)
        .arg(&trigger.body)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|mut child| child.wait());
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

    fn cfg(alerts: &str) -> Config {
        Config::parse(&format!("coins = [\"bitcoin\"]\n{alerts}")).unwrap()
    }

    #[test]
    fn a_crossed_ceiling_fires() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]);
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers.first().unwrap().kind, Kind::Above);
    }

    #[test]
    fn an_unreached_ceiling_does_not_fire() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        assert_eq!(evaluate(&config, &[quote("bitcoin", "BTC", 99_000.0, None)]).len(), 0);
    }

    #[test]
    fn a_crossed_floor_fires() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nbelow = 50000\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 49_000.0, None)]);
        assert_eq!(triggers.first().unwrap().kind, Kind::Below);
    }

    #[test]
    fn the_amplitude_counts_both_ways() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nchange_pct = 5\n");
        assert_eq!(
            evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(-6.0))]).len(),
            1
        );
        assert_eq!(
            evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(6.0))]).len(),
            1
        );
        assert_eq!(evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(2.0))]).len(), 0);
    }

    #[test]
    fn a_missing_change_does_not_fire() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nchange_pct = 5\n");
        assert_eq!(evaluate(&config, &[quote("bitcoin", "BTC", 1.0, None)]).len(), 0);
        assert_eq!(evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(f64::NAN))]).len(), 0);
    }

    #[test]
    fn a_disabled_alert_is_ignored() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 1\nenabled = false\n");
        assert_eq!(evaluate(&config, &[quote("bitcoin", "BTC", 100.0, None)]).len(), 0);
    }

    #[test]
    fn without_a_quote_nothing_fires_or_re_arms() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 1\n");
        assert_eq!(evaluate(&config, &[]).len(), 0);
    }

    #[test]
    fn an_already_fired_condition_does_not_notify_again() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]);
        let (fresh, fired) = diff(&triggers, &BTreeSet::new());
        assert_eq!(fresh.len(), 1);
        let (again, still) = diff(&triggers, &fired);
        assert_eq!(again.len(), 0);
        assert_eq!(still, fired);
    }

    #[test]
    fn a_condition_that_falls_back_re_arms() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let (_, fired) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &BTreeSet::new(),
        );
        // The price falls back under the threshold: no condition holds any more.
        let (_, cleared) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 90_000.0, None)]),
            &fired,
        );
        assert_eq!(cleared.len(), 0);
        // Then it rises above again: it has to notify once more.
        let (fresh, _) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &cleared,
        );
        assert_eq!(fresh.len(), 1);
    }

    #[test]
    fn changing_the_threshold_re_arms_the_alert() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let (_, fired) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &BTreeSet::new(),
        );
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100500\n");
        let (fresh, _) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &fired,
        );
        assert_eq!(fresh.len(), 1);
    }

    #[test]
    fn one_block_can_combine_several_conditions() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\nchange_pct = 5\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, Some(9.0))]);
        assert_eq!(triggers.len(), 2);
    }
}
