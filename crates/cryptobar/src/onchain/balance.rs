use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::CoinId;

use super::chain::Chain;

/// Balance read from the chain, in the unit of the native asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    /// Chain the address belongs to.
    pub chain: Chain,
    /// Public address that was read.
    pub address: String,
    /// Quantity, in BTC, ETH or SOL depending on the chain.
    pub amount: f64,
    /// Unix time of the last successful read.
    pub fetched_at: u64,
}

/// Balances summed per cryptocurrency, ready to join the portfolio.
#[must_use]
pub fn totals(balances: &[Balance]) -> BTreeMap<CoinId, f64> {
    let mut totals: BTreeMap<CoinId, f64> = BTreeMap::new();
    for balance in balances
        .iter()
        .filter(|candidate| candidate.amount.is_finite())
    {
        *totals.entry(balance.chain.coin()).or_insert(0.0) += balance.amount;
    }
    totals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balances_add_up_per_cryptocurrency() {
        let balances = [
            Balance {
                chain: Chain::Bitcoin,
                address: "a".to_owned(),
                amount: 0.5,
                fetched_at: 0,
            },
            Balance {
                chain: Chain::Bitcoin,
                address: "b".to_owned(),
                amount: 0.25,
                fetched_at: 0,
            },
        ];
        let totals = totals(&balances);
        let sum = totals.get(&Chain::Bitcoin.coin()).copied().unwrap_or(0.0);
        assert!((sum - 0.75).abs() < 1e-9);
    }
}
