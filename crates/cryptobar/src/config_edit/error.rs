//! Failures of the configuration editor: the file itself, the shape of its
//! lists, and the validation of the value written.

use std::io;
use std::path::PathBuf;

use crate::config::ConfigError;
use crate::model::CoinId;
use crate::onchain::OnchainError;

/// Failures of the configuration editor.
#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("reading {}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("malformed TOML: {}", .path.display())]
    Malformed {
        path: PathBuf,
        #[source]
        source: toml_edit::TomlError,
    },
    #[error("creating {}", .path.display())]
    Directory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("writing {}", .path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("replacing {}", .path.display())]
    Replace {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("{0} is already watched")]
    AlreadyWatched(CoinId),
    #[error("{0} is not watched")]
    NotWatched(String),
    #[error("at least one cryptocurrency has to stay watched")]
    LastCoin,
    #[error("`coins` is not an array")]
    CoinsNotArray,
    #[error("an alert has to define at least one condition")]
    AlertWithoutCondition,
    #[error("`alerts` is not an array of tables")]
    AlertsNotArray,
    #[error("no alert at index {0}")]
    NoAlert(usize),
    #[error(transparent)]
    Wallet(#[from] OnchainError),
    #[error("this address is already watched")]
    WalletAlreadyWatched,
    #[error("`wallets` is not an array of tables")]
    WalletsNotArray,
    #[error("no wallet at index {0}")]
    NoWallet(usize),
}
