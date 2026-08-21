//! Contexte d'exécution : configuration, chemins, source cachée et état.
//!
//! Toutes les commandes reçoivent cette structure ; c'est le seul endroit qui
//! assemble les E/S, ce qui garde le reste du code testable sans réseau.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use crate::config::Config;
use crate::source::cache::CachedSource;
use crate::source::coingecko::CoinGecko;
use crate::state::State;

/// Durée pendant laquelle une sélection manuelle suspend la rotation.
const MANUAL_HOLD: u64 = 300;

pub struct App {
    pub config: Config,
    pub source: CachedSource<CoinGecko>,
    state_path: PathBuf,
}

impl App {
    /// Assemble le contexte à partir de la configuration.
    ///
    /// # Errors
    /// Échoue si la configuration est illisible ou invalide, ou si les
    /// répertoires XDG ne peuvent pas être résolus.
    pub fn new(config_path: Option<PathBuf>) -> Result<Self> {
        let config = match config_path {
            Some(path) => Config::load_from(&path)?,
            None => Config::load()?,
        };
        let cache_dir = crate::paths::cache_dir()?;
        // Le TTL du cache est l'intervalle de rafraîchissement : Waybar peut
        // ticker plus vite sans jamais générer de requête supplémentaire.
        let ttl = Duration::from_secs(config.refresh_interval);
        let source = CachedSource::new(CoinGecko::new(), cache_dir, ttl);
        Ok(Self {
            config,
            source,
            state_path: State::default_path()?,
        })
    }

    #[must_use]
    pub fn state(&self) -> State {
        State::load(&self.state_path)
    }

    /// Persiste l'état de cyclage.
    ///
    /// # Errors
    /// Remonte les erreurs d'écriture.
    pub fn save_state(&self, state: &State) -> Result<()> {
        state.save(&self.state_path)?;
        Ok(())
    }

    /// Indice de la cryptomonnaie à afficher maintenant.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.state().effective_index(
            self.config.coins.len(),
            self.config.cycle_interval,
            MANUAL_HOLD,
        )
    }

    /// Réveille Waybar si un numéro de signal est configuré. Un échec n'est
    /// jamais fatal : l'utilisateur peut simplement ne pas avoir Waybar ouvert.
    pub fn notify(&self) {
        let _ = crate::waybar_signal::notify_waybar(self.config.waybar_signal);
    }
}
