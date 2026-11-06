//! Client of the public CoinGecko API. No key required.

use std::time::Duration;

use serde::Deserialize;

use super::{PriceSource, SearchHit, SourceError};
use crate::model::{CoinId, Currency, Quote, Range, Series};

const DEFAULT_BASE_URL: &str = "https://api.coingecko.com/api/v3";
const USER_AGENT: &str = concat!(
    "cryptobar/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Akayashuu/cryptobar)"
);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
/// Read ceiling for a response: beyond it, we refuse rather than let memory
/// grow on an abnormal response.
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

pub struct CoinGecko {
    base_url: String,
    agent: ureq::Agent,
}

impl Default for CoinGecko {
    fn default() -> Self {
        Self::new()
    }
}

impl CoinGecko {
    #[must_use]
    /// Default client. `CRYPTOBAR_API_BASE` allows targeting another instance
    /// — that is what makes the end-to-end tests hermetic.
    pub fn new() -> Self {
        match std::env::var("CRYPTOBAR_API_BASE") {
            Ok(base) if !base.trim().is_empty() => Self::with_base_url(base.trim()),
            _ => Self::with_base_url(DEFAULT_BASE_URL),
        }
    }

    #[must_use]
    pub fn with_base_url(base_url: &str) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(TOTAL_TIMEOUT))
            .user_agent(USER_AGENT)
            .max_redirects(0)
            .build();
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            agent: config.into(),
        }
    }

    fn markets_url(&self, ids: &[CoinId], vs: &Currency) -> String {
        let joined = ids
            .iter()
            .map(CoinId::as_str)
            .collect::<Vec<_>>()
            .join("%2C");
        format!(
            "{}/coins/markets?vs_currency={}&ids={}\
             &price_change_percentage=1h%2C24h%2C7d&sparkline=true&precision=full",
            self.base_url,
            vs.as_str(),
            joined
        )
    }

    fn chart_url(&self, id: &CoinId, range: Range, vs: &Currency) -> String {
        format!(
            "{}/coins/{}/market_chart?vs_currency={}&days={}",
            self.base_url,
            id.as_str(),
            vs.as_str(),
            range.days()
        )
    }

    fn search_url(&self, term: &str) -> String {
        // The term comes from the user: we keep only safe characters rather
        // than trusting an approximate encoding.
        let encoded: String = term
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ')
            .map(|c| if c == ' ' { '+' } else { c })
            .take(64)
            .collect();
        format!("{}/search?query={encoded}", self.base_url)
    }

    /// Runs a GET request and returns the body as text, capped in size.
    fn get(&self, url: &str) -> Result<String, SourceError> {
        match self.agent.get(url).call() {
            Ok(mut response) => response
                .body_mut()
                .with_config()
                .limit(MAX_RESPONSE_BYTES)
                .read_to_string()
                .map_err(|e| SourceError::Decode(e.to_string())),
            Err(ureq::Error::StatusCode(429)) => {
                Err(SourceError::RateLimited { retry_after: None })
            }
            Err(ureq::Error::StatusCode(code)) => {
                Err(SourceError::Http(format!("HTTP code {code}")))
            }
            Err(e) => Err(SourceError::Http(e.to_string())),
        }
    }
}

#[derive(Deserialize)]
struct RawSparkline {
    #[serde(default)]
    price: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct RawMarket {
    id: String,
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    current_price: Option<f64>,
    #[serde(default)]
    price_change_percentage_1h_in_currency: Option<f64>,
    #[serde(default)]
    price_change_percentage_24h_in_currency: Option<f64>,
    #[serde(default)]
    price_change_percentage_7d_in_currency: Option<f64>,
    #[serde(default)]
    sparkline_in_7d: Option<RawSparkline>,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    market_cap: Option<f64>,
    #[serde(default)]
    total_volume: Option<f64>,
    #[serde(default)]
    market_cap_rank: Option<u32>,
    #[serde(default)]
    ath: Option<f64>,
    #[serde(default)]
    ath_date: Option<String>,
}

/// Converts a `/coins/markets` response. Unusable entries (invalid id, missing
/// price) are skipped rather than failing the whole batch.
///
/// # Errors
/// Returns [`SourceError::Decode`] when the response is not the expected JSON
/// array.
pub fn parse_markets(raw: &str) -> Result<Vec<Quote>, SourceError> {
    let entries: Vec<RawMarket> =
        serde_json::from_str(raw).map_err(|e| SourceError::Decode(e.to_string()))?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let id = CoinId::parse(&entry.id).ok()?;
            let price = entry.current_price.filter(|p| p.is_finite())?;
            let symbol = entry.symbol.to_ascii_uppercase();
            let name = entry.name.unwrap_or_else(|| symbol.clone());
            let sparkline = entry
                .sparkline_in_7d
                .map(|s| {
                    s.price
                        .into_iter()
                        .flatten()
                        .filter(|v| v.is_finite())
                        .collect()
                })
                .unwrap_or_default();
            Some(Quote {
                id,
                symbol,
                name,
                price,
                change_1h: entry.price_change_percentage_1h_in_currency,
                change_24h: entry.price_change_percentage_24h_in_currency,
                change_7d: entry.price_change_percentage_7d_in_currency,
                sparkline,
                image: entry.image.filter(|url| !url.trim().is_empty()),
                market_cap: entry.market_cap.filter(|v| v.is_finite() && *v > 0.0),
                volume_24h: entry.total_volume.filter(|v| v.is_finite() && *v >= 0.0),
                rank: entry.market_cap_rank,
                ath: entry.ath.filter(|v| v.is_finite() && *v > 0.0),
                ath_date: entry.ath_date.filter(|d| !d.trim().is_empty()),
                icon: None,
            })
        })
        .collect())
}

#[derive(Deserialize)]
struct RawChart {
    #[serde(default)]
    prices: Vec<(f64, f64)>,
}

/// Converts a `/coins/{id}/market_chart` response.
///
/// # Errors
/// [`SourceError::Decode`] when the JSON is unreadable, [`SourceError::Payload`]
/// when the series is empty.
pub fn parse_market_chart(id: &CoinId, range: Range, raw: &str) -> Result<Series, SourceError> {
    let chart: RawChart =
        serde_json::from_str(raw).map_err(|e| SourceError::Decode(e.to_string()))?;
    let points: Vec<f64> = chart
        .prices
        .into_iter()
        .map(|(_, price)| price)
        .filter(|p| p.is_finite())
        .collect();
    if points.is_empty() {
        return Err(SourceError::Payload(format!("empty series for {id}")));
    }
    Ok(Series {
        id: id.clone(),
        range,
        points,
    })
}

#[derive(Deserialize)]
struct RawSearch {
    #[serde(default)]
    coins: Vec<RawSearchCoin>,
}

#[derive(Deserialize)]
struct RawSearchCoin {
    id: String,
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    name: String,
    /// 25×25 thumbnail: exactly the size shown in the result list, no need to
    /// pull the full-size logo.
    #[serde(default)]
    thumb: Option<String>,
    #[serde(default)]
    large: Option<String>,
}

fn parse_search(raw: &str) -> Result<Vec<SearchHit>, SourceError> {
    let parsed: RawSearch =
        serde_json::from_str(raw).map_err(|e| SourceError::Decode(e.to_string()))?;
    Ok(parsed
        .coins
        .into_iter()
        .filter_map(|coin| {
            Some(SearchHit {
                id: CoinId::parse(&coin.id).ok()?,
                symbol: coin.symbol.to_ascii_uppercase(),
                name: coin.name,
                image: coin.large.or(coin.thumb),
                icon: None,
            })
        })
        .collect())
}

impl PriceSource for CoinGecko {
    fn markets(&self, ids: &[CoinId], vs: &Currency) -> Result<Vec<Quote>, SourceError> {
        parse_markets(&self.get(&self.markets_url(ids, vs))?)
    }

    fn series(&self, id: &CoinId, range: Range, vs: &Currency) -> Result<Series, SourceError> {
        parse_market_chart(id, range, &self.get(&self.chart_url(id, range, vs))?)
    }

    fn search(&self, term: &str) -> Result<Vec<SearchHit>, SourceError> {
        parse_search(&self.get(&self.search_url(term))?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const MARKETS: &str = include_str!("../../tests/fixtures/markets.json");
    const DEGRADED: &str = include_str!("../../tests/fixtures/markets_degraded.json");
    const CHART: &str = include_str!("../../tests/fixtures/market_chart.json");

    #[test]
    fn parse_markets_reads_a_nominal_response() {
        let quotes = parse_markets(MARKETS).unwrap();
        assert_eq!(quotes.len(), 2);
        let btc = quotes.first().unwrap();
        assert_eq!(btc.id.as_str(), "bitcoin");
        assert_eq!(btc.name, "Bitcoin");
        assert!((btc.price - 108_420.0).abs() < f64::EPSILON);
        assert_eq!(btc.change_24h, Some(2.14));
        assert_eq!(btc.sparkline.len(), 20);
    }

    #[test]
    fn parse_markets_tolerates_a_degraded_response() {
        let quotes = parse_markets(DEGRADED).unwrap();
        assert_eq!(quotes.len(), 2);
        let btc = quotes.first().unwrap();
        assert_eq!(btc.change_24h, None);
        assert_eq!(btc.sparkline.len(), 0);
        let doge = quotes.get(1).unwrap();
        // Without `name`, we fall back to the symbol rather than failing.
        assert_eq!(doge.name, "DOGE");
    }

    #[test]
    fn parse_markets_rejects_json_that_is_not_an_array() {
        assert!(matches!(parse_markets("{}"), Err(SourceError::Decode(_))));
        assert!(matches!(
            parse_markets("not json"),
            Err(SourceError::Decode(_))
        ));
    }

    #[test]
    fn parse_markets_skips_entries_without_a_valid_id() {
        let raw = r#"[{"id": "bit coin", "symbol": "x", "current_price": 1.0}]"#;
        assert_eq!(parse_markets(raw).unwrap().len(), 0);
    }

    #[test]
    fn parse_market_chart_extracts_the_prices_in_order() {
        let id = CoinId::parse("bitcoin").unwrap();
        let series = parse_market_chart(&id, Range::H24, CHART).unwrap();
        assert_eq!(series.points.len(), 4);
        assert!((series.points.last().copied().unwrap() - 108_420.0).abs() < f64::EPSILON);
        assert_eq!(series.range, Range::H24);
    }

    #[test]
    fn parse_market_chart_rejects_an_empty_series() {
        let id = CoinId::parse("bitcoin").unwrap();
        let raw = r#"{"prices": []}"#;
        assert!(matches!(
            parse_market_chart(&id, Range::H24, raw),
            Err(SourceError::Payload(_))
        ));
    }

    #[test]
    fn the_markets_url_encodes_the_parameters_correctly() {
        let gecko = CoinGecko::with_base_url("https://example.test/api/v3");
        let ids = [
            CoinId::parse("bitcoin").unwrap(),
            CoinId::parse("ethereum").unwrap(),
        ];
        let currency = Currency::parse("eur").unwrap();
        let url = gecko.markets_url(&ids, &currency);
        assert!(url.starts_with("https://example.test/api/v3/coins/markets?"));
        assert!(url.contains("ids=bitcoin%2Cethereum"));
        assert!(url.contains("vs_currency=eur"));
        assert!(url.contains("sparkline=true"));
    }

    #[test]
    fn the_search_url_filters_the_dangerous_characters() {
        let gecko = CoinGecko::with_base_url("https://example.test/api/v3");
        let url = gecko.search_url("sol&query=../../etc");
        assert!(!url.contains('&'));
        assert!(!url.contains(".."));
        assert!(url.contains("query=solqueryetc"));
    }
}
