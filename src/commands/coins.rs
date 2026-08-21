//! `coins` subcommand: searching CoinGecko ids.
//!
//! The configuration file expects ids, not symbols. This command bridges the
//! gap for the user.

use std::fmt::Write as _;
use std::io::Write as _;

use anyhow::{Context as _, Result};

use crate::app::App;
use crate::source::SearchHit;

/// Number of results displayed: beyond it, the list stops being useful.
const MAX_RESULTS: usize = 15;

/// Searches `term` and prints the matching ids.
///
/// # Errors
/// Fails when the network search fails — it is not cached.
pub fn run(app: &App, term: &str) -> Result<()> {
    let hits = app.source.search(term).context("search failed")?;
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{}", render(&hits, term))?;
    stdout.flush()?;
    Ok(())
}

/// Formats the results. Pure function: testable without network.
#[must_use]
pub fn render(hits: &[SearchHit], term: &str) -> String {
    if hits.is_empty() {
        return format!("no result for \"{term}\"\n");
    }
    let shown: Vec<&SearchHit> = hits.iter().take(MAX_RESULTS).collect();
    let id_width = shown
        .iter()
        .map(|h| h.id.as_str().chars().count())
        .max()
        .unwrap_or(0);
    let symbol_width = shown
        .iter()
        .map(|h| h.symbol.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for hit in &shown {
        // Writing into a `String` is infallible.
        let _ = writeln!(
            out,
            "{:<id_width$}  {:<symbol_width$}  {}",
            hit.id.as_str(),
            hit.symbol.to_uppercase(),
            hit.name,
        );
    }
    if hits.len() > shown.len() {
        let _ = writeln!(
            out,
            "… {} more results",
            hits.len().saturating_sub(shown.len())
        );
    }
    out.push_str("\nAdd the id from the first column to `coins` in the configuration.\n");
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::CoinId;

    fn hit(id: &str, symbol: &str, name: &str) -> SearchHit {
        SearchHit {
            image: None,
            icon: None,
            id: CoinId::parse(id).unwrap(),
            symbol: symbol.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn an_empty_search_says_so_explicitly() {
        let out = render(&[], "zzz");
        assert!(out.contains("no result"));
        assert!(out.contains("zzz"));
    }

    #[test]
    fn every_result_shows_id_symbol_and_name() {
        let out = render(&[hit("bitcoin", "btc", "Bitcoin")], "btc");
        let first = out.lines().next().unwrap();
        assert!(first.contains("bitcoin"));
        assert!(first.contains("BTC"));
        assert!(first.contains("Bitcoin"));
    }

    #[test]
    fn the_columns_are_aligned() {
        let hits = vec![
            hit("btc", "btc", "Bitcoin"),
            hit("ethereum", "eth", "Ethereum"),
        ];
        let out = render(&hits, "b");
        let positions: Vec<_> = out
            .lines()
            .take(2)
            .map(|l| l.find("BTC").or(l.find("ETH")))
            .collect();
        assert_eq!(positions.first(), positions.get(1));
    }

    #[test]
    fn the_list_is_truncated_and_the_rest_announced() {
        let hits: Vec<SearchHit> = (0..MAX_RESULTS + 5)
            .map(|_| hit("bitcoin", "btc", "B"))
            .collect();
        let out = render(&hits, "b");
        assert!(out.contains("5 more results"));
    }
}
