//! Failures of the watch-only layer: address shape, balance decoding, network
//! reads and the balance cache.

use std::io;
use std::num::ParseIntError;

use crate::paths::PathsError;

/// Failures of the watch-only layer.
#[derive(Debug, thiserror::Error)]
pub enum OnchainError {
    /// The address is empty.
    #[error("empty address")]
    EmptyAddress,
    /// The address contains whitespace.
    #[error("a public address contains no space")]
    ContainsSpace,
    /// The value is not a public address: it looks like a secret.
    #[error("this value is not a public address")]
    NotPublic,
    /// The address does not decode as bech32.
    #[error("malformed bech32 address")]
    MalformedBech32,
    /// The address matches no Bitcoin format.
    #[error("unrecognised Bitcoin address")]
    UnrecognisedBitcoin,
    /// The address is not a `0x` followed by 40 hexadecimal digits.
    #[error("expected an Ethereum address: 0x followed by 40 characters")]
    MalformedEthereum,
    /// The address matches no Solana format.
    #[error("unrecognised Solana address")]
    UnrecognisedSolana,
    /// The chain is not supported.
    #[error("unknown chain: {0}")]
    UnknownChain(String),
    /// A balance response is not valid JSON.
    #[error("unreadable response")]
    Json(#[from] serde_json::Error),
    /// A balance response holds a number that is not valid hexadecimal.
    #[error("unreadable balance")]
    Hex(#[from] ParseIntError),
    /// A balance response holds no balance.
    #[error("response without a balance")]
    MissingBalance,
    /// The chain endpoint could not be reached.
    #[error("network error")]
    Http(#[from] ureq::Error),
    /// The balance cache could not be read or written.
    #[error("balance cache")]
    Cache(#[from] io::Error),
    /// The balance cache directory could not be prepared.
    #[error(transparent)]
    Directory(#[from] PathsError),
}
