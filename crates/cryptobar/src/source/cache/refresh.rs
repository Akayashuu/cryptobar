use serde::{Deserialize, Serialize};

use crate::model::{CoinId, Currency, Quote, Range, Series};
use crate::source::{PriceSource, SearchHit, SourceError};

use super::key::markets_key;
use super::store::CachedSource;
use super::{Cached, Entry, now};

impl<S: PriceSource> CachedSource<S> {
    /// Heart of the cache logic, shared by every method.
    fn get_or_refresh<T, F>(&self, key: &str, fetch: F) -> Result<Cached<T>, SourceError>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
        F: FnOnce() -> Result<T, SourceError>,
    {
        let existing = self.read::<T>(key);
        let now_secs = now();

        if let Some(entry) = &existing
            && self.is_fresh(entry.fetched_at, now_secs)
        {
            return Ok(Cached {
                value: entry.value.clone(),
                fetched_at: entry.fetched_at,
                stale: false,
            });
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
        self.get_or_refresh(&markets_key(ids, vs), || self.source.markets(ids, vs))
    }

    /// Quotes as the disk holds them, without ever touching the network: what
    /// a window shows while it opens, before a worker refreshes it.
    #[must_use]
    pub fn markets_stored(&self, ids: &[CoinId], vs: &Currency) -> Option<Cached<Vec<Quote>>> {
        let entry = self.read::<Vec<Quote>>(&markets_key(ids, vs))?;
        let stale = !self.is_fresh(entry.fetched_at, now());
        Some(Cached {
            value: entry.value,
            fetched_at: entry.fetched_at,
            stale,
        })
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
