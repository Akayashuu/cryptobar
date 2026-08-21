//! `popup` subcommand: opening the graphical window.

use anyhow::Result;

use crate::app::App;

/// Opens the window from the cached data.
///
/// # Errors
/// Fails when GTK cannot initialise.
#[cfg(feature = "popup")]
pub fn run(app: App) -> Result<()> {
    let (mut quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        Err(_) => (Vec::new(), true),
    };
    app.attach_icons(&mut quotes);
    // The window tops the missing logos up itself, on a dedicated thread: it
    // opens immediately rather than after the download.
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
