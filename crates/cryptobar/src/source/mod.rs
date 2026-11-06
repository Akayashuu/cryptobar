//! Market data access. The network always sits behind the [`PriceSource`]
//! trait, which lets everything else be tested without leaving the machine.

pub mod cache;
pub mod coingecko;
pub mod icons;

use std::io;

use crate::model::{CoinId, Currency, Quote, Range, Series};

/// Failures of the market data sources.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// The HTTP request failed with this message.
    #[error("network error: {0}")]
    Http(String),
    /// The API refused the request because its quota is exhausted.
    #[error("API quota exceeded")]
    RateLimited {
        /// Seconds the API asked to wait, when it said so.
        retry_after: Option<u64>,
    },
    /// The response body is not valid JSON.
    #[error("unreadable response")]
    Decode(#[from] serde_json::Error),
    /// The request could not reach the API.
    #[error("network error")]
    Transport(#[from] ureq::Error),
    /// The response is valid JSON but not the expected shape.
    #[error("unexpected response: {0}")]
    Payload(String),
    /// The source is backing off and no cached data is available.
    #[error("backoff active after a network failure, no cached data")]
    Backoff,
    /// The disk cache could not be read or written.
    #[error("local cache")]
    Cache(#[from] io::Error),
}

/// A cryptocurrency returned by a search.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Id, usable with the other commands.
    pub id: CoinId,
    /// Ticker symbol.
    pub symbol: String,
    /// Human name.
    pub name: String,
    /// Official thumbnail URL, as published by the source.
    pub image: Option<String>,
    /// Local path of the logo, filled in after going through the icon cache.
    pub icon: Option<String>,
}

/// Access to market data: the network client, its disk cache and test fakes implement it.
pub trait PriceSource {
    /// Quotes for every requested cryptocurrency, in a single request.
    ///
    /// # Errors
    /// See [`SourceError`].
    fn markets(&self, ids: &[CoinId], vs: &Currency) -> Result<Vec<Quote>, SourceError>;

    /// Price series of a cryptocurrency over a given range.
    ///
    /// # Errors
    /// See [`SourceError`].
    fn series(&self, id: &CoinId, range: Range, vs: &Currency) -> Result<Series, SourceError>;

    /// Free-text search for ids.
    ///
    /// # Errors
    /// See [`SourceError`].
    fn search(&self, term: &str) -> Result<Vec<SearchHit>, SourceError>;
}
