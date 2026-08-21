//! Runtime context: configuration, paths, cached source and state.
//!
//! Every command receives this struct; it is the only place that wires up the
//! I/O, which keeps the rest of the code testable without a network.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use crate::config::Config;
use crate::source::cache::CachedSource;
use crate::source::coingecko::CoinGecko;
use crate::state::State;

/// How long a manual selection suspends the rotation.
const MANUAL_HOLD: u64 = 300;

pub struct App {
    pub config: Config,
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
    pub fn new(config_path: Option<PathBuf>) -> Result<Self> {
        let config_path = if let Some(path) = config_path {
            path
        } else {
            // `Config::load` creates the file on first run: resolve the path
            // first so it can be edited afterwards.
            let path = crate::paths::config_file()?;
            if !path.exists() {
                Config::write_default(&path, false)?;
            }
            path
        };
        let config = Config::load_from(&config_path)?;
        let cache_dir = crate::paths::cache_dir()?;
        // The cache TTL is the refresh interval: the bar can tick faster
        // without ever triggering an extra request.
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
    pub fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    /// On-chain balance reader, with its cache in the cache directory.
    #[must_use]
    pub fn watcher(&self) -> crate::onchain::Watcher {
        crate::onchain::Watcher::new(&self.cache_dir)
    }

    /// Editor for the current configuration file.
    #[must_use]
    pub fn editor(&self) -> crate::config_edit::Editor {
        crate::config_edit::Editor::new(self.config_path.clone())
    }

    /// Rebuilds the context from the configuration file.
    ///
    /// # Errors
    /// Fails when the configuration has become invalid.
    pub fn reload(&self) -> Result<Self> {
        Self::new(Some(self.config_path.clone()))
    }

    /// Attaches the logos already present in the cache. No network access:
    /// safe to call from a bar tick.
    pub fn attach_icons(&self, quotes: &mut [crate::model::Quote]) {
        crate::source::icons::attach(&self.cache_dir, quotes);
    }

    /// Local path of a cryptocurrency logo, downloaded when needed.
    ///
    /// The agent passed in is reused across calls: a whole list costs a single
    /// connection.
    #[must_use]
    pub fn icon_for(
        &self,
        id: &str,
        url: Option<&str>,
        agent: &mut Option<ureq::Agent>,
    ) -> Option<String> {
        crate::source::icons::ensure(&self.cache_dir, id, url, agent)
    }

    /// Attaches the logos, downloading the missing ones. Reserved for paths
    /// where a few hundred milliseconds go unnoticed.
    pub fn fetch_icons(&self, quotes: &mut [crate::model::Quote]) {
        crate::source::icons::fetch_missing(&self.cache_dir, quotes);
    }

    #[must_use]
    pub fn state(&self) -> State {
        State::load(&self.state_path)
    }

    /// Persists the cycling state.
    ///
    /// # Errors
    /// Propagates the write errors.
    pub fn save_state(&self, state: &State) -> Result<()> {
        state.save(&self.state_path)?;
        Ok(())
    }

    /// Index of the cryptocurrency to display right now.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.state().effective_index(
            self.config.coins.len(),
            self.config.cycle_interval,
            MANUAL_HOLD,
        )
    }

    /// Wakes Waybar up when a signal number is configured. A failure is never
    /// fatal: the user may simply not have Waybar running.
    pub fn notify(&self) {
        let _ = crate::waybar_signal::notify_waybar(self.config.waybar_signal);
    }
}
