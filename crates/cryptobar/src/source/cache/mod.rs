//! Disk cache with TTL, atomic writes and an advisory lock.
//!
//! The goal: whatever the number of Waybar ticks, the API is called only once
//! per interval, and two concurrent processes never fire a duplicate request.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

mod key;
mod refresh;
mod store;
#[cfg(test)]
mod tests;

pub use store::CachedSource;

const BACKOFF_MIN: u64 = 60;
const BACKOFF_MAX: u64 = 1800;

/// Value coming from the cache, with the time of the last network success.
#[derive(Debug, Clone)]
pub struct Cached<T> {
    /// The cached value.
    pub value: T,
    /// Unix time, in seconds, of the last successful fetch.
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
    retry_after: u64,
    delay: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
