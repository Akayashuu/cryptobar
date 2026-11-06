//! Subcommand implementations.

use std::collections::HashSet;
use std::error::Error;
use std::fmt::Display;
use std::hash::Hash;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::app::{App, AppError};
use crate::config::ConfigError;
use crate::config_edit::EditError;
use crate::model::ModelError;
use crate::output::Output;
use crate::paths::PathsError;
use crate::report::Report;
use crate::source::SourceError;
use crate::state::{LoadedState, StateError};
#[cfg(feature = "popup")]
use crate::ui::popup::PopupError;
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
    /// The application context could not be built.
    #[error("could not build the context")]
    App(#[from] AppError),
    /// The runtime state could not be read or written.
    #[error("could not access the runtime state")]
    State(#[from] StateError),
    /// The XDG directories could not be resolved.
    #[error("could not resolve the directories")]
    Paths(#[from] PathsError),
    /// The configuration file could not be loaded.
    #[error("could not load the configuration")]
    Config(#[from] ConfigError),
    /// The configuration file could not be changed.
    #[error("could not change the configuration")]
    Edit(#[from] EditError),
    /// The cryptocurrency source failed.
    #[error("the cryptocurrency source failed")]
    Source(#[from] SourceError),
    /// The price series of a cryptocurrency could not be fetched.
    #[error("could not fetch the series of {id}")]
    Series {
        /// Cryptocurrency whose series was requested.
        id: String,
        /// The underlying source failure.
        #[source]
        source: SourceError,
    },
    /// The cryptocurrency search failed.
    #[error("could not search the cryptocurrencies")]
    Search(#[source] SourceError),
    /// Waybar could not be signalled.
    #[error("could not notify Waybar")]
    Signal(#[from] SignalError),
    /// A command argument is invalid.
    #[error("the arguments are invalid")]
    Input(#[from] ModelError),
    /// A module payload could not be encoded as JSON.
    #[error("could not render the module")]
    Render(#[from] serde_json::Error),
    /// The output could not be written.
    #[error("could not write the output")]
    Write(#[source] io::Error),
    /// A watched file could not be inspected.
    #[error("could not watch {}", path.display())]
    Watch {
        /// File that could not be inspected.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The running executable could not be located.
    #[error("could not locate the running executable")]
    Executable(#[source] io::Error),
    /// A program could not be started.
    #[error("could not start {program}")]
    Launch {
        /// Program that could not be started.
        program: String,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// No cryptocurrency is configured.
    #[error("no cryptocurrency is configured")]
    NoCoin,
    /// No terminal emulator is configured.
    #[error("no terminal emulator is configured")]
    NoTerminal,
    /// The graphical window failed.
    #[cfg(feature = "popup")]
    #[error("the popup window failed")]
    Popup(#[from] PopupError),
    /// The binary was built without the `popup` feature.
    #[cfg(not(feature = "popup"))]
    #[error("this binary was compiled without the `popup` feature")]
    PopupUnavailable,
}

/// Failures met on the previous ticks of a long-running command. A failure is
/// reported when it first appears, stays silent while it lasts, and is reported
/// again once it has recovered and comes back.
#[derive(Debug)]
pub struct Ongoing<Key> {
    active: HashSet<Key>,
}

impl<Key> Default for Ongoing<Key> {
    fn default() -> Self {
        Self {
            active: HashSet::new(),
        }
    }
}

impl<Key: Eq + Hash> Ongoing<Key> {
    /// Records a failure seen on this tick. Returns whether it is new, that is,
    /// whether it must be reported.
    #[must_use]
    pub fn begin(&mut self, failure: Key) -> bool {
        self.active.insert(failure)
    }

    /// Forgets every failure absent from `present`, so that each one that
    /// recovers is reported again when it comes back.
    pub fn settle(&mut self, present: &[Key]) {
        self.active.retain(|failure| present.contains(failure));
    }
}

/// Writes one line of result to stdout.
///
/// # Errors
/// Fails when stdout cannot be written.
pub fn print<Out: Write, Err: Write>(
    output: &mut Output<Out, Err>,
    text: impl Display,
) -> Result<(), CommandError> {
    output.line(text).map_err(CommandError::Write)
}

/// Writes a text as it is, the newline included or not.
///
/// # Errors
/// Fails when stdout cannot be written.
pub fn print_text<Out: Write, Err: Write>(
    output: &mut Output<Out, Err>,
    text: impl Display,
) -> Result<(), CommandError> {
    output.text(text).map_err(CommandError::Write)
}

/// Loads the state, reporting each of its notices once on stderr. Call it once
/// per run and pass the result along: a second call would report them again.
///
/// # Errors
/// Fails only when stderr cannot be written.
pub fn load_state<Out: Write, Err: Write>(
    app: &App,
    output: &mut Output<Out, Err>,
) -> Result<LoadedState, CommandError> {
    let loaded = app.state();
    warn_each(output, &loaded.notices)?;
    Ok(loaded)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lasting_failure_is_new_only_once() {
        let mut ongoing = Ongoing::default();
        assert!(ongoing.begin("state"));
        assert!(!ongoing.begin("state"));
    }

    #[test]
    fn a_failure_is_new_again_after_it_recovered() {
        let mut ongoing = Ongoing::default();
        assert!(ongoing.begin("state"));
        ongoing.settle(&[]);
        assert!(ongoing.begin("state"));
    }

    #[test]
    fn distinct_failures_recover_apart() {
        let mut ongoing = Ongoing::default();
        assert!(ongoing.begin("read"));
        assert!(ongoing.begin("write"));
        ongoing.settle(&["read"]);
        assert!(!ongoing.begin("read"));
        assert!(ongoing.begin("write"));
    }
}
