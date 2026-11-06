//! Navigation subcommands: `next`, `prev` and `refresh`.

use std::io::Write;

use super::{CommandError, load_state};
use crate::app::App;
use crate::output::Output;
use crate::state::LoadedState;

/// Moves the selection by `delta` positions then wakes Waybar up. An unreadable
/// state file is never overwritten, so the step is then not kept.
///
/// # Errors
/// Fails when the state cannot be saved, or Waybar cannot be signalled.
pub fn step<Out: Write, Err: Write>(
    app: &App,
    output: &mut Output<Out, Err>,
    delta: isize,
) -> Result<(), CommandError> {
    let mut loaded = load_state(app, output)?;
    let len = app.config.coins.len();
    loaded.state.select(app.index_of(&loaded.state), len);
    loaded.state.step(delta, len);
    persist(app, &loaded)?;
    app.notify()?;
    Ok(())
}

/// Pins the selection to an explicit index then wakes Waybar up.
///
/// # Errors
/// Fails when the state cannot be saved, or Waybar cannot be signalled.
pub fn select<Out: Write, Err: Write>(
    app: &App,
    output: &mut Output<Out, Err>,
    index: usize,
) -> Result<(), CommandError> {
    let mut loaded = load_state(app, output)?;
    loaded.state.select(index, app.config.coins.len());
    persist(app, &loaded)?;
    app.notify()?;
    Ok(())
}

fn persist(app: &App, loaded: &LoadedState) -> Result<(), CommandError> {
    if loaded.is_writable() {
        app.save_state(&loaded.state)?;
    }
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
