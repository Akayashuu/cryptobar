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
    config_path: PathBuf,
    cache_dir: PathBuf,
    state_path: PathBuf,
}

impl App {
    /// Assemble le contexte à partir de la configuration.
    ///
    /// # Errors
    /// Échoue si la configuration est illisible ou invalide, ou si les
    /// répertoires XDG ne peuvent pas être résolus.
    pub fn new(config_path: Option<PathBuf>) -> Result<Self> {
        let config_path = if let Some(path) = config_path {
            path
        } else {
            // `Config::load` crée le fichier au premier lancement : on résout
            // le chemin d'abord, pour pouvoir l'éditer ensuite.
            let path = crate::paths::config_file()?;
            if !path.exists() {
                Config::write_default(&path, false)?;
            }
            path
        };
        let config = Config::load_from(&config_path)?;
        let cache_dir = crate::paths::cache_dir()?;
        // Le TTL du cache est l'intervalle de rafraîchissement : Waybar peut
        // ticker plus vite sans jamais générer de requête supplémentaire.
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

    /// Chemin du fichier de configuration effectivement utilisé.
    #[must_use]
    pub fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    /// Lecteur de soldes on-chain, avec son cache dans le répertoire de cache.
    #[must_use]
    pub fn watcher(&self) -> crate::onchain::Watcher {
        crate::onchain::Watcher::new(&self.cache_dir)
    }

    /// Éditeur du fichier de configuration courant.
    #[must_use]
    pub fn editor(&self) -> crate::config_edit::Editor {
        crate::config_edit::Editor::new(self.config_path.clone())
    }

    /// Reconstruit le contexte à partir du fichier de configuration.
    ///
    /// # Errors
    /// Échoue si la configuration est devenue invalide.
    pub fn reload(&self) -> Result<Self> {
        Self::new(Some(self.config_path.clone()))
    }

    /// Rattache les logos déjà présents dans le cache. Aucun accès réseau :
    /// utilisable dans un tick de barre.
    pub fn attach_icons(&self, quotes: &mut [crate::model::Quote]) {
        crate::source::icons::attach(&self.cache_dir, quotes);
    }

    /// Rattache les logos, en téléchargeant ceux qui manquent. À réserver aux
    /// chemins où quelques centaines de millisecondes ne se voient pas.
    pub fn fetch_icons(&self, quotes: &mut [crate::model::Quote]) {
        crate::source::icons::fetch_missing(&self.cache_dir, quotes);
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
