use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::balance::Balance;
use super::chain::Chain;
use super::decode::{decode_bitcoin, decode_ethereum, decode_solana};
use super::wallet::Wallet;

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
