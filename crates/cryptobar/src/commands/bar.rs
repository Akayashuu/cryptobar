//! `bar` subcommand: emission of the Waybar module.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::mem::{self, Discriminant};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use super::{CommandError, Ongoing, print, warn, warn_each};
use crate::alerts;
use crate::app::App;
use crate::model::Quote;
use crate::onchain;
use crate::output::Output;
use crate::render::waybar::{self, Markup};
use crate::source::SourceError;
use crate::state::{LoadedState, Occurrence, State, StateError};

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
        emit(&app, markup, false, &mut Session::default(), output)
    }
}

fn watch_loop<Out: Write, Err: Write>(
    mut app: App,
    markup: Markup,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    let mut config_watch = Watched::start(app.config_path().to_path_buf(), output)?;
    let mut state_watch = Watched::start(State::default_path()?, output)?;
    let mut session = Session::default();

    loop {
        emit(&app, markup, true, &mut session, output)?;

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
                if state_watch.is_present() {
                    session.state_rewritten();
                }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SourceOccurrence {
    kind: Discriminant<SourceError>,
    message: String,
}

#[derive(Debug, Default)]
struct Session {
    state_reads: Ongoing<Occurrence>,
    state_writes: Ongoing<Occurrence>,
    source: Ongoing<SourceOccurrence>,
    fired: Option<BTreeSet<String>>,
}

impl Session {
    fn source_failed(&mut self, failure: &SourceError) -> bool {
        self.source.begin(SourceOccurrence {
            kind: mem::discriminant(failure),
            message: failure.to_string(),
        })
    }

    fn source_recovered(&mut self) {
        self.source.settle(&[]);
    }

    fn state_rewritten(&mut self) {
        self.state_writes.settle(&[]);
    }
}

struct Watched {
    path: PathBuf,
    stamp: Option<SystemTime>,
    failures: Ongoing<Occurrence>,
}

impl Watched {
    fn start<Out: Write, Err: Write>(
        path: PathBuf,
        output: &mut Output<Out, Err>,
    ) -> Result<Self, CommandError> {
        let mut watched = Self {
            path,
            stamp: None,
            failures: Ongoing::default(),
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

    fn is_present(&self) -> bool {
        self.stamp.is_some()
    }

    fn sample<Out: Write, Err: Write>(
        &mut self,
        output: &mut Output<Out, Err>,
    ) -> Result<Option<SystemTime>, CommandError> {
        match stamp(&self.path) {
            Ok(stamp) => {
                self.failures.settle(&[]);
                Ok(stamp)
            }
            Err(source) => {
                let occurrence = Occurrence {
                    path: self.path.clone(),
                    kind: source.kind(),
                };
                if self.failures.begin(occurrence) {
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
    session: &mut Session,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    if app.config.alerts.is_empty() || !loaded.is_writable() {
        return Ok(());
    }
    let remembered = session
        .fired
        .get_or_insert_with(|| loaded.state.fired.clone());
    let triggers = alerts::evaluate(&app.config, quotes);
    let (fresh, current) = alerts::diff(&triggers, remembered);
    for trigger in &fresh {
        if let Err(failure) = alerts::notify(trigger) {
            warn(output, &failure)?;
        }
    }
    if current != *remembered {
        loaded.state.fired.clone_from(&current);
        *remembered = current;
        match app.save_state(&loaded.state) {
            Ok(()) => session.state_writes.settle(&[]),
            Err(failure) => loaded.notices.push(failure),
        }
    }
    Ok(())
}

fn repaired_cleanly(loaded: &LoadedState) -> bool {
    let repaired = loaded
        .notices
        .iter()
        .any(|notice| matches!(notice, StateError::Corrupt { .. }));
    repaired && !loaded.notices.iter().any(StateError::is_write)
}

fn report_state<Out: Write, Err: Write>(
    app: &App,
    loaded: &LoadedState,
    live: bool,
    session: &mut Session,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    let reads: Vec<Occurrence> = loaded
        .notices
        .iter()
        .filter(|notice| !notice.is_write())
        .filter_map(StateError::occurrence)
        .collect();
    session.state_reads.settle(&reads);
    if repaired_cleanly(loaded) {
        session.state_writes.settle(&[]);
    }
    let mut fresh = false;
    for notice in &loaded.notices {
        let failures = if notice.is_write() {
            &mut session.state_writes
        } else {
            &mut session.state_reads
        };
        if notice
            .occurrence()
            .is_none_or(|occurrence| failures.begin(occurrence))
        {
            fresh = true;
            warn(output, notice)?;
        }
    }
    if fresh && live && !loaded.is_writable() && !app.config.alerts.is_empty() {
        output
            .warning(format_args!(
                "cryptobar: alerts are paused until the state file {} is readable",
                app.state_path().display()
            ))
            .map_err(CommandError::Write)?;
    }
    Ok(())
}

fn report_source<Out: Write, Err: Write>(
    session: &mut Session,
    failure: &SourceError,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    if session.source_failed(failure) {
        warn(output, failure)?;
    }
    Ok(())
}

fn emit<Out: Write, Err: Write>(
    app: &App,
    markup: Markup,
    live: bool,
    session: &mut Session,
    output: &mut Output<Out, Err>,
) -> Result<(), CommandError> {
    let (mut quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => {
            session.source_recovered();
            (cached.value, cached.stale)
        }
        Err(failure) => {
            report_source(session, &failure, output)?;
            (Vec::new(), true)
        }
    };
    if let Some(notice) = app.source.take_notice() {
        report_source(session, &notice, output)?;
    }
    let mut loaded = app.state();
    if live {
        warn_each(output, &app.fetch_icons(&mut quotes))?;
        dispatch_alerts(app, &quotes, &mut loaded, session, output)?;
    } else {
        app.attach_icons(&mut quotes);
    }
    report_state(app, &loaded, live, session, output)?;
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

    #[test]
    fn a_source_failure_is_new_until_a_success_settles_it() {
        let mut session = Session::default();
        assert!(session.source_failed(&SourceError::Backoff));
        assert!(!session.source_failed(&SourceError::Backoff));
        assert!(session.source_failed(&SourceError::Payload(String::new())));
        session.source_recovered();
        assert!(session.source_failed(&SourceError::Backoff));
    }

    #[test]
    fn a_source_failure_of_the_same_kind_but_another_message_is_new() {
        let mut session = Session::default();
        assert!(session.source_failed(&SourceError::Http("status 500".to_owned())));
        assert!(session.source_failed(&SourceError::Http("connection reset".to_owned())));
        assert!(!session.source_failed(&SourceError::Http("connection reset".to_owned())));
    }

    #[test]
    fn a_state_rewrite_settles_a_failed_write() {
        let mut session = Session::default();
        let occurrence = Occurrence {
            path: PathBuf::from("state.json.tmp"),
            kind: ErrorKind::IsADirectory,
        };
        assert!(session.state_writes.begin(occurrence.clone()));
        session.state_rewritten();
        assert!(session.state_writes.begin(occurrence));
    }
}
