//! Subcommand implementations.

use std::error::Error;
use std::io::Write;

use crate::app::AppError;
use crate::config::ConfigError;
use crate::config_edit::EditError;
use crate::model::ModelError;
use crate::output::Output;
use crate::paths::PathsError;
use crate::report::Report;
use crate::source::SourceError;
use crate::state::StateError;
use crate::waybar_signal::SignalError;

pub mod bar;
pub mod chart;
pub mod coins;
pub mod config_cmd;
pub mod nav;
pub mod popup;
pub mod wallet;

/// Failures of the subcommands.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("could not build the context")]
    App(#[from] AppError),
    #[error("could not access the runtime state")]
    State(#[from] StateError),
    #[error("could not resolve the directories")]
    Paths(#[from] PathsError),
    #[error("could not load the configuration")]
    Config(#[from] ConfigError),
    #[error("could not change the configuration")]
    Edit(#[from] EditError),
    #[error("the cryptocurrency source failed")]
    Source(#[from] SourceError),
    #[error("could not fetch the series of {id}")]
    Series {
        id: String,
        #[source]
        source: SourceError,
    },
    #[error("could not search the cryptocurrencies")]
    Search(#[source] SourceError),
    #[error("could not notify Waybar")]
    Signal(#[from] SignalError),
    #[error("the arguments are invalid")]
    Input(#[from] ModelError),
    #[error("could not render the module")]
    Render(#[from] serde_json::Error),
    #[error("could not write the output")]
    Write(#[source] std::io::Error),
    #[error("could not locate the running executable")]
    Executable(#[source] std::io::Error),
    #[error("could not start {program}")]
    Launch {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("no cryptocurrency is configured")]
    NoCoin,
    #[error("no terminal emulator is configured")]
    NoTerminal,
    #[cfg(feature = "popup")]
    #[error("the popup window failed")]
    Popup(#[from] crate::ui::popup::PopupError),
    #[cfg(not(feature = "popup"))]
    #[error("this binary was compiled without the `popup` feature")]
    PopupUnavailable,
}

/// Writes one line of result to stdout.
///
/// # Errors
/// Fails when stdout cannot be written.
pub fn print<Out: Write, Err: Write>(
    output: &mut Output<Out, Err>,
    text: impl std::fmt::Display,
) -> Result<(), CommandError> {
    output.line(text).map_err(CommandError::Write)
}

/// Writes a text as it is, the newline included or not.
///
/// # Errors
/// Fails when stdout cannot be written.
pub fn print_text<Out: Write, Err: Write>(
    output: &mut Output<Out, Err>,
    text: impl std::fmt::Display,
) -> Result<(), CommandError> {
    output.text(text).map_err(CommandError::Write)
}

/// Writes one failure to stderr, as `cryptobar: outer: cause: root`.
///
/// # Errors
/// Fails when stderr cannot be written.
pub fn warn<Out: Write, Err: Write>(
    output: &mut Output<Out, Err>,
    failure: &dyn Error,
) -> Result<(), CommandError> {
    output
        .warning(format_args!("cryptobar: {}", Report(failure)))
        .map_err(CommandError::Write)
}

/// Writes each failure of a batch to stderr, in order.
///
/// # Errors
/// Fails when stderr cannot be written.
pub fn warn_each<Out: Write, Err: Write, E: Error>(
    output: &mut Output<Out, Err>,
    failures: &[E],
) -> Result<(), CommandError> {
    failures
        .iter()
        .try_for_each(|failure| warn(output, failure))
}
