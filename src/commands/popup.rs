//! `popup` subcommand: opening the graphical window.

use anyhow::Result;

use crate::app::App;

/// Opens the window from the data already on disk: the network is left to
/// the window's own workers, so a slow API never delays its opening.
///
/// # Errors
/// Fails when GTK cannot initialise.
#[cfg(feature = "popup")]
pub fn run(app: App) -> Result<()> {
    let (mut quotes, stale) = app
        .source
        .markets_stored(&app.config.coins, &app.config.currency)
        .map_or((Vec::new(), true), |cached| (cached.value, cached.stale));
    app.attach_icons(&mut quotes);
    crate::ui::popup::show(app, quotes, stale)
}

/// Variant compiled without GTK: the command exists but explains its absence.
///
/// # Errors
/// Always fails.
#[cfg(not(feature = "popup"))]
pub fn run(_app: App) -> Result<()> {
    anyhow::bail!("binary compiled without the `popup` feature")
}
