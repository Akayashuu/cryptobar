//! Navigation subcommands: `next`, `prev` and `refresh`.

use super::CommandError;
use crate::app::App;

/// Moves the selection by `delta` positions then wakes Waybar up.
///
/// # Errors
/// Fails when the state cannot be read or written, or Waybar cannot be signalled.
pub fn step(app: &App, delta: isize) -> Result<(), CommandError> {
    let mut state = app.state()?;
    state.select(app.active_index()?, app.config.coins.len());
    state.step(delta, app.config.coins.len());
    app.save_state(&state)?;
    app.notify()?;
    Ok(())
}

/// Pins the selection to an explicit index then wakes Waybar up.
///
/// # Errors
/// Fails when the state cannot be read or written, or Waybar cannot be signalled.
pub fn select(app: &App, index: usize) -> Result<(), CommandError> {
    let mut state = app.state()?;
    state.select(index, app.config.coins.len());
    app.save_state(&state)?;
    app.notify()?;
    Ok(())
}

/// Empties the cache to force a request on the next display.
///
/// # Errors
/// Fails when the cache files cannot be removed, or Waybar cannot be signalled.
pub fn refresh(app: &App) -> Result<(), CommandError> {
    app.source.invalidate()?;
    app.notify()?;
    Ok(())
}
