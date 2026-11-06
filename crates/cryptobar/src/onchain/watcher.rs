use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::balance::Balance;
use super::chain::Chain;
use super::decode::{decode_bitcoin, decode_ethereum, decode_solana};
use super::error::OnchainError;
use super::wallet::Wallet;
use crate::paths;

const USER_AGENT: &str = concat!(
    "cryptobar/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Akayashuu/cryptobar)"
);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
const TTL: Duration = Duration::from_secs(300);

fn endpoint(chain: Chain) -> String {
    let (var, default) = match chain {
        Chain::Bitcoin => ("CRYPTOBAR_BTC_API", "https://mempool.space/api"),
        Chain::Ethereum => ("CRYPTOBAR_ETH_RPC", "https://ethereum-rpc.publicnode.com"),
        Chain::Solana => ("CRYPTOBAR_SOL_RPC", "https://api.mainnet-beta.solana.com"),
    };
    match env::var(var) {
        Ok(base) if !base.trim().is_empty() => base.trim().trim_end_matches('/').to_owned(),
        _ => default.to_owned(),
    }
}

/// Balances read for a set of wallets, with the reads that failed.
///
/// A wallet whose read failed keeps its previous balance when one is cached: the
/// failure is listed so that the caller can tell the user, never swallowed.
#[derive(Debug, Default)]
pub struct Refresh {
    /// Balances read successfully, one per wallet.
    pub balances: Vec<Balance>,
    /// Reads that failed, with their cause.
    pub failures: Vec<OnchainError>,
}

/// Balance reader, with its disk cache.
#[derive(Debug)]
pub struct Watcher {
    agent: ureq::Agent,
    path: PathBuf,
}

impl Watcher {
    /// Watcher whose balance cache lives in `cache_dir`.
    #[must_use]
    pub fn new(cache_dir: &Path) -> Self {
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
    pub fn cached(&self, wallets: &[Wallet]) -> Refresh {
        match self.read() {
            Ok(stored) => Refresh {
                balances: Self::select(&stored, wallets),
                failures: Vec::new(),
            },
            Err(failure) => Refresh {
                balances: Vec::new(),
                failures: vec![failure],
            },
        }
    }

    /// Up-to-date balances: the still-fresh entries come from the cache, the
    /// others from the chain. A failed read leaves the previous value in place
    /// rather than wiping the portfolio, and is listed in the failures.
    #[must_use]
    pub fn refresh(&self, wallets: &[Wallet]) -> Refresh {
        let mut failures = Vec::new();
        let mut stored = match self.read() {
            Ok(stored) => stored,
            Err(failure) => {
                failures.push(failure);
                BTreeMap::new()
            }
        };
        let now = now();
        let mut changed = false;

        for wallet in wallets.iter().filter(|wallet| wallet.enabled) {
            let fresh = stored
                .get(&wallet.address)
                .is_some_and(|balance| now.saturating_sub(balance.fetched_at) < TTL.as_secs());
            if fresh {
                continue;
            }
            if let Err(invalid) = wallet.validate() {
                failures.push(invalid);
                continue;
            }
            match self.fetch(wallet.chain, &wallet.address) {
                Ok(amount) => {
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
                Err(failure) => failures.push(failure),
            }
        }

        let known: BTreeSet<&str> = wallets
            .iter()
            .map(|wallet| wallet.address.as_str())
            .collect();
        let before = stored.len();
        stored.retain(|address, _| known.contains(address.as_str()));
        if (changed || stored.len() != before)
            && let Err(failure) = self.write(&stored)
        {
            failures.push(failure);
        }
        Refresh {
            balances: Self::select(&stored, wallets),
            failures,
        }
    }

    fn select(stored: &BTreeMap<String, Balance>, wallets: &[Wallet]) -> Vec<Balance> {
        wallets
            .iter()
            .filter(|wallet| wallet.enabled)
            .filter_map(|wallet| stored.get(&wallet.address).cloned())
            .collect()
    }

    fn fetch(&self, chain: Chain, address: &str) -> Result<f64, OnchainError> {
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

    fn get(&self, url: &str) -> Result<String, OnchainError> {
        Ok(self
            .agent
            .get(url)
            .call()?
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()?)
    }

    fn rpc(
        &self,
        url: &str,
        method: &str,
        params: &[serde_json::Value],
    ) -> Result<String, OnchainError> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        Ok(self
            .agent
            .post(url)
            .send_json(&request)?
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()?)
    }

    fn read(&self) -> Result<BTreeMap<String, Balance>, OnchainError> {
        match fs::read_to_string(&self.path) {
            Ok(raw) => Ok(serde_json::from_str(&raw)?),
            Err(absent) if absent.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(unreadable) => Err(unreadable.into()),
        }
    }

    fn write(&self, balances: &BTreeMap<String, Balance>) -> Result<(), OnchainError> {
        if let Some(dir) = self.path.parent() {
            paths::ensure_dir(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec(balances)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
