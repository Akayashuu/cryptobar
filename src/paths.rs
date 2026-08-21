//! Résolution des répertoires XDG. Les variables `CRYPTOBAR_*_DIR` priment,
//! ce qui rend les tests d'intégration hermétiques.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use etcetera::BaseStrategy as _;

const APP: &str = "cryptobar";

fn strategy() -> Result<etcetera::base_strategy::Xdg> {
    etcetera::base_strategy::Xdg::new().context("impossible de résoudre les répertoires XDG")
}

fn overridden(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Répertoire de configuration : `$CRYPTOBAR_CONFIG_DIR` ou `$XDG_CONFIG_HOME/cryptobar`.
///
/// # Errors
/// Échoue si le répertoire personnel est introuvable.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = overridden("CRYPTOBAR_CONFIG_DIR") {
        return Ok(dir);
    }
    Ok(strategy()?.config_dir().join(APP))
}

/// Chemin du fichier de configuration.
///
/// # Errors
/// Voir [`config_dir`].
pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Répertoire de cache : `$CRYPTOBAR_CACHE_DIR` ou `$XDG_CACHE_HOME/cryptobar`.
///
/// # Errors
/// Échoue si le répertoire personnel est introuvable.
pub fn cache_dir() -> Result<PathBuf> {
    if let Some(dir) = overridden("CRYPTOBAR_CACHE_DIR") {
        return Ok(dir);
    }
    Ok(strategy()?.cache_dir().join(APP))
}

/// Répertoire d'état volatil : `$CRYPTOBAR_RUNTIME_DIR`, `$XDG_RUNTIME_DIR/cryptobar`,
/// ou le répertoire de cache en dernier recours (systèmes sans répertoire d'exécution).
///
/// # Errors
/// Échoue si aucun répertoire ne peut être déterminé.
pub fn runtime_dir() -> Result<PathBuf> {
    if let Some(dir) = overridden("CRYPTOBAR_RUNTIME_DIR") {
        return Ok(dir);
    }
    match strategy()?.runtime_dir() {
        Some(dir) => Ok(dir.join(APP)),
        None => cache_dir(),
    }
}

/// Crée le répertoire s'il n'existe pas, avec des permissions restreintes sous Unix.
///
/// # Errors
/// Remonte les erreurs d'E/S de la création.
pub fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        // Un échec ici (montage exotique, système de fichiers sans permissions)
        // ne doit pas empêcher le fonctionnement.
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}
