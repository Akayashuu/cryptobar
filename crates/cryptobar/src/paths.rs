//! XDG directory resolution. The `CRYPTOBAR_*_DIR` variables take precedence,
//! which keeps the integration tests hermetic.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use etcetera::BaseStrategy as _;

const APP: &str = "cryptobar";

fn strategy() -> Result<etcetera::base_strategy::Xdg> {
    etcetera::base_strategy::Xdg::new().context("could not resolve the XDG directories")
}

fn overridden(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Configuration directory: `$CRYPTOBAR_CONFIG_DIR` or `$XDG_CONFIG_HOME/cryptobar`.
///
/// # Errors
/// Fails when the home directory cannot be found.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = overridden("CRYPTOBAR_CONFIG_DIR") {
        return Ok(dir);
    }
    Ok(strategy()?.config_dir().join(APP))
}

/// Path of the configuration file.
///
/// # Errors
/// See [`config_dir`].
pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Cache directory: `$CRYPTOBAR_CACHE_DIR` or `$XDG_CACHE_HOME/cryptobar`.
///
/// # Errors
/// Fails when the home directory cannot be found.
pub fn cache_dir() -> Result<PathBuf> {
    if let Some(dir) = overridden("CRYPTOBAR_CACHE_DIR") {
        return Ok(dir);
    }
    Ok(strategy()?.cache_dir().join(APP))
}

/// Volatile state directory: `$CRYPTOBAR_RUNTIME_DIR`, `$XDG_RUNTIME_DIR/cryptobar`,
/// or the cache directory as a last resort (systems without a runtime directory).
///
/// # Errors
/// Fails when no directory can be determined.
pub fn runtime_dir() -> Result<PathBuf> {
    if let Some(dir) = overridden("CRYPTOBAR_RUNTIME_DIR") {
        return Ok(dir);
    }
    match strategy()?.runtime_dir() {
        Some(dir) => Ok(dir.join(APP)),
        None => cache_dir(),
    }
}

/// Creates the directory when missing, with restricted permissions on Unix.
///
/// # Errors
/// Propagates the I/O errors of the creation.
pub fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        // A failure here (exotic mount, filesystem without permissions) must
        // not get in the way.
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Home directory, where the window writes its exports.
///
/// `$CRYPTOBAR_EXPORT_DIR` takes precedence, which keeps the tests hermetic and
/// allows redirecting the files somewhere other than `~`.
///
/// # Errors
/// Fails when the home directory cannot be found.
pub fn export_dir() -> Result<PathBuf> {
    if let Some(dir) = overridden("CRYPTOBAR_EXPORT_DIR") {
        return Ok(dir);
    }
    Ok(strategy()?.home_dir().to_path_buf())
}
