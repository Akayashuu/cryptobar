use crate::model::{CoinId, Currency, Quote, Range, Series};
use crate::source::{PriceSource, SearchHit, SourceError};
use std::time::Duration;

use super::*;
use std::cell::Cell;

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
        ok.markets_cached(&ids(), &vs).unwrap()
    };
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
fn the_stored_quotes_never_reach_the_network() {
    let (_dir, cached) = setup(Duration::from_secs(3600), false);
    let vs = Currency::default();
    assert!(cached.markets_stored(&ids(), &vs).is_none());
    cached.markets_cached(&ids(), &vs).unwrap();
    let stored = cached.markets_stored(&ids(), &vs).unwrap();
    assert_eq!(cached.source().calls.get(), 1);
    assert!(!stored.stale);
    assert_eq!(stored.value.len(), 1);
}

#[test]
fn stored_quotes_past_the_ttl_are_flagged_stale() {
    let (_dir, cached) = setup(Duration::from_secs(0), false);
    let vs = Currency::default();
    cached.markets_cached(&ids(), &vs).unwrap();
    assert!(cached.markets_stored(&ids(), &vs).unwrap().stale);
    assert_eq!(cached.source().calls.get(), 1);
}

#[test]
fn an_outage_without_a_cache_bubbles_the_error_up() {
    let (_dir, cached) = setup(Duration::from_secs(60), true);
    cached
        .markets_cached(&ids(), &Currency::default())
        .unwrap_err();
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
        ok.markets_cached(&ids(), &vs).unwrap()
    };
    let broken = CachedSource::new(
        FakeSource::new(true),
        dir.path().to_path_buf(),
        Duration::from_secs(0),
    );
    broken.markets_cached(&ids(), &vs).unwrap();
    broken.markets_cached(&ids(), &vs).unwrap();
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
