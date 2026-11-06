use serde::{Deserialize, Serialize};

use crate::source::PriceSource;

use super::key::sanitize;
use super::{BACKOFF_MAX, BACKOFF_MIN, Backoff, Entry, now};
use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

/// Wraps a source with a disk cache.
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

    pub(super) fn read<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<Entry<T>> {
        let raw = std::fs::read_to_string(self.path(key)).ok()?;
        // A corrupt cache is treated as missing: we never want to fail on that.
        serde_json::from_str(&raw).ok()
    }

    pub(super) fn write<T: Serialize>(&self, key: &str, entry: &Entry<T>) -> std::io::Result<()> {
        crate::paths::ensure_dir(&self.dir)?;
        let final_path = self.path(key);
        let tmp_path = final_path.with_extension("json.tmp");
        {
            let mut file = File::create(&tmp_path)?;
            let bytes = serde_json::to_vec(entry)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp_path, &final_path)
    }

    /// Takes the write lock. `None` when another process already holds it: the
    /// caller then serves the existing cache instead of waiting.
    pub(super) fn try_lock(&self) -> Option<File> {
        crate::paths::ensure_dir(&self.dir).ok()?;
        let file = File::create(self.dir.join(".lock")).ok()?;
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Some(file),
            Err(_) => None,
        }
    }

    pub(super) fn backoff_path(&self) -> PathBuf {
        self.dir.join("backoff.state")
    }

    pub(super) fn backoff(&self) -> Backoff {
        std::fs::read_to_string(self.backoff_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub(super) fn record_failure(&self, retry_after: Option<u64>) {
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
        if let Ok(bytes) = serde_json::to_vec(&next) {
            let _ = crate::paths::ensure_dir(&self.dir);
            let _ = std::fs::write(self.backoff_path(), bytes);
        }
    }

    pub(super) fn clear_failure(&self) {
        let _ = std::fs::remove_file(self.backoff_path());
    }
}
