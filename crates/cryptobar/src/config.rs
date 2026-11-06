//! Loading, validating and writing the configuration file.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{CoinId, Currency};
use crate::onchain::OnchainError;
use crate::paths::{self, PathsError};
use crate::render::format::Decimals;

/// Failures to read, write or validate the configuration file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The XDG directories could not be resolved.
    #[error(transparent)]
    Paths(#[from] PathsError),
    /// The configuration file could not be read.
    #[error("reading the configuration {}", .path.display())]
    Read {
        /// File that could not be read.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The configuration file could not be written.
    #[error("writing {}", .path.display())]
    Write {
        /// File that could not be written.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The directory of the configuration file could not be created.
    #[error("creating {}", .path.display())]
    Directory {
        /// Directory that could not be created.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The configuration file already exists and `--force` was not given.
    #[error("{} already exists, use --force to overwrite it", .path.display())]
    Exists {
        /// File that already exists.
        path: PathBuf,
    },
    /// The file is not valid TOML.
    #[error("malformed TOML")]
    Toml(#[from] toml::de::Error),
    /// The configuration file is invalid.
    #[error("invalid configuration {}", .path.display())]
    File {
        /// File that failed validation.
        path: PathBuf,
        /// The validation failure that made the file invalid.
        #[source]
        source: Box<Self>,
    },
    /// `coins` is empty.
    #[error("`coins` must hold at least one cryptocurrency")]
    NoCoin,
    /// `coins` lists the same cryptocurrency twice.
    #[error("`coins` holds a duplicate: {0}")]
    DuplicateCoin(CoinId),
    /// `refresh_interval` is below the minimum.
    #[error(
        "`refresh_interval` = {value} is too short; {min} s minimum to stay within the public \
         CoinGecko API quota"
    )]
    RefreshTooShort {
        /// Value written in the file, in seconds.
        value: u64,
        /// Floor the value must reach, in seconds.
        min: u64,
    },
    /// `waybar_signal` is outside `1..=30`.
    #[error("`waybar_signal` = {0} is out of range; expected 1..=30 (SIGRTMIN+n)")]
    SignalOutOfRange(u8),
    /// `sparkline_width` is outside `1..=64`.
    #[error("`sparkline_width` = {0} is out of range; expected 1..=64")]
    SparklineOutOfRange(usize),
    /// A watched wallet address was rejected.
    #[error("`wallets.address` = {address:?}")]
    Wallet {
        /// The address as written in the configuration.
        address: String,
        /// Why the address was rejected.
        #[source]
        reason: OnchainError,
    },
    /// An alert watches a cryptocurrency missing from `coins`.
    #[error("the alert watches `{0}`, which is not in `coins`: add it there first")]
    AlertUnknownCoin(CoinId),
    /// An alert threshold is not strictly positive.
    #[error("`alerts.{field}` = {value} must be a strictly positive number")]
    AlertNotPositive {
        /// Name of the offending field in the `[[alerts]]` block.
        field: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// An alert defines no condition for its cryptocurrency.
    #[error("the alert on `{0}` defines no condition: set `above`, `below` or `change_pct`")]
    AlertNoCondition(CoinId),
    /// An alert has a floor above its ceiling.
    #[error(
        "the alert on `{coin}` has `below` = {below} above `above` = {above}: it would fire \
         permanently"
    )]
    AlertInverted {
        /// Cryptocurrency watched by the alert.
        coin: CoinId,
        /// Floor of the alert, in the quote currency.
        below: f64,
        /// Ceiling of the alert, in the quote currency.
        above: f64,
    },
    /// `decimals` is neither `auto` nor an integer.
    #[error("`decimals` must be \"auto\" or an integer, not {0:?}")]
    Decimals(String),
}

/// Floor of the refresh interval. Below it, the public CoinGecko API answers
/// with 429s and eventually bans the address for a while.
pub const MIN_REFRESH_INTERVAL: u64 = 15;

pub(crate) const DEFAULT_CONFIG: &str = r#"# cryptobar configuration: https://github.com/Akayashuu/cryptobar
# CoinGecko ids; look them up with `cryptobar coins <term>`.
coins = ["bitcoin", "ethereum", "solana"]

# Quote currency: eur, usd, gbp, chf, cad, jpy, btc, eth
currency = "eur"

# Price refresh interval, in seconds (minimum 15).
refresh_interval = 60

# Automatic rotation between coins, in seconds. 0 = manual scroll only.
cycle_interval = 0

# Inline sparkline in the bar.
sparkline = true
sparkline_width = 8

# Decimals: "auto" adapts to the order of magnitude, or an integer to pin them.
decimals = "auto"

# Terminal emulator opened by the right click.
terminal = "kitty"

# Real-time signal used to refresh Waybar: SIGRTMIN+<n>, 1..=30.
waybar_signal = 8

# Wallets watched read-only. Only a PUBLIC ADDRESS goes here: cryptobar reads
# its balance from the chain and adds it to the portfolio. No private key, no
# recovery phrase: they are rejected.
# Supported chains: "bitcoin", "ethereum", "solana".
# [[wallets]]
# chain = "bitcoin"
# address = "bc1q…"
# label = "Ledger"

# Price alerts. Each block watches one coin; `above` and `below` are expressed
# in the currency above, `change_pct` as an absolute 24 h change in percent. An
# alert re-arms as soon as its condition becomes false again.
# [[alerts]]
# coin = "bitcoin"
# above = 100000
#
# [[alerts]]
# coin = "ethereum"
# change_pct = 5
"#;

/// How many decimals the prices are displayed with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DecimalsSetting {
    /// Decimals pinned to this count.
    Fixed(u8),
    /// Automatic decimals, written as the string `auto` in the file.
    Auto(String),
}

impl Default for DecimalsSetting {
    fn default() -> Self {
        Self::Auto("auto".to_owned())
    }
}

/// Notification threshold on a watched cryptocurrency.
///
/// The three conditions are independent and can be combined: a single block can
/// notify on crossing a ceiling, a floor, and a 24 h change amplitude.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alert {
    /// Cryptocurrency watched by the alert.
    pub coin: CoinId,
    /// Ceiling in the quote currency: the alert fires when the price rises above it.
    #[serde(default)]
    pub above: Option<f64>,
    /// Floor in the quote currency: the alert fires when the price falls below it.
    #[serde(default)]
    pub below: Option<f64>,
    /// Absolute 24 h change, in percent, beyond which the alert fires.
    #[serde(default)]
    pub change_pct: Option<f64>,
    /// Whether the alert is evaluated. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// The configuration file, as parsed and validated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// CoinGecko ids of the watched cryptocurrencies, in display order.
    pub coins: Vec<CoinId>,
    /// Quote currency of every price.
    #[serde(default)]
    pub currency: Currency,
    /// Seconds between two network refreshes, at least [`MIN_REFRESH_INTERVAL`].
    #[serde(default = "default_refresh")]
    pub refresh_interval: u64,
    /// Seconds between two automatic rotations of the displayed coin, `0` for manual only.
    #[serde(default)]
    pub cycle_interval: u64,
    /// Whether the bar draws a sparkline.
    #[serde(default = "default_true")]
    pub sparkline: bool,
    /// Width of the sparkline in cells, between 1 and 64.
    #[serde(default = "default_sparkline_width")]
    pub sparkline_width: usize,
    /// Decimal places of the displayed prices.
    #[serde(default)]
    pub decimals: DecimalsSetting,
    /// Terminal emulator opened by a right click.
    #[serde(default = "default_terminal")]
    pub terminal: String,
    /// Real-time signal number, between 1 and 30, that refreshes Waybar.
    #[serde(default = "default_signal")]
    pub waybar_signal: u8,
    /// Wallets watched read-only, by public address.
    #[serde(default)]
    pub wallets: Vec<crate::onchain::Wallet>,
    /// Notification thresholds. As `[[alerts]]` is an array of tables it has to
    /// stay the last serialised field; it already is by construction.
    #[serde(default)]
    pub alerts: Vec<Alert>,
}

fn default_refresh() -> u64 {
    60
}
pub(crate) fn default_true() -> bool {
    true
}
fn default_sparkline_width() -> usize {
    8
}
fn default_terminal() -> String {
    "kitty".to_owned()
}
fn default_signal() -> u8 {
    8
}

impl Alert {
    fn validate(&self, coins: &[CoinId]) -> Result<(), ConfigError> {
        if !coins.contains(&self.coin) {
            return Err(ConfigError::AlertUnknownCoin(self.coin.clone()));
        }
        for (field, value) in [
            ("above", self.above),
            ("below", self.below),
            ("change_pct", self.change_pct),
        ] {
            if let Some(value) = value
                && (!value.is_finite() || value <= 0.0)
            {
                return Err(ConfigError::AlertNotPositive { field, value });
            }
        }
        if self.above.is_none() && self.below.is_none() && self.change_pct.is_none() {
            return Err(ConfigError::AlertNoCondition(self.coin.clone()));
        }
        if let (Some(above), Some(below)) = (self.above, self.below)
            && below >= above
        {
            return Err(ConfigError::AlertInverted {
                coin: self.coin.clone(),
                below,
                above,
            });
        }
        Ok(())
    }
}

fn ensure_directory(dir: &Path) -> Result<(), ConfigError> {
    paths::ensure_dir(dir).map_err(|source| ConfigError::Directory {
        path: dir.to_path_buf(),
        source,
    })
}

impl Config {
    /// Loads the configuration from the XDG path, creating it when missing.
    ///
    /// # Errors
    /// Fails when the configuration is unreadable or invalid.
    pub fn load() -> Result<Self, ConfigError> {
        let path = paths::config_file()?;
        if !path.exists() {
            if let Some(parent) = path.parent() {
                ensure_directory(parent)?;
            }
            Self::write_default(&path, false)?;
        }
        Self::load_from(&path)
    }

    /// Loads and validates the configuration from an explicit path.
    ///
    /// # Errors
    /// Fails when the file is unreadable or invalid.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&raw).map_err(|source| ConfigError::File {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    /// Parses and validates a TOML payload.
    ///
    /// # Errors
    /// Fails when the TOML is malformed or when a value breaks the contract.
    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(raw)?;
        config.validate()?;
        Ok(config)
    }

    /// Writes the default configuration, never overwriting an existing file.
    ///
    /// # Errors
    /// Fails when the file already exists or cannot be written.
    pub fn write_default(path: &Path, force: bool) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            ensure_directory(parent)?;
        }
        if path.exists() && !force {
            return Err(ConfigError::Exists {
                path: path.to_path_buf(),
            });
        }
        std::fs::write(path, DEFAULT_CONFIG).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.coins.is_empty() {
            return Err(ConfigError::NoCoin);
        }
        let mut seen = std::collections::HashSet::new();
        for coin in &self.coins {
            if !seen.insert(coin.as_str()) {
                return Err(ConfigError::DuplicateCoin(coin.clone()));
            }
        }
        if self.refresh_interval < MIN_REFRESH_INTERVAL {
            return Err(ConfigError::RefreshTooShort {
                value: self.refresh_interval,
                min: MIN_REFRESH_INTERVAL,
            });
        }
        if !(1..=30).contains(&self.waybar_signal) {
            return Err(ConfigError::SignalOutOfRange(self.waybar_signal));
        }
        if !(1..=64).contains(&self.sparkline_width) {
            return Err(ConfigError::SparklineOutOfRange(self.sparkline_width));
        }
        for wallet in &self.wallets {
            wallet.validate().map_err(|reason| ConfigError::Wallet {
                address: wallet.address.clone(),
                reason,
            })?;
        }
        for alert in &self.alerts {
            alert.validate(&self.coins)?;
        }
        if let DecimalsSetting::Auto(value) = &self.decimals
            && value != "auto"
        {
            return Err(ConfigError::Decimals(value.clone()));
        }
        Ok(())
    }

    /// Turns the decimals setting into a value the formatter can use.
    #[must_use]
    pub fn decimals(&self) -> Decimals {
        match &self.decimals {
            DecimalsSetting::Fixed(n) => Decimals::Fixed(*n),
            DecimalsSetting::Auto(_) => Decimals::Auto,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
