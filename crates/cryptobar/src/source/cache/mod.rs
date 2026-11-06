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
#[allow(clippy::unwrap_used)]
mod tests;

pub use store::CachedSource;

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
