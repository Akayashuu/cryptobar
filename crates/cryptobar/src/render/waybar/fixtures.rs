use std::collections::BTreeMap;

use crate::config::Config;
use crate::model::{CoinId, Quote};

pub(super) fn config() -> Config {
    Config::parse("coins = [\"bitcoin\", \"ethereum\"]").unwrap()
}

pub(super) fn quote(symbol: &str, price: f64, change: Option<f64>) -> Quote {
    Quote {
        id: CoinId::parse("bitcoin").unwrap(),
        symbol: symbol.to_owned(),
        name: symbol.to_owned(),
        price,
        change_1h: Some(0.5),
        change_24h: change,
        change_7d: Some(-2.0),
        sparkline: vec![1.0, 2.0, 3.0, 2.0],
        image: None,
        market_cap: None,
        volume_24h: None,
        rank: None,
        ath: None,
        ath_date: None,
        icon: None,
    }
}

pub(super) fn no_onchain() -> BTreeMap<CoinId, f64> {
    BTreeMap::new()
}
