//! Loading, validating and writing the configuration file.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::{CoinId, Currency};
use crate::paths;
use crate::render::format::Decimals;

/// Floor of the refresh interval. Below it, the public CoinGecko API answers
/// with 429s and eventually bans the address for a while.
pub const MIN_REFRESH_INTERVAL: u64 = 15;

pub(crate) const DEFAULT_CONFIG: &str = r#"# cryptobar configuration — https://github.com/Akayashuu/cryptobar
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
# recovery phrase — they are rejected.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DecimalsSetting {
    Fixed(u8),
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
    pub coin: CoinId,
    #[serde(default)]
    pub above: Option<f64>,
    #[serde(default)]
    pub below: Option<f64>,
    #[serde(default)]
    pub change_pct: Option<f64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub coins: Vec<CoinId>,
    #[serde(default)]
    pub currency: Currency,
    #[serde(default = "default_refresh")]
    pub refresh_interval: u64,
    #[serde(default)]
    pub cycle_interval: u64,
    #[serde(default = "default_true")]
    pub sparkline: bool,
    #[serde(default = "default_sparkline_width")]
    pub sparkline_width: usize,
    #[serde(default)]
    pub decimals: DecimalsSetting,
    #[serde(default = "default_terminal")]
    pub terminal: String,
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
    fn validate(&self, coins: &[CoinId]) -> Result<()> {
        if !coins.contains(&self.coin) {
            bail!(
                "the alert watches `{}`, which is not in `coins`: add it there first",
                self.coin
            );
        }
        for (label, value) in [
            ("above", self.above),
            ("below", self.below),
            ("change_pct", self.change_pct),
        ] {
            if let Some(value) = value
                && (!value.is_finite() || value <= 0.0)
            {
                bail!("`alerts.{label}` = {value} must be a strictly positive number");
            }
        }
        if self.above.is_none() && self.below.is_none() && self.change_pct.is_none() {
            bail!(
                "the alert on `{}` defines no condition: set `above`, `below` or \
                 `change_pct`",
                self.coin
            );
        }
        if let (Some(above), Some(below)) = (self.above, self.below)
            && below >= above
        {
            bail!(
                "the alert on `{}` has `below` = {below} above `above` = {above}: it would \
                 fire permanently",
                self.coin
            );
        }
        Ok(())
    }
}

impl Config {
    /// Loads the configuration from the XDG path, creating it when missing.
    ///
    /// # Errors
    /// Fails when the configuration is unreadable or invalid.
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;
        if !path.exists() {
            if let Some(parent) = path.parent() {
                paths::ensure_dir(parent)?;
            }
            Self::write_default(&path, false)?;
        }
        Self::load_from(&path)
    }

    /// Loads and validates the configuration from an explicit path.
    ///
    /// # Errors
    /// Fails when the file is unreadable or invalid.
    pub fn load_from(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading the configuration {}", path.display()))?;
        Self::parse(&raw).with_context(|| format!("invalid configuration: {}", path.display()))
    }

    /// Parses and validates a TOML payload.
    ///
    /// # Errors
    /// Fails when the TOML is malformed or when a value breaks the contract.
    pub fn parse(raw: &str) -> Result<Self> {
        let config: Self = toml::from_str(raw)?;
        config.validate()?;
        Ok(config)
    }

    /// Writes the default configuration, never overwriting an existing file.
    ///
    /// # Errors
    /// Fails when the file already exists or cannot be written.
    pub fn write_default(path: &Path, force: bool) -> Result<()> {
        if let Some(parent) = path.parent() {
            paths::ensure_dir(parent)?;
        }
        if path.exists() && !force {
            bail!(
                "{} already exists, use --force to overwrite it",
                path.display()
            );
        }
        std::fs::write(path, DEFAULT_CONFIG).with_context(|| format!("writing {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
        if self.coins.is_empty() {
            bail!("`coins` must hold at least one cryptocurrency");
        }
        let mut seen = std::collections::HashSet::new();
        for coin in &self.coins {
            if !seen.insert(coin.as_str()) {
                bail!("`coins` holds a duplicate: {coin}");
            }
        }
        if self.refresh_interval < MIN_REFRESH_INTERVAL {
            bail!(
                "`refresh_interval` = {} is too short; {MIN_REFRESH_INTERVAL} s minimum to stay \
                 within the public CoinGecko API quota",
                self.refresh_interval
            );
        }
        if !(1..=30).contains(&self.waybar_signal) {
            bail!(
                "`waybar_signal` = {} is out of range; expected 1..=30 (SIGRTMIN+n)",
                self.waybar_signal
            );
        }
        if !(1..=64).contains(&self.sparkline_width) {
            bail!(
                "`sparkline_width` = {} is out of range; expected 1..=64",
                self.sparkline_width
            );
        }
        for wallet in &self.wallets {
            if let Err(reason) = wallet.validate() {
                bail!("`wallets.address` = {:?}: {reason}", wallet.address);
            }
        }
        for alert in &self.alerts {
            alert.validate(&self.coins)?;
        }
        if let DecimalsSetting::Auto(value) = &self.decimals
            && value != "auto"
        {
            bail!("`decimals` must be \"auto\" or an integer, not {value:?}");
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
mod tests {
    use super::*;

    #[test]
    fn a_minimal_config_uses_the_defaults() {
        let cfg = Config::parse(r#"coins = ["bitcoin"]"#).unwrap();
        assert_eq!(cfg.currency.as_str(), "eur");
        assert_eq!(cfg.refresh_interval, 60);
        assert_eq!(cfg.cycle_interval, 0);
        assert!(cfg.sparkline);
        assert_eq!(cfg.sparkline_width, 8);
        assert_eq!(cfg.waybar_signal, 8);
    }

    #[test]
    fn an_empty_coin_list_is_rejected() {
        let err = Config::parse("coins = []").unwrap_err().to_string();
        assert!(err.contains("at least one"), "unclear message: {err}");
    }

    #[test]
    fn a_duplicate_coin_is_rejected() {
        let err = Config::parse(r#"coins = ["bitcoin", "Bitcoin"]"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("bitcoin"), "unclear message: {err}");
    }

    #[test]
    fn too_short_an_interval_is_rejected_to_protect_the_quota() {
        let err = Config::parse("coins = [\"bitcoin\"]\nrefresh_interval = 5")
            .unwrap_err()
            .to_string();
        assert!(err.contains("15"), "unclear message: {err}");
    }

    #[test]
    fn an_out_of_range_signal_is_rejected() {
        assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 0").is_err());
        assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 31").is_err());
        assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 30").is_ok());
    }

    #[test]
    fn an_invalid_currency_is_rejected_naming_the_field() {
        let err = Config::parse("coins = [\"bitcoin\"]\ncurrency = \"xyz\"")
            .unwrap_err()
            .to_string();
        assert!(err.contains("currency"), "unclear message: {err}");
    }

    #[test]
    fn decimals_are_auto_or_fixed() {
        assert_eq!(
            Config::parse("coins = [\"bitcoin\"]").unwrap().decimals(),
            Decimals::Auto
        );
        assert_eq!(
            Config::parse("coins = [\"bitcoin\"]\ndecimals = 3")
                .unwrap()
                .decimals(),
            Decimals::Fixed(3)
        );
    }

    #[test]
    fn an_unknown_field_is_rejected_to_catch_typos() {
        let err = Config::parse("coins = [\"bitcoin\"]\nrefresh_intervall = 60")
            .unwrap_err()
            .to_string();
        assert!(err.contains("refresh_intervall"), "unclear message: {err}");
    }

    #[test]
    fn the_default_config_written_then_reread_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::write_default(&path, false).unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert!(!cfg.coins.is_empty());
    }

    #[test]
    fn write_default_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::write_default(&path, false).unwrap();
        assert!(Config::write_default(&path, false).is_err());
    }
}
