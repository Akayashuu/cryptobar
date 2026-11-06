//! Market data access. The network always sits behind the [`PriceSource`]
//! trait, which lets everything else be tested without leaving the machine.

pub mod cache;
pub mod coingecko;
pub mod icons;

use crate::model::{CoinId, Currency, Quote, Range, Series};

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("network error: {0}")]
    Http(String),
    #[error("API quota exceeded")]
    RateLimited { retry_after: Option<u64> },
    #[error("unreadable response: {0}")]
    Decode(String),
    #[error("unexpected response: {0}")]
    Payload(String),
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: CoinId,
    pub symbol: String,
    pub name: String,
    /// Official thumbnail URL, as published by the source.
    pub image: Option<String>,
    /// Local path of the logo, filled in after going through the icon cache.
    pub icon: Option<String>,
}

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
