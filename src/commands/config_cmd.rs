//! Sous-commande `config` : initialisation, chemin et validation.
//!
//! Elle doit fonctionner sans contexte applicatif : c'est la commande qu'on
//! lance justement quand la configuration est absente ou cassée.

use std::path::PathBuf;

use anyhow::{Context as _, Result};

use crate::cli::ConfigAction;
use crate::config::Config;

/// Exécute l'action demandée sur le fichier de configuration.
///
/// # Errors
/// Échoue si le chemin ne peut être résolu, si `init` refuse d'écraser un
/// fichier existant, ou si `check` trouve une configuration invalide.
pub fn run(action: &ConfigAction, override_path: Option<PathBuf>) -> Result<()> {
    let path = match override_path {
        Some(path) => path,
        None => crate::paths::config_file().context("résolution du chemin de configuration")?,
    };
    match action {
        ConfigAction::Path => println!("{}", path.display()),
        ConfigAction::Init { force } => {
            Config::write_default(&path, *force)?;
            println!("configuration écrite dans {}", path.display());
        }
        ConfigAction::Check => {
            let config = Config::load_from(&path)?;
            println!(
                "configuration valide : {} ({} cryptomonnaies, rafraîchissement {} s)",
                path.display(),
                config.coins.len(),
                config.refresh_interval,
            );
        }
    }
    Ok(())
}
