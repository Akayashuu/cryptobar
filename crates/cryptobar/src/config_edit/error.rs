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
    /// The configuration file could not be read.
    #[error("reading {}", .path.display())]
    Read {
        /// File that could not be read.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The configuration file is not valid TOML.
    #[error("malformed TOML: {}", .path.display())]
    Malformed {
        /// File that is not valid TOML.
        path: PathBuf,
        /// The TOML parse error.
        #[source]
        source: toml_edit::TomlError,
    },
    /// The directory of the configuration file could not be created.
    #[error("creating {}", .path.display())]
    Directory {
        /// Directory that could not be created.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The configuration file could not be written.
    #[error("writing {}", .path.display())]
    Write {
        /// File that could not be written.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The configuration file could not be replaced by its new version.
    #[error("replacing {}", .path.display())]
    Replace {
        /// File that could not be replaced.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The configuration could not be loaded.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The cryptocurrency is already in `coins`.
    #[error("{0} is already watched")]
    AlreadyWatched(CoinId),
    /// The cryptocurrency is not in `coins`.
    #[error("{0} is not watched")]
    NotWatched(String),
    /// The last cryptocurrency cannot be removed: `coins` must stay non-empty.
    #[error("at least one cryptocurrency has to stay watched")]
    LastCoin,
    /// `coins` exists but is not an array.
    #[error("`coins` is not an array")]
    CoinsNotArray,
    /// The alert would define no condition.
    #[error("an alert has to define at least one condition")]
    AlertWithoutCondition,
    /// `alerts` exists but is not an array of tables.
    #[error("`alerts` is not an array of tables")]
    AlertsNotArray,
    /// No alert sits at this index.
    #[error("no alert at index {0}")]
    NoAlert(usize),
    /// The wallet is invalid.
    #[error(transparent)]
    Wallet(#[from] OnchainError),
    /// The address is already watched.
    #[error("this address is already watched")]
    WalletAlreadyWatched,
    /// `wallets` exists but is not an array of tables.
    #[error("`wallets` is not an array of tables")]
    WalletsNotArray,
    /// No wallet sits at this index.
    #[error("no wallet at index {0}")]
    NoWallet(usize),
}
