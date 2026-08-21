//! Accès aux données de marché. Le réseau est toujours derrière le trait
//! [`PriceSource`], ce qui permet de tester tout le reste sans sortir de la machine.

pub mod cache;
pub mod coingecko;
pub mod icons;

use crate::model::{CoinId, Currency, Quote, Range, Series};

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("erreur réseau : {0}")]
    Http(String),
    #[error("quota de l'API dépassé")]
    RateLimited { retry_after: Option<u64> },
    #[error("réponse illisible : {0}")]
    Decode(String),
    #[error("réponse inattendue : {0}")]
    Payload(String),
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: CoinId,
    pub symbol: String,
    pub name: String,
}

pub trait PriceSource {
    /// Cotations de toutes les cryptomonnaies demandées, en une seule requête.
    ///
    /// # Errors
    /// Voir [`SourceError`].
    fn markets(&self, ids: &[CoinId], vs: &Currency) -> Result<Vec<Quote>, SourceError>;

    /// Série de prix d'une cryptomonnaie sur une plage donnée.
    ///
    /// # Errors
    /// Voir [`SourceError`].
    fn series(&self, id: &CoinId, range: Range, vs: &Currency) -> Result<Series, SourceError>;

    /// Recherche d'identifiants par terme libre.
    ///
    /// # Errors
    /// Voir [`SourceError`].
    fn search(&self, term: &str) -> Result<Vec<SearchHit>, SourceError>;
}
