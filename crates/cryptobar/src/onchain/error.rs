//! Failures of the watch-only layer: address shape, balance decoding, network
//! reads and the balance cache.

use std::io;
use std::num::ParseIntError;

/// Failures of the watch-only layer.
#[derive(Debug, thiserror::Error)]
pub enum OnchainError {
    #[error("empty address")]
    EmptyAddress,
    #[error("a public address contains no space")]
    ContainsSpace,
    #[error("this value is not a public address")]
    NotPublic,
    #[error("malformed bech32 address")]
    MalformedBech32,
    #[error("unrecognised Bitcoin address")]
    UnrecognisedBitcoin,
    #[error("expected an Ethereum address: 0x followed by 40 characters")]
    MalformedEthereum,
    #[error("unrecognised Solana address")]
    UnrecognisedSolana,
    #[error("unknown chain: {0}")]
    UnknownChain(String),
    #[error("unreadable response")]
    Json(#[from] serde_json::Error),
    #[error("unreadable balance")]
    Hex(#[from] ParseIntError),
    #[error("response without a balance")]
    MissingBalance,
    #[error("network error")]
    Http(#[from] ureq::Error),
    #[error("balance cache")]
    Cache(#[from] io::Error),
}
