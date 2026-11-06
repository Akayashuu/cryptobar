//! `bar` subcommand: emission of the Waybar module.

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use super::{CommandError, print, warn, warn_each};
use crate::alerts;
use crate::app::App;
use crate::model::Quote;
use crate::output::Output;
use crate::render::waybar::{self, Markup};
use crate::state::State;

const POLL: Duration = Duration::from_millis(400);

/// Emits one JSON line on standard output.
///
/// # Errors
/// Fails when the state cannot be read or the output cannot be written. A
/// network outage never fails the command as long as a cache exists, and a
/// missing cache is shown as a degraded module with the cause on stderr.
pub fn run<Out: Write, Err: Write>(
    app: App,
    watch: bool,
    markup: Markup,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    if watch {
        watch_loop(app, markup, output)
    } else {
        emit(&app, markup, false, output)
    }
}

fn watch_loop<Out: Write, Err: Write>(
    mut app: App,
    markup: Markup,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    let state_path = State::default_path()?;
    let mut config_stamp = stamp(app.config_path());
    let mut state_stamp = stamp(&state_path);

    loop {
        emit(&app, markup, true, output)?;

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
            match app.reload() {
                Ok(reloaded) => app = reloaded,
                Err(failure) => warn(output, &failure)?,
            }
        }
    }
}

fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

fn dispatch_alerts<Out: Write, Err: Write>(
    app: &App,
    quotes: &[Quote],
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    if app.config.alerts.is_empty() {
        return Ok(());
    }
    let triggers = alerts::evaluate(&app.config, quotes);
    let mut state = app.state()?;
    let (fresh, current) = alerts::diff(&triggers, &state.fired);
    for trigger in &fresh {
        if let Err(failure) = alerts::notify(trigger) {
            warn(output, &failure)?;
        }
    }
    if current != state.fired {
        state.fired = current;
        if let Err(failure) = app.save_state(&state) {
            warn(output, &failure)?;
        }
    }
    Ok(())
}

fn emit<Out: Write, Err: Write>(
    app: &App,
    markup: Markup,
    live: bool,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    let (mut quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        Err(failure) => {
            warn(output, &failure)?;
            (Vec::new(), true)
        }
    };
    if live {
        warn_each(output, &app.fetch_icons(&mut quotes))?;
        dispatch_alerts(app, &quotes, output)?;
    } else {
        app.attach_icons(&mut quotes);
    }
    let watcher = app.watcher();
    let refresh = if live {
        watcher.refresh(&app.config.wallets)
    } else {
        watcher.cached(&app.config.wallets)
    };
    warn_each(output, &refresh.failures)?;
    let onchain = crate::onchain::totals(&refresh.balances);
    let module = waybar::render(
        &quotes,
        app.active_index()?,
        &app.config,
        stale,
        markup,
        &onchain,
    );
    print(output, module.to_line()?)
}
