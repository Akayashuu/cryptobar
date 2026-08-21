//! Disk cache with TTL, atomic writes and an advisory lock.
//!
//! The goal: whatever the number of Waybar ticks, the API is called only once
//! per interval, and two concurrent processes never fire a duplicate request.

use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{PriceSource, SearchHit, SourceError};
use crate::model::{CoinId, Currency, Quote, Range, Series};

/// First backoff delay after a network failure.
const BACKOFF_MIN: u64 = 60;
/// Backoff ceiling: beyond it, spacing further out is pointless.
const BACKOFF_MAX: u64 = 1800;

/// Value coming from the cache, with the time of the last network success.
#[derive(Debug, Clone)]
pub struct Cached<T> {
    pub value: T,
    pub fetched_at: u64,
    /// `true` when the served value could not be refreshed.
    pub stale: bool,
}

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    fetched_at: u64,
    value: T,
}

#[derive(Serialize, Deserialize, Default)]
struct Backoff {
    /// Unix timestamp before which no request must be attempted.
    retry_after: u64,
    /// Current delay, doubled on every consecutive failure.
    delay: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Safe file name derived from a logical key. The key only holds already
/// validated characters, but it is filtered again so that no path can escape
/// the cache directory.
fn sanitize(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(120)
        .collect()
}

/// Wraps a source with a disk cache.
pub struct CachedSource<S: PriceSource> {
    source: S,
    dir: PathBuf,
    ttl: Duration,
}

impl<S: PriceSource> CachedSource<S> {
    pub fn new(source: S, dir: PathBuf, ttl: Duration) -> Self {
        Self { source, dir, ttl }
    }

    #[must_use]
    pub fn source(&self) -> &S {
        &self.source
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.json", sanitize(key)))
    }

    fn read<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<Entry<T>> {
        let raw = std::fs::read_to_string(self.path(key)).ok()?;
        // A corrupt cache is treated as missing: we never want to fail on that.
        serde_json::from_str(&raw).ok()
    }

    fn write<T: Serialize>(&self, key: &str, entry: &Entry<T>) -> std::io::Result<()> {
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
    fn try_lock(&self) -> Option<File> {
        crate::paths::ensure_dir(&self.dir).ok()?;
        let file = File::create(self.dir.join(".lock")).ok()?;
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Some(file),
            Err(_) => None,
        }
    }

    fn backoff_path(&self) -> PathBuf {
        self.dir.join("backoff.state")
    }

    fn backoff(&self) -> Backoff {
        std::fs::read_to_string(self.backoff_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn record_failure(&self, retry_after: Option<u64>) {
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

    fn clear_failure(&self) {
        let _ = std::fs::remove_file(self.backoff_path());
    }

    /// Heart of the cache logic, shared by every method.
    fn get_or_refresh<T, F>(&self, key: &str, fetch: F) -> Result<Cached<T>, SourceError>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
        F: FnOnce() -> Result<T, SourceError>,
    {
        let existing = self.read::<T>(key);
        let now_secs = now();

        if let Some(entry) = &existing {
            let age = now_secs.saturating_sub(entry.fetched_at);
            if age < self.ttl.as_secs() {
                return Ok(Cached {
                    value: entry.value.clone(),
                    fetched_at: entry.fetched_at,
                    stale: false,
                });
            }
        }

        let blocked = now_secs < self.backoff().retry_after;
        let lock = if blocked { None } else { self.try_lock() };

        if lock.is_none() {
            // Backoff active or lock held elsewhere: we serve what we have.
            return match existing {
                Some(entry) => Ok(Cached {
                    value: entry.value,
                    fetched_at: entry.fetched_at,
                    stale: true,
                }),
                None if blocked => Err(SourceError::Http(
                    "backoff active after a network failure, no cached data".into(),
                )),
                None => fetch().map(|value| {
                    let _ = self.write(
                        key,
                        &Entry {
                            fetched_at: now_secs,
                            value: value.clone(),
                        },
                    );
                    Cached {
                        value,
                        fetched_at: now_secs,
                        stale: false,
                    }
                }),
            };
        }

        match fetch() {
            Ok(value) => {
                self.clear_failure();
                let _ = self.write(
                    key,
                    &Entry {
                        fetched_at: now_secs,
                        value: value.clone(),
                    },
                );
                Ok(Cached {
                    value,
                    fetched_at: now_secs,
                    stale: false,
                })
            }
            Err(err) => {
                let retry_after = match &err {
                    SourceError::RateLimited { retry_after } => *retry_after,
                    _ => None,
                };
                self.record_failure(retry_after);
                match existing {
                    Some(entry) => Ok(Cached {
                        value: entry.value,
                        fetched_at: entry.fetched_at,
                        stale: true,
                    }),
                    None => Err(err),
                }
            }
        }
    }

    /// Quotes, served from the cache as long as it is fresh.
    ///
    /// # Errors
    /// Fails only when the network fails **and** no cache exists.
    pub fn markets_cached(
        &self,
        ids: &[CoinId],
        vs: &Currency,
    ) -> Result<Cached<Vec<Quote>>, SourceError> {
        let key = format!(
            "markets-{}-{}",
            vs.as_str(),
            ids.iter().map(CoinId::as_str).collect::<Vec<_>>().join("_")
        );
        self.get_or_refresh(&key, || self.source.markets(ids, vs))
    }

    /// Price series, cached per (cryptocurrency, range) pair.
    ///
    /// # Errors
    /// Fails only when the network fails **and** no cache exists.
    pub fn series_cached(
        &self,
        id: &CoinId,
        range: Range,
        vs: &Currency,
    ) -> Result<Cached<Series>, SourceError> {
        let key = format!("series-{}-{}-{}", vs.as_str(), id.as_str(), range.slug());
        self.get_or_refresh(&key, || self.source.series(id, range, vs))
    }

    /// Search: never cached, it is one-shot and interactive.
    ///
    /// # Errors
    /// See [`SourceError`].
    pub fn search(&self, term: &str) -> Result<Vec<SearchHit>, SourceError> {
        self.source.search(term)
    }

    /// Removes the whole cache content and resets the backoff.
    ///
    /// # Errors
    /// Bubbles up I/O errors other than "file not found".
    pub fn invalidate(&self) -> std::io::Result<()> {
        self.clear_failure();
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Test source: counts calls and can be forced to fail.
    struct FakeSource {
        calls: Cell<usize>,
        fail: bool,
    }

    impl FakeSource {
        fn new(fail: bool) -> Self {
            Self {
                calls: Cell::new(0),
                fail,
            }
        }
    }

    impl PriceSource for FakeSource {
        fn markets(&self, ids: &[CoinId], _vs: &Currency) -> Result<Vec<Quote>, SourceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            if self.fail {
                return Err(SourceError::Http("simulated outage".into()));
            }
            Ok(ids
                .iter()
                .map(|id| Quote {
                    id: id.clone(),
                    symbol: "X".into(),
                    name: "X".into(),
                    price: 1.0,
                    change_1h: None,
                    change_24h: Some(1.0),
                    change_7d: None,
                    sparkline: vec![1.0, 2.0],
                    image: None,
                    market_cap: None,
                    volume_24h: None,
                    rank: None,
                    ath: None,
                    ath_date: None,
                    icon: None,
                })
                .collect())
        }

        fn series(&self, id: &CoinId, range: Range, _vs: &Currency) -> Result<Series, SourceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            if self.fail {
                return Err(SourceError::Http("simulated outage".into()));
            }
            Ok(Series {
                id: id.clone(),
                range,
                points: vec![1.0, 2.0, 3.0],
            })
        }

        fn search(&self, _term: &str) -> Result<Vec<SearchHit>, SourceError> {
            Ok(Vec::new())
        }
    }

    fn setup(ttl: Duration, fail: bool) -> (tempfile::TempDir, CachedSource<FakeSource>) {
        let dir = tempfile::tempdir().unwrap();
        let cached = CachedSource::new(FakeSource::new(fail), dir.path().to_path_buf(), ttl);
        (dir, cached)
    }

    fn ids() -> Vec<CoinId> {
        vec![CoinId::parse("bitcoin").unwrap()]
    }

    #[test]
    fn the_second_call_within_the_ttl_makes_no_new_request() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        let first = cached.markets_cached(&ids(), &vs).unwrap();
        let second = cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 1);
        assert!(!first.stale);
        assert!(!second.stale);
    }

    #[test]
    fn an_expired_ttl_triggers_a_new_request() {
        let (_dir, cached) = setup(Duration::from_secs(0), false);
        let vs = Currency::default();
        cached.markets_cached(&ids(), &vs).unwrap();
        cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn a_network_outage_serves_the_cache_marked_stale() {
        let dir = tempfile::tempdir().unwrap();
        let vs = Currency::default();
        {
            let ok = CachedSource::new(
                FakeSource::new(false),
                dir.path().to_path_buf(),
                Duration::from_secs(0),
            );
            ok.markets_cached(&ids(), &vs).unwrap();
        }
        let broken = CachedSource::new(
            FakeSource::new(true),
            dir.path().to_path_buf(),
            Duration::from_secs(0),
        );
        let result = broken.markets_cached(&ids(), &vs).unwrap();
        assert!(result.stale);
        assert_eq!(result.value.len(), 1);
    }

    #[test]
    fn an_outage_without_a_cache_bubbles_the_error_up() {
        let (_dir, cached) = setup(Duration::from_secs(60), true);
        assert!(cached.markets_cached(&ids(), &Currency::default()).is_err());
    }

    #[test]
    fn the_backoff_keeps_the_api_from_being_hammered_after_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let vs = Currency::default();
        {
            let ok = CachedSource::new(
                FakeSource::new(false),
                dir.path().to_path_buf(),
                Duration::from_secs(0),
            );
            ok.markets_cached(&ids(), &vs).unwrap();
        }
        let broken = CachedSource::new(
            FakeSource::new(true),
            dir.path().to_path_buf(),
            Duration::from_secs(0),
        );
        broken.markets_cached(&ids(), &vs).unwrap();
        broken.markets_cached(&ids(), &vs).unwrap();
        // The second call is blocked by the backoff: a single network request.
        assert_eq!(broken.source().calls.get(), 1);
    }

    #[test]
    fn changing_currency_or_list_changes_the_cache_key() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        cached
            .markets_cached(&ids(), &Currency::parse("eur").unwrap())
            .unwrap();
        cached
            .markets_cached(&ids(), &Currency::parse("usd").unwrap())
            .unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn invalidate_forces_a_new_request() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        cached.markets_cached(&ids(), &vs).unwrap();
        cached.invalidate().unwrap();
        cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn a_corrupt_cache_is_treated_as_missing() {
        let (dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        cached.markets_cached(&ids(), &vs).unwrap();
        for entry in std::fs::read_dir(dir.path()).unwrap().flatten() {
            if entry.path().extension().is_some_and(|e| e == "json") {
                std::fs::write(entry.path(), b"{ not json").unwrap();
            }
        }
        cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn the_series_are_cached_per_range() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        let id = CoinId::parse("bitcoin").unwrap();
        cached.series_cached(&id, Range::D7, &vs).unwrap();
        cached.series_cached(&id, Range::D7, &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 1);
        cached.series_cached(&id, Range::H24, &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }
}
