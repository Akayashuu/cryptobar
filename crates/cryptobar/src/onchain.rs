//! Read-only wallet watching.
//!
//! The principle is "watch-only": you paste a **public address**, cryptobar
//! reads the balance from the chain and values it. No private key, no recovery
//! phrase, no signature — [`Wallet::validate`] explicitly rejects anything that
//! looks like one. The program cannot spend, only look.
//!
//! Three chains, three public endpoints with no API key: mempool.space for
//! Bitcoin, a public Ethereum RPC, the Solana mainnet RPC. Balances are cached
//! on disk, so the bar never opens a connection: it re-reads a file.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::model::CoinId;

/// A watched chain. The list is short on purpose: every entry needs a public,
/// key-free endpoint and a clean balance decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    Bitcoin,
    Ethereum,
    Solana,
}

impl Chain {
    /// Every chain, in display order.
    pub const ALL: [Self; 3] = [Self::Bitcoin, Self::Ethereum, Self::Solana];

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Bitcoin => "bitcoin",
            Self::Ethereum => "ethereum",
            Self::Solana => "solana",
        }
    }

    /// Symbol of the chain's native asset.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Bitcoin => "BTC",
            Self::Ethereum => "ETH",
            Self::Solana => "SOL",
        }
    }

    /// CoinGecko id of the native asset: that is how the balance joins the
    /// portfolio.
    #[must_use]
    pub fn coin(self) -> CoinId {
        CoinId::trusted(self.slug())
    }
}

impl std::str::FromStr for Chain {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "bitcoin" | "btc" => Ok(Self::Bitcoin),
            "ethereum" | "eth" => Ok(Self::Ethereum),
            "solana" | "sol" => Ok(Self::Solana),
            other => Err(format!("unknown chain: {other}")),
        }
    }
}

/// A watched wallet, as written in the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wallet {
    pub chain: Chain,
    /// Public address. See [`Wallet::validate`]: nothing else is accepted.
    pub address: String,
    /// Display name; otherwise the shortened address will do.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "crate::config::default_true")]
    pub enabled: bool,
}

impl Wallet {
    /// Checks that the address has the shape its chain expects.
    ///
    /// The check is deliberately strict: a malformed address would only produce
    /// a useless request, and above all we want to shut the door on anything
    /// that looks like a secret pasted by mistake.
    ///
    /// # Errors
    /// Describes what is wrong, ready to be displayed as is.
    pub fn validate(&self) -> Result<(), String> {
        let address = self.address.trim();
        if address.is_empty() {
            return Err("empty address".to_owned());
        }
        if address.chars().any(char::is_whitespace) {
            // A run of words separated by spaces is a recovery phrase. Refuse
            // outright rather than writing it to disk.
            return Err("a public address contains no space".to_owned());
        }
        let lowered = address.to_ascii_lowercase();
        for secret in [
            "xprv",
            "yprv",
            "zprv",
            "0x0000000000000000000000000000000000000000",
        ] {
            if lowered.starts_with(secret) {
                return Err("this value is not a public address".to_owned());
            }
        }
        match self.chain {
            Chain::Bitcoin => validate_bitcoin(address),
            Chain::Ethereum => validate_ethereum(address),
            Chain::Solana => validate_solana(address),
        }
    }

    /// Display name: the label when there is one, otherwise the short address.
    #[must_use]
    pub fn title(&self) -> String {
        match self
            .label
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            Some(label) => label.to_owned(),
            None => shorten(&self.address),
        }
    }
}

/// Shortens an address keeping both ends, the way explorers do.
#[must_use]
pub fn shorten(address: &str) -> String {
    let chars: Vec<char> = address.chars().collect();
    if chars.len() <= 16 {
        return address.to_owned();
    }
    let head: String = chars.iter().take(6).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}…{tail}")
}

fn validate_bitcoin(address: &str) -> Result<(), String> {
    let lowered = address.to_ascii_lowercase();
    // Bech32 (bc1…): lowercase, without the ambiguous characters `1 b i o`.
    if lowered.starts_with("bc1") {
        let body = lowered.get(3..).unwrap_or_default();
        let ok = (11..=87).contains(&body.len())
            && body
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
            && !body.contains(['b', 'i', 'o'])
            && !body.contains('1');
        return ok
            .then_some(())
            .ok_or_else(|| "malformed bech32 address".to_owned());
    }
    // Legacy base58: starts with 1 (P2PKH) or 3 (P2SH).
    let ok = address.starts_with(['1', '3'])
        && (26..=35).contains(&address.chars().count())
        && address.chars().all(is_base58);
    ok.then_some(())
        .ok_or_else(|| "unrecognised Bitcoin address".to_owned())
}

fn validate_ethereum(address: &str) -> Result<(), String> {
    let ok = address.len() == 42
        && address.starts_with("0x")
        && address
            .get(2..)
            .is_some_and(|body| body.chars().all(|c| c.is_ascii_hexdigit()));
    ok.then_some(())
        .ok_or_else(|| "expected an Ethereum address: 0x followed by 40 characters".to_owned())
}

fn validate_solana(address: &str) -> Result<(), String> {
    let ok = (32..=44).contains(&address.chars().count()) && address.chars().all(is_base58);
    ok.then_some(())
        .ok_or_else(|| "unrecognised Solana address".to_owned())
}

/// Base58 alphabet: no zero, no O, no I, no l — too easy to confuse.
fn is_base58(c: char) -> bool {
    c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l')
}

/// Balance read from the chain, in the unit of the native asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub chain: Chain,
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
    for balance in balances.iter().filter(|b| b.amount.is_finite()) {
        *totals.entry(balance.chain.coin()).or_insert(0.0) += balance.amount;
    }
    totals
}

const USER_AGENT: &str = concat!(
    "cryptobar/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Akayashuu/cryptobar)"
);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
/// The expected responses are a few hundred bytes; this ceiling guards against
/// a server going haywire without ever getting in the way of normal use.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
/// How long a cached balance stays valid. A chain settles in minutes, not in
/// seconds: querying more often would only load the public servers that do us a
/// favour for free.
const TTL: Duration = Duration::from_secs(300);

fn endpoint(chain: Chain) -> String {
    let (var, default) = match chain {
        Chain::Bitcoin => ("CRYPTOBAR_BTC_API", "https://mempool.space/api"),
        Chain::Ethereum => ("CRYPTOBAR_ETH_RPC", "https://ethereum-rpc.publicnode.com"),
        Chain::Solana => ("CRYPTOBAR_SOL_RPC", "https://api.mainnet-beta.solana.com"),
    };
    match std::env::var(var) {
        Ok(base) if !base.trim().is_empty() => base.trim().trim_end_matches('/').to_owned(),
        _ => default.to_owned(),
    }
}

/// Balance reader, with its disk cache.
pub struct Watcher {
    agent: ureq::Agent,
    path: PathBuf,
}

impl Watcher {
    #[must_use]
    pub fn new(cache_dir: &std::path::Path) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(TOTAL_TIMEOUT))
            .user_agent(USER_AGENT)
            .max_redirects(0)
            .build();
        Self {
            agent: config.into(),
            path: cache_dir.join("wallets.json"),
        }
    }

    /// Cached balances, with no network access at all. This is what the bar reads.
    #[must_use]
    pub fn cached(&self, wallets: &[Wallet]) -> Vec<Balance> {
        let stored = self.read();
        wallets
            .iter()
            .filter(|wallet| wallet.enabled)
            .filter_map(|wallet| stored.get(&wallet.address).cloned())
            .collect()
    }

    /// Up-to-date balances: the still-fresh entries come from the cache, the
    /// others from the chain. A failed read leaves the previous value in place
    /// rather than wiping the portfolio.
    #[must_use]
    pub fn refresh(&self, wallets: &[Wallet]) -> Vec<Balance> {
        let mut stored = self.read();
        let now = now();
        let mut changed = false;

        for wallet in wallets.iter().filter(|wallet| wallet.enabled) {
            let fresh = stored
                .get(&wallet.address)
                .is_some_and(|balance| now.saturating_sub(balance.fetched_at) < TTL.as_secs());
            if fresh || wallet.validate().is_err() {
                continue;
            }
            if let Ok(amount) = self.fetch(wallet.chain, &wallet.address) {
                stored.insert(
                    wallet.address.clone(),
                    Balance {
                        chain: wallet.chain,
                        address: wallet.address.clone(),
                        amount,
                        fetched_at: now,
                    },
                );
                changed = true;
            }
        }

        // The cache only keeps the wallets still declared: removing an address
        // from the configuration must erase it from disk.
        let known: std::collections::BTreeSet<&str> =
            wallets.iter().map(|w| w.address.as_str()).collect();
        let before = stored.len();
        stored.retain(|address, _| known.contains(address.as_str()));
        if changed || stored.len() != before {
            let _ = self.write(&stored);
        }
        self.cached(wallets)
    }

    /// Queries the chain for one address.
    fn fetch(&self, chain: Chain, address: &str) -> Result<f64, String> {
        let base = endpoint(chain);
        match chain {
            Chain::Bitcoin => {
                let url = format!("{base}/address/{address}");
                decode_bitcoin(&self.get(&url)?)
            }
            Chain::Ethereum => {
                let body = self.rpc(&base, "eth_getBalance", &[address.into(), "latest".into()])?;
                decode_ethereum(&body)
            }
            Chain::Solana => {
                let body = self.rpc(&base, "getBalance", &[address.into()])?;
                decode_solana(&body)
            }
        }
    }

    fn get(&self, url: &str) -> Result<String, String> {
        self.agent
            .get(url)
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|e| e.to_string())
    }

    fn rpc(&self, url: &str, method: &str, params: &[serde_json::Value]) -> Result<String, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        self.agent
            .post(url)
            .send_json(&request)
            .map_err(|e| e.to_string())?
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|e| e.to_string())
    }

    fn read(&self) -> BTreeMap<String, Balance> {
        // An unreadable cache is treated as missing: never an error.
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn write(&self, balances: &BTreeMap<String, Balance>) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            crate::paths::ensure_dir(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(balances)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Bitcoin balance: what came in minus what went out, confirmed as well as
/// pending, converted from satoshis to BTC.
fn decode_bitcoin(body: &str) -> Result<f64, String> {
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
    let parsed: Address = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let sats = parsed
        .chain_stats
        .funded_txo_sum
        .saturating_sub(parsed.chain_stats.spent_txo_sum)
        .saturating_add(parsed.mempool_stats.funded_txo_sum)
        .saturating_sub(parsed.mempool_stats.spent_txo_sum);
    Ok(units(sats_to_f64(sats), 100_000_000.0))
}

/// Ethereum balance: a hexadecimal integer in wei.
fn decode_ethereum(body: &str) -> Result<f64, String> {
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        result: Option<String>,
    }
    let parsed: Reply = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let raw = parsed
        .result
        .ok_or_else(|| "response without a balance".to_owned())?;
    let digits = raw.trim_start_matches("0x");
    let wei = u128::from_str_radix(digits, 16).map_err(|e| e.to_string())?;
    Ok(units(wei_to_f64(wei), 1e18))
}

/// Solana balance: an integer number of lamports.
fn decode_solana(body: &str) -> Result<f64, String> {
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
    let parsed: Reply = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let lamports = parsed
        .result
        .and_then(|result| result.value)
        .ok_or_else(|| "response without a balance".to_owned())?;
    Ok(units(lamports_to_f64(lamports), 1e9))
}

/// Conversion from a base unit to the displayed unit.
fn units(base: f64, divisor: f64) -> f64 {
    if divisor.abs() < f64::EPSILON {
        return 0.0;
    }
    let value = base / divisor;
    if value.is_finite() { value } else { 0.0 }
}

// The three conversions below lose digits beyond 2^53, which only happens to
// unrealistic balances; the display counts in decimals, not in satoshis.
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn wallet(chain: Chain, address: &str) -> Wallet {
        Wallet {
            chain,
            address: address.to_owned(),
            label: None,
            enabled: true,
        }
    }

    #[test]
    fn a_valid_bitcoin_address_is_accepted() {
        assert!(
            wallet(Chain::Bitcoin, "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq")
                .validate()
                .is_ok()
        );
        assert!(
            wallet(Chain::Bitcoin, "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn a_malformed_bitcoin_address_is_rejected() {
        assert!(wallet(Chain::Bitcoin, "bc1!!!").validate().is_err());
        assert!(wallet(Chain::Bitcoin, "2Nabc").validate().is_err());
    }

    #[test]
    fn an_ethereum_address_must_be_forty_hex_digits() {
        assert!(
            wallet(
                Chain::Ethereum,
                "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
            )
            .validate()
            .is_ok()
        );
        assert!(wallet(Chain::Ethereum, "0x1234").validate().is_err());
    }

    #[test]
    fn a_solana_address_is_base58() {
        assert!(
            wallet(
                Chain::Solana,
                "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
            )
            .validate()
            .is_ok()
        );
        assert!(wallet(Chain::Solana, "0OIl").validate().is_err());
    }

    #[test]
    fn a_recovery_phrase_is_rejected() {
        let phrase = wallet(Chain::Bitcoin, "abandon abandon abandon ability");
        let error = phrase.validate().expect_err("must be rejected");
        assert!(error.contains("space"), "{error}");
    }

    #[test]
    fn an_extended_private_key_is_rejected() {
        let error = wallet(Chain::Bitcoin, "xprv9s21ZrQH143K3QTDL4LXw2F7Hek")
            .validate()
            .expect_err("must be rejected");
        assert!(error.contains("public"), "{error}");
    }

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

    #[test]
    fn a_long_address_displays_shortened() {
        let short = shorten("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
        assert!(short.starts_with("bc1qar"), "{short}");
        assert!(short.ends_with("wf5mdq"), "{short}");
    }
}
