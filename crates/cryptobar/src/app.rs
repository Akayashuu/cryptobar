//! Runtime context: configuration, paths, cached source and state.
//!
//! Every command receives this struct; it is the only place that wires up the
//! I/O, which keeps the rest of the code testable without a network.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::alerts::AlertError;
use crate::config::{Config, ConfigError};
use crate::config_edit::Editor;
use crate::model::Quote;
use crate::onchain::Watcher;
use crate::paths::{self, PathsError};
use crate::source::cache::CachedSource;
use crate::source::coingecko::CoinGecko;
use crate::source::icons::{self, IconError};
use crate::state::{LoadedState, State, StateError};
use crate::waybar_signal::{self, SignalError};

const MANUAL_HOLD: u64 = 300;

/// Failures of the application context.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The XDG directories could not be resolved.
    #[error("could not resolve the directories")]
    Paths(#[from] PathsError),
    /// The configuration file could not be loaded.
    #[error("could not load the configuration")]
    Config(#[from] ConfigError),
    /// The runtime state could not be read or written.
    #[error("could not access the runtime state")]
    State(#[from] StateError),
    /// A cryptocurrency logo could not be downloaded.
    #[error("could not download a logo")]
    Icon(#[from] IconError),
    /// Waybar could not be signalled.
    #[error("could not signal Waybar")]
    Signal(#[from] SignalError),
    /// The desktop notification could not be sent.
    #[error("could not send the desktop notification")]
    Alert(#[from] AlertError),
}

/// Runtime context shared by every command.
#[derive(Debug)]
pub struct App {
    /// The configuration as loaded at start-up.
    pub config: Config,
    /// The market data source, behind the disk cache.
    pub source: CachedSource<CoinGecko>,
    config_path: PathBuf,
    cache_dir: PathBuf,
    state_path: PathBuf,
}

impl App {
    /// Builds the context from the configuration.
    ///
    /// # Errors
    /// Fails when the configuration is unreadable or invalid, or when the XDG
    /// directories cannot be resolved.
    pub fn new(config_path: Option<PathBuf>) -> Result<Self, AppError> {
        let config_path = if let Some(path) = config_path {
            path
        } else {
            let path = paths::config_file()?;
            if !path.exists() {
                Config::write_default(&path, false)?;
            }
            path
        };
        let config = Config::load_from(&config_path)?;
        let cache_dir = paths::cache_dir()?;
        let ttl = Duration::from_secs(config.refresh_interval);
        let source = CachedSource::new(CoinGecko::new(), cache_dir.clone(), ttl);
        Ok(Self {
            config,
            source,
            config_path,
            cache_dir,
            state_path: State::default_path()?,
        })
    }

    /// Path of the configuration file actually in use.
    #[must_use]
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// On-chain balance reader, with its cache in the cache directory.
    #[must_use]
    pub fn watcher(&self) -> Watcher {
        Watcher::new(&self.cache_dir)
    }

    /// Editor for the current configuration file.
    #[must_use]
    pub fn editor(&self) -> Editor {
        Editor::new(self.config_path.clone())
    }

    /// Rebuilds the context from the configuration file.
    ///
    /// # Errors
    /// Fails when the configuration has become invalid.
    pub fn reload(&self) -> Result<Self, AppError> {
        Self::new(Some(self.config_path.clone()))
    }

    /// Attaches the logos already present in the cache. No network access:
    /// safe to call from a bar tick.
    pub fn attach_icons(&self, quotes: &mut [Quote]) {
        icons::attach(&self.cache_dir, quotes);
    }

    /// Local path of a cryptocurrency logo, downloaded when needed.
    ///
    /// The agent passed in is reused across calls: a whole list costs a single
    /// connection.
    ///
    /// # Errors
    /// See [`IconError`]: a failed download or store, or an image that is not a PNG.
    pub fn icon_for(
        &self,
        id: &str,
        url: Option<&str>,
        agent: &mut Option<ureq::Agent>,
    ) -> Result<Option<String>, IconError> {
        icons::ensure(&self.cache_dir, id, url, agent)
    }

    /// Attaches the logos, downloading the missing ones. Reserved for paths
    /// where a few hundred milliseconds go unnoticed. The failed downloads are
    /// returned, the quotes keep their badge for them.
    #[must_use]
    pub fn fetch_icons(&self, quotes: &mut [Quote]) -> Vec<IconError> {
        icons::fetch_missing(&self.cache_dir, quotes)
    }

    /// Reads the state. A corrupt file is replaced by the default state right
    /// away, so the next command starts clean; the corruption is returned in
    /// [`LoadedState::reset`] for the caller to report.
    ///
    /// # Errors
    /// Fails with [`StateError::Read`] when the file cannot be read, and with
    /// [`StateError::Write`] when a corrupt file cannot be replaced.
    pub fn state(&self) -> Result<LoadedState, StateError> {
        match State::load(&self.state_path) {
            Ok(state) => Ok(LoadedState { state, reset: None }),
            Err(corrupt @ StateError::Corrupt { .. }) => {
                let state = State::default();
                self.save_state(&state)?;
                Ok(LoadedState {
                    state,
                    reset: Some(corrupt),
                })
            }
            Err(failure) => Err(failure),
        }
    }

    /// Persists the cycling state.
    ///
    /// # Errors
    /// Propagates the write errors.
    pub fn save_state(&self, state: &State) -> Result<(), StateError> {
        state.save(&self.state_path)
    }

    /// Index of the cryptocurrency to display for the given state.
    #[must_use]
    pub fn index_of(&self, state: &State) -> usize {
        state.effective_index(
            self.config.coins.len(),
            self.config.cycle_interval,
            MANUAL_HOLD,
        )
    }

    /// Wakes Waybar up when a signal number is configured. Waybar not running is
    /// the normal state of a desktop without it, so it is not a failure.
    ///
    /// # Errors
    /// Fails when the signal cannot be built or delivered.
    pub fn notify(&self) -> Result<(), SignalError> {
        match waybar_signal::notify_waybar(self.config.waybar_signal) {
            Ok(_) | Err(SignalError::NotRunning) => Ok(()),
            Err(failure) => Err(failure),
        }
    }
}
