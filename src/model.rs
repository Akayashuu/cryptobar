//! Domain types. No I/O, no dependency on the network or on rendering.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// CoinGecko currencies we expose, with their display symbol.
const CURRENCIES: &[(&str, &str)] = &[
    ("eur", "€"),
    ("usd", "$"),
    ("gbp", "£"),
    ("chf", "CHF"),
    ("cad", "CA$"),
    ("jpy", "¥"),
    ("btc", "₿"),
    ("eth", "Ξ"),
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("the cryptocurrency id is empty")]
    EmptyCoinId,
    #[error(
        "invalid cryptocurrency id: {0:?} (expected lowercase letters, digits and dashes, \
         like `bitcoin` — search with `cryptobar coins <term>`)"
    )]
    InvalidCoinId(String),
    #[error("unsupported currency: {0:?} (supported: eur, usd, gbp, chf, cad, jpy, btc, eth)")]
    UnsupportedCurrency(String),
    #[error("unknown range: {0:?} (expected 1h, 24h, 7d, 30d, 1y)")]
    UnknownRange(String),
}

/// Validated CoinGecko id. The constructor is the only way in, which guarantees
/// no unfiltered string can ever reach a URL or a cache path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CoinId(String);

impl CoinId {
    /// # Errors
    /// Returns [`ModelError::EmptyCoinId`] when the input is empty once trimmed, and
    /// [`ModelError::InvalidCoinId`] when it holds anything other than lowercase
    /// letters, digits and dashes.
    pub fn parse(raw: &str) -> Result<Self, ModelError> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Err(ModelError::EmptyCoinId);
        }
        let valid = trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid || trimmed.starts_with('-') || trimmed.ends_with('-') || trimmed.len() > 64 {
            return Err(ModelError::InvalidCoinId(raw.to_owned()));
        }
        Ok(Self(trimmed))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Id hard-coded in the source, therefore already valid. Reserved for the
    /// program's own constants: anything coming from outside goes through
    /// [`CoinId::parse`].
    #[must_use]
    pub(crate) fn trusted(raw: &'static str) -> Self {
        Self(raw.to_owned())
    }
}

impl TryFrom<String> for CoinId {
    type Error = ModelError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl fmt::Display for CoinId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated quote currency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Currency(String);

impl Currency {
    /// # Errors
    /// Returns [`ModelError::UnsupportedCurrency`] when the currency is not supported.
    pub fn parse(raw: &str) -> Result<Self, ModelError> {
        let lowered = raw.trim().to_ascii_lowercase();
        if CURRENCIES.iter().any(|(code, _)| *code == lowered) {
            Ok(Self(lowered))
        } else {
            Err(ModelError::UnsupportedCurrency(raw.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Every supported quote currency, in display order.
    #[must_use]
    pub fn all() -> Vec<Self> {
        CURRENCIES
            .iter()
            .map(|(code, _)| Self((*code).to_owned()))
            .collect()
    }

    #[must_use]
    pub fn symbol(&self) -> &'static str {
        CURRENCIES
            .iter()
            .find(|(code, _)| *code == self.0)
            .map_or("", |(_, symbol)| *symbol)
    }
}

impl TryFrom<String> for Currency {
    type Error = ModelError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl Default for Currency {
    fn default() -> Self {
        Self("eur".to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Up,
    Down,
    Flat,
}

impl Trend {
    #[must_use]
    pub fn class(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Flat => "flat",
        }
    }
}

/// A cryptocurrency quote at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub id: CoinId,
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change_1h: Option<f64>,
    pub change_24h: Option<f64>,
    pub change_7d: Option<f64>,
    pub sparkline: Vec<f64>,
    /// Official logo URL as provided by the source. Kept in the cache so the
    /// image can be downloaded later, off the critical path of a bar tick.
    #[serde(default)]
    pub image: Option<String>,
    /// Market capitalisation in the quote currency.
    #[serde(default)]
    pub market_cap: Option<f64>,
    /// Volume traded over 24 h in the quote currency.
    #[serde(default)]
    pub volume_24h: Option<f64>,
    /// Rank by market capitalisation, 1 being the highest.
    #[serde(default)]
    pub rank: Option<u32>,
    /// All-time high in the quote currency.
    #[serde(default)]
    pub ath: Option<f64>,
    /// All-time high date, in ISO 8601 as provided by the source.
    #[serde(default)]
    pub ath_date: Option<String>,
    /// Local path of the logo once downloaded. Filled at runtime and never
    /// persisted: a cache path has no business outliving the cache.
    #[serde(skip)]
    pub icon: Option<String>,
}

impl Quote {
    /// Trend shown in the bar, based on the 24 h change.
    /// A missing or non-finite change is treated as flat.
    #[must_use]
    pub fn trend(&self) -> Trend {
        match self.change_24h {
            Some(change) if change.is_finite() && change > 0.0 => Trend::Up,
            Some(change) if change.is_finite() && change < 0.0 => Trend::Down,
            _ => Trend::Flat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Range {
    H1,
    H24,
    D7,
    D30,
    Y1,
}

impl Range {
    /// Value of the CoinGecko API `days` parameter.
    #[must_use]
    pub fn days(self) -> &'static str {
        match self {
            Self::H1 => "0.0417",
            Self::H24 => "1",
            Self::D7 => "7",
            Self::D30 => "30",
            Self::Y1 => "365",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::H1 => "1h",
            Self::H24 => "24h",
            Self::D7 => "7d",
            Self::D30 => "30d",
            Self::Y1 => "1y",
        }
    }

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::H1 => "1h",
            Self::H24 => "24h",
            Self::D7 => "7d",
            Self::D30 => "30d",
            Self::Y1 => "1y",
        }
    }

    /// Time span covered by the range, in minutes. Used to date a point of the
    /// curve: series carry values only, no timestamps.
    #[must_use]
    pub fn span_minutes(self) -> f64 {
        match self {
            Self::H1 => 60.0,
            Self::H24 => 1_440.0,
            Self::D7 => 10_080.0,
            Self::D30 => 43_200.0,
            Self::Y1 => 525_600.0,
        }
    }
}

impl FromStr for Range {
    type Err = ModelError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "1h" => Ok(Self::H1),
            "24h" | "1d" => Ok(Self::H24),
            "7d" | "1w" => Ok(Self::D7),
            "30d" | "1m" => Ok(Self::D30),
            "1y" | "365d" => Ok(Self::Y1),
            other => Err(ModelError::UnknownRange(other.to_owned())),
        }
    }
}

/// Price series for a given range, ordered from oldest to newest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub id: CoinId,
    pub range: Range,
    pub points: Vec<f64>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn coin_id_accepts_coingecko_ids() {
        assert_eq!(CoinId::parse("bitcoin").unwrap().as_str(), "bitcoin");
        assert_eq!(CoinId::parse("  Bitcoin ").unwrap().as_str(), "bitcoin");
        assert_eq!(
            CoinId::parse("matic-network").unwrap().as_str(),
            "matic-network"
        );
    }

    #[test]
    fn coin_id_rejects_what_is_not_an_id() {
        assert!(matches!(CoinId::parse(""), Err(ModelError::EmptyCoinId)));
        assert!(matches!(CoinId::parse("   "), Err(ModelError::EmptyCoinId)));
        assert!(CoinId::parse("bit coin").is_err());
        assert!(CoinId::parse("bitcoin;rm -rf /").is_err());
        assert!(CoinId::parse("../etc/passwd").is_err());
    }

    #[test]
    fn currency_normalises_and_validates() {
        assert_eq!(Currency::parse("EUR").unwrap().as_str(), "eur");
        assert_eq!(Currency::parse("eur").unwrap().symbol(), "€");
        assert_eq!(Currency::parse("usd").unwrap().symbol(), "$");
        assert!(Currency::parse("xyz").is_err());
    }

    #[test]
    fn trend_from_the_24h_change() {
        let q = |c: Option<f64>| Quote {
            id: CoinId::parse("bitcoin").unwrap(),
            symbol: "btc".into(),
            name: "Bitcoin".into(),
            price: 1.0,
            change_1h: None,
            change_24h: c,
            change_7d: None,
            sparkline: Vec::new(),
            image: None,
            market_cap: None,
            volume_24h: None,
            rank: None,
            ath: None,
            ath_date: None,
            icon: None,
        };
        assert_eq!(q(Some(2.0)).trend(), Trend::Up);
        assert_eq!(q(Some(-2.0)).trend(), Trend::Down);
        assert_eq!(q(Some(0.0)).trend(), Trend::Flat);
        assert_eq!(q(None).trend(), Trend::Flat);
        assert_eq!(q(Some(f64::NAN)).trend(), Trend::Flat);
    }

    #[test]
    fn range_parses_and_exposes_its_days() {
        assert_eq!("7d".parse::<Range>().unwrap(), Range::D7);
        assert_eq!("24H".parse::<Range>().unwrap(), Range::H24);
        assert_eq!(Range::H1.days(), "0.0417");
        assert_eq!(Range::Y1.days(), "365");
        assert!("42d".parse::<Range>().is_err());
    }
}
