use std::fs::File;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::paths;
use crate::source::{PriceSource, SourceError};

use super::key::sanitize;
use super::{BACKOFF_MAX, BACKOFF_MIN, Backoff, Entry, now};

/// Wraps a source with a disk cache.
///
/// An entry that is corrupt or unreadable is a miss, never an error: the cache
/// is derived from the network and is rebuilt on the next fetch. Every write
/// that fails (entry, lock, backoff) is a [`SourceError::Cache`] or an I/O error
/// surfaced to the caller, never dropped.
pub struct CachedSource<S: PriceSource> {
    pub(super) source: S,
    pub(super) dir: PathBuf,
    pub(super) ttl: Duration,
}

impl<S: PriceSource> CachedSource<S> {
    pub fn new(source: S, dir: PathBuf, ttl: Duration) -> Self {
        Self { source, dir, ttl }
    }

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

    /// A missing, unreadable or unparsable entry is a miss: the caller fetches
    /// again and the next write replaces the broken file.
    pub(super) fn read<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<Entry<T>> {
        let raw = std::fs::read_to_string(self.path(key)).ok()?;
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
        std::fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }

    /// Takes the write lock. `None` when another process already holds it: the
    /// caller then serves the existing cache instead of waiting.
    pub(super) fn try_lock(&self) -> Result<Option<File>, SourceError> {
        paths::ensure_dir(&self.dir)?;
        let file = File::create(self.dir.join(".lock"))?;
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Some(file)),
            Err(errno) if errno == rustix::io::Errno::WOULDBLOCK => Ok(None),
            Err(errno) => Err(io::Error::from(errno).into()),
        }
    }

    pub(super) fn backoff_path(&self) -> PathBuf {
        self.dir.join("backoff.state")
    }

    /// The backoff state of the upstream. A missing or unparsable file means no
    /// failure is recorded, which is the default backoff.
    pub(super) fn backoff(&self) -> Backoff {
        std::fs::read_to_string(self.backoff_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub(super) fn record_failure(&self, retry_after: Option<u64>) -> Result<(), SourceError> {
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
        std::fs::write(self.backoff_path(), bytes)?;
        Ok(())
    }

    pub(super) fn clear_failure(&self) -> Result<(), SourceError> {
        match std::fs::remove_file(self.backoff_path()) {
            Ok(()) => Ok(()),
            Err(absent) if absent.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(failure) => Err(failure.into()),
        }
    }
}

fn encoding_error(source: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, source)
}
