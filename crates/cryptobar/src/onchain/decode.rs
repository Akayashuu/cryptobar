use serde::Deserialize;

use super::error::OnchainError;

/// Bitcoin balance: what came in minus what went out, confirmed as well as
/// pending, converted from satoshis to BTC.
pub(super) fn decode_bitcoin(body: &str) -> Result<f64, OnchainError> {
    #[derive(Deserialize, Default)]
    struct Stats {
        #[serde(default)]
        funded_txo_sum: i64,
        #[serde(default)]
        spent_txo_sum: i64,
    }
    #[derive(Deserialize)]
    struct Address {
        #[serde(default)]
        chain_stats: Stats,
        #[serde(default)]
        mempool_stats: Stats,
    }
    let parsed: Address = serde_json::from_str(body)?;
    let sats = parsed
        .chain_stats
        .funded_txo_sum
        .saturating_sub(parsed.chain_stats.spent_txo_sum)
        .saturating_add(parsed.mempool_stats.funded_txo_sum)
        .saturating_sub(parsed.mempool_stats.spent_txo_sum);
    Ok(units(sats_to_f64(sats), 100_000_000.0))
}

/// Ethereum balance: a hexadecimal integer in wei.
pub(super) fn decode_ethereum(body: &str) -> Result<f64, OnchainError> {
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        result: Option<String>,
    }
    let parsed: Reply = serde_json::from_str(body)?;
    let raw = parsed.result.ok_or(OnchainError::MissingBalance)?;
    let digits = raw.trim_start_matches("0x");
    let wei = u128::from_str_radix(digits, 16)?;
    Ok(units(wei_to_f64(wei), 1e18))
}

/// Solana balance: an integer number of lamports.
pub(super) fn decode_solana(body: &str) -> Result<f64, OnchainError> {
    #[derive(Deserialize)]
    struct Value {
        #[serde(default)]
        value: Option<u64>,
    }
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        result: Option<Value>,
    }
    let parsed: Reply = serde_json::from_str(body)?;
    let lamports = parsed
        .result
        .and_then(|result| result.value)
        .ok_or(OnchainError::MissingBalance)?;
    Ok(units(lamports_to_f64(lamports), 1e9))
}

fn units(base: f64, divisor: f64) -> f64 {
    if divisor.abs() < f64::EPSILON {
        return 0.0;
    }
    let value = base / divisor;
    if value.is_finite() { value } else { 0.0 }
}

#[allow(clippy::cast_precision_loss)]
fn sats_to_f64(value: i64) -> f64 {
    value as f64
}

#[allow(clippy::cast_precision_loss)]
fn wei_to_f64(value: u128) -> f64 {
    value as f64
}

#[allow(clippy::cast_precision_loss)]
fn lamports_to_f64(value: u64) -> f64 {
    value as f64
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_bitcoin_balance_counts_in_btc() {
        let body = r#"{"chain_stats":{"funded_txo_sum":150000000,"spent_txo_sum":50000000},
                       "mempool_stats":{"funded_txo_sum":0,"spent_txo_sum":0}}"#;
        let amount = decode_bitcoin(body).expect("readable balance");
        assert!((amount - 1.0).abs() < 1e-9, "{amount}");
    }

    #[test]
    fn the_bitcoin_balance_accounts_for_the_mempool() {
        let body = r#"{"chain_stats":{"funded_txo_sum":100000000,"spent_txo_sum":0},
                       "mempool_stats":{"funded_txo_sum":0,"spent_txo_sum":100000000}}"#;
        assert!(decode_bitcoin(body).expect("readable balance").abs() < 1e-12);
    }

    #[test]
    fn the_ethereum_balance_reads_as_hexadecimal() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"0x0de0b6b3a7640000"}"#;
        let amount = decode_ethereum(body).expect("readable balance");
        assert!((amount - 1.0).abs() < 1e-9, "{amount}");
    }

    #[test]
    fn an_ethereum_response_without_a_result_is_an_error() {
        let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000}}"#;
        assert!(decode_ethereum(body).is_err());
    }

    #[test]
    fn the_solana_balance_counts_in_lamports() {
        let body = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1},"value":2500000000},"id":1}"#;
        let amount = decode_solana(body).expect("readable balance");
        assert!((amount - 2.5).abs() < 1e-9, "{amount}");
    }
}
