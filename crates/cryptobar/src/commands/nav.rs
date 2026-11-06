//! Navigation subcommands: `next`, `prev` and `refresh`.

use std::io::Write;

use super::{CommandError, load_state};
use crate::app::App;
use crate::output::Output;

/// Moves the selection by `delta` positions then wakes Waybar up.
///
/// # Errors
/// Fails when the state cannot be read or written, or Waybar cannot be signalled.
pub fn step<Out: Write, Err: Write>(
    app: &App,
    output: &mut Output<Out, Err>,
    delta: isize,
) -> Result<(), CommandError> {
    let mut state = load_state(app, output)?;
    let len = app.config.coins.len();
    state.select(app.index_of(&state), len);
    state.step(delta, len);
    app.save_state(&state)?;
    app.notify()?;
    Ok(())
}

/// Pins the selection to an explicit index then wakes Waybar up.
///
/// # Errors
/// Fails when the state cannot be read or written, or Waybar cannot be signalled.
pub fn select<Out: Write, Err: Write>(
    app: &App,
    output: &mut Output<Out, Err>,
    index: usize,
) -> Result<(), CommandError> {
    let mut state = load_state(app, output)?;
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
