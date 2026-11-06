use serde::{Deserialize, Serialize};

use crate::model::CoinId;

use super::error::OnchainError;

/// A watched chain. The list is short on purpose: every entry needs a public,
/// key-free endpoint and a clean balance decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    /// The Bitcoin network.
    Bitcoin,
    /// The Ethereum network.
    Ethereum,
    /// The Solana network.
    Solana,
}

impl Chain {
    /// Every chain, in display order.
    pub const ALL: [Self; 3] = [Self::Bitcoin, Self::Ethereum, Self::Solana];

    /// Lowercase name of the chain, as written in the configuration.
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
    type Err = OnchainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "bitcoin" | "btc" => Ok(Self::Bitcoin),
            "ethereum" | "eth" => Ok(Self::Ethereum),
            "solana" | "sol" => Ok(Self::Solana),
            other => Err(OnchainError::UnknownChain(other.to_owned())),
        }
    }
}
