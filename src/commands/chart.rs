//! `chart` subcommand: braille chart in the terminal.

use std::fmt::Write as _;
use std::io::Write as _;

use anyhow::{Context as _, Result, bail};

use crate::app::App;
use crate::model::{CoinId, Range, Series};
use crate::render::{braille, format};

/// Margins reserved around the plot: y axis and borders.
const AXIS_WIDTH: usize = 12;
const MIN_COLS: usize = 20;
const MIN_ROWS: usize = 4;
/// Beyond it, the chart becomes unreadable rather than more precise.
const MAX_ROWS: usize = 24;
const DEFAULT_COLS: usize = 80;
const DEFAULT_ROWS: usize = 24;

/// Prints the chart of `coin` over the requested range.
///
/// # Errors
/// Fails when the id is invalid, or when the series is unavailable and missing
/// from the cache.
pub fn run(app: &App, coin: Option<&str>, range: Range) -> Result<()> {
    let id = match coin {
        Some(raw) => CoinId::parse(raw).context("invalid cryptocurrency id")?,
        None => app
            .config
            .coins
            .get(app.active_index())
            .or_else(|| app.config.coins.first())
            .cloned()
            .context("no cryptocurrency configured")?,
    };

    let cached = app
        .source
        .series_cached(&id, range, &app.config.currency)
        .with_context(|| format!("fetching the series of {}", id.as_str()))?;

    let (cols, rows) = viewport();
    let rendered = render(&cached.value, &app.config, cols, rows, cached.stale);
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{rendered}")?;
    stdout.flush()?;
    Ok(())
}

/// Usable dimensions, with a fallback when the output is not a terminal.
fn viewport() -> (usize, usize) {
    terminal_size::terminal_size().map_or((DEFAULT_COLS, DEFAULT_ROWS), |(w, h)| {
        (usize::from(w.0), usize::from(h.0))
    })
}

/// Composes the header, the plot and the axis. Pure function: fully testable.
#[must_use]
pub fn render(
    series: &Series,
    config: &crate::config::Config,
    cols: usize,
    rows: usize,
    stale: bool,
) -> String {
    let plot_cols = cols.saturating_sub(AXIS_WIDTH).max(MIN_COLS);
    // Two header lines and one footer line are taken off the height, then the
    // height in cells is a quarter of the height in dots.
    let plot_rows = rows.saturating_sub(4).clamp(MIN_ROWS, MAX_ROWS);

    let decimals = config.decimals();
    let (min, max) = braille::bounds(&series.points);
    let lines = braille::plot(&series.points, plot_cols, plot_rows);

    let last = series.points.last().copied().unwrap_or(0.0);
    let first = series.points.first().copied().unwrap_or(last);
    let change = if first.abs() > f64::EPSILON {
        Some((last - first) / first * 100.0)
    } else {
        None
    };

    let mut out = String::new();
    // Writing into a `String` is infallible; the error is ignored knowingly
    // rather than bubbled up through the whole call chain.
    let _ = writeln!(
        out,
        "{}  {}   {}  ({})",
        series.id.as_str().to_uppercase(),
        format::price(last, &config.currency, decimals),
        format::percent(change),
        series.range.label(),
    );
    if stale {
        out.push_str("stale data — network unavailable\n");
    }
    out.push('\n');

    let label_max = format::price(max, &config.currency, decimals);
    let label_min = format::price(min, &config.currency, decimals);
    let width = label_max.chars().count().max(label_min.chars().count());
    let last_row = lines.len().saturating_sub(1);
    for (index, line) in lines.iter().enumerate() {
        let label = if index == 0 {
            label_max.clone()
        } else if index == last_row {
            label_min.clone()
        } else {
            String::new()
        };
        let _ = writeln!(out, "{label:>width$} │{line}");
    }
    out
}

/// Opens the chart in a new terminal window.
///
/// # Errors
/// Fails when the configured emulator cannot be found.
pub fn spawn_terminal(app: &App, coin: Option<&str>, range: Range) -> Result<()> {
    let terminal = app.config.terminal.trim();
    if terminal.is_empty() {
        bail!("no terminal emulator configured");
    }
    let exe = std::env::current_exe().context("executable path not found")?;
    let mut command = std::process::Command::new(terminal);
    // `-e` is accepted by foot, kitty, alacritty and wezterm.
    command
        .arg("-e")
        .arg(exe)
        .arg("chart")
        .arg("--range")
        .arg(range.slug());
    if let Some(coin) = coin {
        command.arg(coin);
    }
    command
        .spawn()
        .with_context(|| format!("launching {terminal}"))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn config() -> Config {
        Config::parse("coins = [\"bitcoin\"]").unwrap()
    }

    fn series(points: Vec<f64>) -> Series {
        Series {
            id: CoinId::parse("bitcoin").unwrap(),
            range: Range::D7,
            points,
        }
    }

    #[test]
    fn the_header_holds_the_symbol_the_price_and_the_range() {
        let out = render(&series(vec![100.0, 110.0]), &config(), 80, 20, false);
        let header = out.lines().next().unwrap();
        assert!(header.contains("BITCOIN"));
        assert!(header.contains("7d"), "header: {header}");
    }

    #[test]
    fn the_change_is_computed_between_the_first_and_last_point() {
        let out = render(&series(vec![100.0, 110.0]), &config(), 80, 20, false);
        assert!(out.lines().next().unwrap().contains("10.0"));
    }

    #[test]
    fn the_stale_state_is_reported() {
        let out = render(&series(vec![1.0, 2.0]), &config(), 80, 20, true);
        assert!(out.contains("stale"));
    }

    #[test]
    fn the_axis_carries_the_max_on_top_and_the_min_at_the_bottom() {
        let out = render(&series(vec![10.0, 50.0]), &config(), 80, 20, false);
        let plot: Vec<&str> = out.lines().filter(|l| l.contains('│')).collect();
        assert!(plot.first().unwrap().contains("50"));
        assert!(plot.last().unwrap().contains("10"));
    }

    #[test]
    fn a_tiny_terminal_yields_no_zero_dimension() {
        let out = render(&series(vec![1.0, 2.0, 3.0]), &config(), 1, 1, false);
        assert!(out.contains('│'));
    }

    #[test]
    fn an_empty_series_does_not_panic() {
        let out = render(&series(Vec::new()), &config(), 80, 20, false);
        assert_ne!(out.len(), 0);
    }

    #[test]
    fn a_series_starting_at_zero_announces_no_change() {
        let out = render(&series(vec![0.0, 5.0]), &config(), 80, 20, false);
        assert!(out.lines().next().unwrap().contains('—'));
    }
}
