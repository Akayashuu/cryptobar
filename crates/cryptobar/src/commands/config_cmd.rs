//! `config` subcommand: initialisation, path and validation.
//!
//! It has to work without any application context: it is the very command one
//! runs when the configuration is missing or broken.

use std::io::Write;
use std::path::PathBuf;

use super::{CommandError, print};
use crate::cli::ConfigAction;
use crate::config::Config;
use crate::output::Output;

/// Runs the requested action on the configuration file.
///
/// # Errors
/// Fails when the path cannot be resolved, when `init` refuses to overwrite an
/// existing file, when `check` finds an invalid configuration, or when stdout
/// cannot be written.
pub fn run<Out: Write, Err: Write>(
    action: &ConfigAction,
    override_path: Option<PathBuf>,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    let path = match override_path {
        Some(path) => path,
        None => crate::paths::config_file()?,
    };
    match action {
        ConfigAction::Path => print(output, path.display()),
        ConfigAction::Init { force } => {
            Config::write_default(&path, *force)?;
            print(
                output,
                format_args!("configuration written to {}", path.display()),
            )
        }
        ConfigAction::Check => {
            let config = Config::load_from(&path)?;
            print(
                output,
                format_args!(
                    "valid configuration: {} ({} cryptocurrencies, refresh {} s)",
                    path.display(),
                    config.coins.len(),
                    config.refresh_interval,
                ),
            )
        }
    }
}
