//! XDG directory resolution. The `CRYPTOBAR_*_DIR` variables take precedence,
//! which keeps the integration tests hermetic.

use std::env;
use std::fs::{self, Permissions};
use std::io;
use std::path::{Path, PathBuf};

use etcetera::BaseStrategy as _;
use etcetera::base_strategy::Xdg;
use rustix::process;

const APP: &str = "cryptobar";

/// Failures to locate the directories of the user.
#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    /// The home directory is unknown.
    #[error("could not resolve the XDG directories")]
    Home(#[from] etcetera::HomeDirError),
    /// The directory could not be created or restricted to its owner.
    #[error("could not prepare the directory {}", .path.display())]
    Directory {
        /// Directory that could not be prepared.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
}

fn strategy() -> Result<Xdg, PathsError> {
    Ok(Xdg::new()?)
}

fn overridden(var: &str) -> Option<PathBuf> {
    env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Configuration directory: `$CRYPTOBAR_CONFIG_DIR` or `$XDG_CONFIG_HOME/cryptobar`.
///
/// # Errors
/// Fails when the home directory cannot be found.
pub fn config_dir() -> Result<PathBuf, PathsError> {
    if let Some(dir) = overridden("CRYPTOBAR_CONFIG_DIR") {
        return Ok(dir);
    }
    Ok(strategy()?.config_dir().join(APP))
}

/// Path of the configuration file.
///
/// # Errors
/// See [`config_dir`].
pub fn config_file() -> Result<PathBuf, PathsError> {
    Ok(config_dir()?.join("config.toml"))
}

/// Cache directory: `$CRYPTOBAR_CACHE_DIR` or `$XDG_CACHE_HOME/cryptobar`.
///
/// # Errors
/// Fails when the home directory cannot be found.
pub fn cache_dir() -> Result<PathBuf, PathsError> {
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
pub fn runtime_dir() -> Result<PathBuf, PathsError> {
    if let Some(dir) = overridden("CRYPTOBAR_RUNTIME_DIR") {
        return Ok(dir);
    }
    match strategy()?.runtime_dir() {
        Some(dir) => Ok(dir.join(APP)),
        None => cache_dir(),
    }
}

/// Creates the directory when missing, then restricts it to its owner on Unix.
///
/// A directory owned by another user is left as it is: it is not ours to restrict.
///
/// # Errors
/// Fails when the directory cannot be created, or when it belongs to the current
/// user and cannot be restricted to it.
pub fn ensure_dir(dir: &Path) -> Result<(), PathsError> {
    prepare_dir(dir).map_err(|source| PathsError::Directory {
        path: dir.to_path_buf(),
        source,
    })
}

fn prepare_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    restrict_to_owner(dir)
}

#[cfg(unix)]
fn restrict_to_owner(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    if fs::metadata(dir)?.uid() != process::geteuid().as_raw() {
        return Ok(());
    }
    fs::set_permissions(dir, Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn restrict_to_owner(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// Home directory, where the window writes its exports.
///
/// `$CRYPTOBAR_EXPORT_DIR` takes precedence, which keeps the tests hermetic and
/// allows redirecting the files somewhere other than `~`.
///
/// # Errors
/// Fails when the home directory cannot be found.
pub fn export_dir() -> Result<PathBuf, PathsError> {
    if let Some(dir) = overridden("CRYPTOBAR_EXPORT_DIR") {
        return Ok(dir);
    }
    Ok(strategy()?.home_dir().to_path_buf())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    #[test]
    fn a_created_directory_is_restricted_to_its_owner() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache");
        ensure_dir(&target).unwrap();
        let mode = fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn a_path_that_is_a_file_is_reported_as_a_directory_failure() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("occupied");
        fs::write(&file, b"not a directory").unwrap();
        assert!(matches!(
            ensure_dir(&file),
            Err(PathsError::Directory { .. })
        ));
    }
}
