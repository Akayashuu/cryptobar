//! `popup` subcommand: opening the graphical window.

use super::CommandError;
use crate::app::App;
#[cfg(feature = "popup")]
use crate::ui::popup;

/// Opens the window from the data already on disk: the network is left to
/// the window's own workers, so a slow API never delays its opening.
///
/// # Errors
/// Fails when GTK cannot initialise or the window exits with an error.
#[cfg(feature = "popup")]
pub fn run(app: App) -> Result<(), CommandError> {
    let (mut quotes, stale) = app
        .source
        .markets_stored(&app.config.coins, &app.config.currency)
        .map_or((Vec::new(), true), |cached| (cached.value, cached.stale));
    app.attach_icons(&mut quotes);
    popup::show(app, quotes, stale)?;
    Ok(())
}

/// Variant compiled without GTK: the command exists but explains its absence.
///
/// # Errors
/// Always fails.
#[cfg(not(feature = "popup"))]
pub fn run(_app: App) -> Result<(), CommandError> {
    Err(CommandError::PopupUnavailable)
}
