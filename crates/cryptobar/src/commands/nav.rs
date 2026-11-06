//! Navigation subcommands: `next`, `prev` and `refresh`.

use anyhow::Result;

use crate::app::App;

/// Moves the selection by `delta` positions then wakes Waybar up.
///
/// # Errors
/// Fails when the state cannot be written.
pub fn step(app: &App, delta: isize) -> Result<()> {
    let mut state = app.state();
    // We start from the effective index: after an automatic rotation, "next"
    // has to follow what the user sees, not the index frozen on disk.
    state.select(app.active_index(), app.config.coins.len());
    state.step(delta, app.config.coins.len());
    app.save_state(&state)?;
    app.notify();
    Ok(())
}

/// Pins the selection to an explicit index then wakes Waybar up.
///
/// # Errors
/// Fails when the state cannot be written.
pub fn select(app: &App, index: usize) -> Result<()> {
    let mut state = app.state();
    state.select(index, app.config.coins.len());
    app.save_state(&state)?;
    app.notify();
    Ok(())
}

/// Empties the cache to force a request on the next display.
///
/// # Errors
/// Fails when the cache files cannot be removed.
pub fn refresh(app: &App) -> Result<()> {
    app.source.invalidate()?;
    app.notify();
    Ok(())
}
