//! Read-only wallet watching.
//!
//! The principle is "watch-only": you paste a **public address**, cryptobar
//! reads the balance from the chain and values it. No private key, no recovery
//! phrase, no signature, [`Wallet::validate`] explicitly rejects anything that
//! looks like one. The program cannot spend, only look.
//!
//! Three chains, three public endpoints with no API key: mempool.space for
//! Bitcoin, a public Ethereum RPC, the Solana mainnet RPC. Balances are cached
//! on disk, so the bar never opens a connection: it re-reads a file.

mod address;
mod balance;
mod chain;
mod decode;
mod error;
mod wallet;
mod watcher;

pub use address::shorten;
pub use balance::{Balance, totals};
pub use chain::Chain;
pub use error::OnchainError;
pub use wallet::Wallet;
pub use watcher::{Refresh, Watcher};
