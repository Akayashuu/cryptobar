use std::fs;
use std::fs::File;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;
use serde::{Deserialize, Serialize};

use super::key::sanitize;
use super::{BACKOFF_MAX, BACKOFF_MIN, Backoff, Entry, now};
use crate::paths;
use crate::source::{PriceSource, SourceError};

/// Wraps a source with a disk cache.
///
/// An entry that is corrupt or unreadable is a miss, never an error: the cache
/// is derived from the network and is rebuilt on the next fetch. A failed entry
/// or lock write is surfaced to the caller. The backoff is a side channel: a
/// failed read or write of it is kept as the notice, taken once by the caller,
/// and never replaces the outcome of the fetch.
#[derive(Debug)]
pub struct CachedSource<S: PriceSource> {
    pub(super) source: S,
    pub(super) dir: PathBuf,
    pub(super) ttl: Duration,
    notice: Mutex<Option<SourceError>>,
}

impl<S: PriceSource> CachedSource<S> {
    /// Cache around `source`, stored in `dir` and valid for `ttl`.
    pub fn new(source: S, dir: PathBuf, ttl: Duration) -> Self {
        Self {
            source,
            dir,
            ttl,
            notice: Mutex::new(None),
        }
    }

    /// The failure of a side read or write that did not stop the value from
    /// being served, such as an unreadable backoff file. Taken once, so the
    /// caller reports it once.
    #[must_use]
    pub fn take_notice(&self) -> Option<SourceError> {
        self.notice
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// The wrapped source.
    #[must_use]
    pub fn source(&self) -> &S {
        &self.source
    }

    pub(super) fn is_fresh(&self, fetched_at: u64, now_secs: u64) -> bool {
        now_secs.saturating_sub(fetched_at) < self.ttl.as_secs()
    }

    pub(super) fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.json", sanitize(key)))
    }

    pub(super) fn read<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<Entry<T>> {
        let raw = fs::read_to_string(self.path(key)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub(super) fn write<T: Serialize>(
        &self,
        key: &str,
        entry: &Entry<T>,
    ) -> Result<(), SourceError> {
        paths::ensure_dir(&self.dir)?;
        let final_path = self.path(key);
        let tmp_path = final_path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(entry).map_err(encoding_error)?;
        let mut file = File::create(&tmp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }

    pub(super) fn try_lock(&self) -> Result<Option<File>, SourceError> {
        paths::ensure_dir(&self.dir)?;
        let file = File::create(self.dir.join(".lock"))?;
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Some(file)),
            Err(errno) if errno == Errno::WOULDBLOCK => Ok(None),
            Err(errno) => Err(io::Error::from(errno).into()),
        }
    }

    pub(super) fn backoff_path(&self) -> PathBuf {
        self.dir.join("backoff.state")
    }

    pub(super) fn backoff(&self) -> Backoff {
        match self.read_backoff() {
            Ok(backoff) => backoff,
            Err(failure) => {
                self.note(failure.into());
                Backoff::default()
            }
        }
    }

    fn read_backoff(&self) -> io::Result<Backoff> {
        match fs::read_to_string(self.backoff_path()) {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(absent) if absent.kind() == io::ErrorKind::NotFound => Ok(Backoff::default()),
            Err(failure) => Err(failure),
        }
    }

    pub(super) fn note(&self, failure: SourceError) {
        *self.notice.lock().unwrap_or_else(PoisonError::into_inner) = Some(failure);
    }

    pub(super) fn record_failure(&self, retry_after: Option<u64>) {
        if let Err(failure) = self.write_backoff(retry_after) {
            self.note(failure);
        }
    }

    fn write_backoff(&self, retry_after: Option<u64>) -> Result<(), SourceError> {
        let previous = self.backoff();
        let delay = retry_after.unwrap_or_else(|| {
            if previous.delay == 0 {
                BACKOFF_MIN
            } else {
                previous.delay.saturating_mul(2)
            }
        });
        let delay = delay.clamp(BACKOFF_MIN, BACKOFF_MAX);
        let next = Backoff {
            retry_after: now().saturating_add(delay),
            delay,
        };
        let bytes = serde_json::to_vec(&next).map_err(encoding_error)?;
        paths::ensure_dir(&self.dir)?;
        fs::write(self.backoff_path(), bytes)?;
        Ok(())
    }

    pub(super) fn clear_failure(&self) -> Result<(), SourceError> {
        match fs::remove_file(self.backoff_path()) {
            Ok(()) => Ok(()),
            Err(absent) if absent.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(failure) => Err(failure.into()),
        }
    }
}

fn encoding_error(source: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, source)
}
