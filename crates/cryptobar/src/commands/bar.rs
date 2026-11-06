//! `bar` subcommand: emission of the Waybar module.

use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use super::{CommandError, load_state, print, warn, warn_each};
use crate::alerts;
use crate::app::App;
use crate::model::Quote;
use crate::onchain;
use crate::output::Output;
use crate::render::waybar::{self, Markup};
use crate::state::{LoadedState, State};

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
    let mut config_watch = Watched::start(app.config_path().to_path_buf(), output)?;
    let mut state_watch = Watched::start(State::default_path()?, output)?;

    loop {
        emit(&app, markup, true, output)?;

        let deadline =
            Instant::now().checked_add(Duration::from_secs(app.config.refresh_interval.max(1)));
        let mut config_changed = false;
        loop {
            thread::sleep(POLL);
            if config_watch.changed(output)? {
                config_changed = true;
                break;
            }
            if state_watch.changed(output)? {
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

struct Watched {
    path: PathBuf,
    stamp: Option<SystemTime>,
    failure_reported: bool,
}

impl Watched {
    fn start<Out: Write, Err: Write>(
        path: PathBuf,
        output: &mut Output<Out, Err>,
    ) -> Result<Self, CommandError> {
        let mut watched = Self {
            path,
            stamp: None,
            failure_reported: false,
        };
        watched.stamp = watched.sample(output)?;
        Ok(watched)
    }

    fn changed<Out: Write, Err: Write>(
        &mut self,
        output: &mut Output<Out, Err>,
    ) -> Result<bool, CommandError> {
        let stamp = self.sample(output)?;
        let changed = stamp != self.stamp;
        self.stamp = stamp;
        Ok(changed)
    }

    fn sample<Out: Write, Err: Write>(
        &mut self,
        output: &mut Output<Out, Err>,
    ) -> Result<Option<SystemTime>, CommandError> {
        match stamp(&self.path) {
            Ok(stamp) => Ok(stamp),
            Err(source) => {
                if !self.failure_reported {
                    self.failure_reported = true;
                    warn(
                        output,
                        &CommandError::Watch {
                            path: self.path.clone(),
                            source,
                        },
                    )?;
                }
                Ok(None)
            }
        }
    }
}

fn stamp(path: &Path) -> io::Result<Option<SystemTime>> {
    match fs::metadata(path).and_then(|meta| meta.modified()) {
        Ok(modified) => Ok(Some(modified)),
        Err(absent) if absent.kind() == ErrorKind::NotFound => Ok(None),
        Err(failure) => Err(failure),
    }
}

fn dispatch_alerts<Out: Write, Err: Write>(
    app: &App,
    quotes: &[Quote],
    loaded: &mut LoadedState,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    if app.config.alerts.is_empty() || !loaded.is_writable() {
        return Ok(());
    }
    let triggers = alerts::evaluate(&app.config, quotes);
    let (fresh, current) = alerts::diff(&triggers, &loaded.state.fired);
    for trigger in &fresh {
        if let Err(failure) = alerts::notify(trigger) {
            warn(output, &failure)?;
        }
    }
    if current != loaded.state.fired {
        loaded.state.fired = current;
        if let Err(failure) = app.save_state(&loaded.state) {
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
    if let Some(notice) = app.source.take_notice() {
        warn(output, &notice)?;
    }
    let mut loaded = load_state(app, output)?;
    if live {
        warn_each(output, &app.fetch_icons(&mut quotes))?;
        dispatch_alerts(app, &quotes, &mut loaded, output)?;
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
    let onchain = onchain::totals(&refresh.balances);
    let module = waybar::render(
        &quotes,
        app.index_of(&loaded.state),
        &app.config,
        stale,
        markup,
        &onchain,
    );
    print(output, module.to_line()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_has_no_stamp() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(stamp(&dir.path().join("absent")).unwrap(), None);
    }

    #[test]
    fn an_unreadable_path_is_an_error_not_an_absence() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("plain");
        fs::write(&file, b"not a directory").unwrap();
        assert!(matches!(
            stamp(&file.join("child")),
            Err(failure) if failure.kind() != ErrorKind::NotFound
        ));
    }
}
