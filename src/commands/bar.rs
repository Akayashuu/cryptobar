//! `bar` subcommand: emission of the Waybar module.

use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;

use crate::app::App;
use crate::render::waybar::{self, Markup};

/// Step of the wait loop in watch mode. Two `stat` calls per step: the cost is
/// invisible, and that is what makes a click or a setting instant in the bar
/// instead of waiting for the next interval.
const POLL: Duration = Duration::from_millis(400);

/// Emits one JSON line on standard output.
///
/// # Errors
/// Fails when writing to standard output fails. A network outage never fails
/// the command as long as a cache exists.
pub fn run(app: App, watch: bool, markup: Markup) -> Result<()> {
    if watch {
        watch_loop(app, markup)
    } else {
        emit(&app, markup, false)
    }
}

/// Re-emits on every interval, and without waiting as soon as the selection
/// changes or the configuration is edited — from the graphical window, say.
fn watch_loop(mut app: App, markup: Markup) -> Result<()> {
    let state_path = crate::state::State::default_path()?;
    let mut config_stamp = stamp(app.config_path());
    let mut state_stamp = stamp(&state_path);

    loop {
        // Watch mode is not on the critical path of a tick: this is the right
        // place to top the logo cache up.
        emit(&app, markup, true)?;

        let deadline =
            Instant::now().checked_add(Duration::from_secs(app.config.refresh_interval.max(1)));
        let mut config_changed = false;
        loop {
            std::thread::sleep(POLL);
            let now_config = stamp(app.config_path());
            let now_state = stamp(&state_path);
            if now_config != config_stamp {
                config_stamp = now_config;
                config_changed = true;
                break;
            }
            if now_state != state_stamp {
                state_stamp = now_state;
                break;
            }
            if deadline.is_none_or(|deadline| Instant::now() >= deadline) {
                break;
            }
        }

        if config_changed {
            // A configuration that turned invalid must not kill the bar: we
            // keep the last known version and the user sees the error on the
            // next save.
            if let Ok(reloaded) = app.reload() {
                app = reloaded;
            }
        }
    }
}

/// Modification fingerprint of a file. `None` covers both absence and
/// unreadability: either way, nothing to report until it changes.
fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

/// Notifies the newly crossed alerts and remembers the current set.
///
/// Only called in watch mode: a one-shot `exec` tick must not ring an alert
/// watch mode has just handled, and a bar running `exec` every second must not
/// turn one crossing into an avalanche of bubbles.
fn dispatch_alerts(app: &App, quotes: &[crate::model::Quote]) {
    if app.config.alerts.is_empty() {
        return;
    }
    let triggers = crate::alerts::evaluate(&app.config, quotes);
    let mut state = app.state();
    let (fresh, current) = crate::alerts::diff(&triggers, &state.fired);
    for trigger in &fresh {
        crate::alerts::notify(trigger);
    }
    if current != state.fired {
        state.fired = current;
        // A failed write is not fatal: at worst an alert is notified again on
        // the next tick.
        let _ = app.save_state(&state);
    }
}

/// `live` tells watch mode from the one-shot tick: only it tops the logo cache
/// up and rings the alerts.
fn emit(app: &App, markup: Markup, live: bool) -> Result<()> {
    let (mut quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        // No data at all: we show a degraded module rather than leaving a hole
        // in the bar.
        Err(_) => (Vec::new(), true),
    };
    if live {
        app.fetch_icons(&mut quotes);
        dispatch_alerts(app, &quotes);
    } else {
        app.attach_icons(&mut quotes);
    }
    // In watch mode the process lives long enough to query the chains; a
    // one-shot tick sticks to the cache, to stay instant.
    let watcher = app.watcher();
    let balances = if live {
        watcher.refresh(&app.config.wallets)
    } else {
        watcher.cached(&app.config.wallets)
    };
    let onchain = crate::onchain::totals(&balances);
    let module = waybar::render(
        &quotes,
        app.active_index(),
        &app.config,
        stale,
        markup,
        &onchain,
    );
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", module.to_line()?)?;
    stdout.flush()?;
    Ok(())
}
