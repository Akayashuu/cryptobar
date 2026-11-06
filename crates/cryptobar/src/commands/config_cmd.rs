//! `config` subcommand: initialisation, path and validation.
//!
//! It has to work without any application context: it is the very command one
//! runs when the configuration is missing or broken.

use std::path::PathBuf;

use anyhow::{Context as _, Result};

use crate::cli::ConfigAction;
use crate::config::Config;

/// Runs the requested action on the configuration file.
///
/// # Errors
/// Fails when the path cannot be resolved, when `init` refuses to overwrite an
/// existing file, or when `check` finds an invalid configuration.
pub fn run(action: &ConfigAction, override_path: Option<PathBuf>) -> Result<()> {
    let path = match override_path {
        Some(path) => path,
        None => crate::paths::config_file().context("resolving the configuration path")?,
    };
    match action {
        ConfigAction::Path => println!("{}", path.display()),
        ConfigAction::Init { force } => {
            Config::write_default(&path, *force)?;
            println!("configuration written to {}", path.display());
        }
        ConfigAction::Check => {
            let config = Config::load_from(&path)?;
            println!(
                "valid configuration: {} ({} cryptocurrencies, refresh {} s)",
                path.display(),
                config.coins.len(),
                config.refresh_interval,
            );
        }
    }
    Ok(())
}
