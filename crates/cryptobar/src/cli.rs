//! Command line interface definition.

use clap::{Parser, Subcommand};

use crate::model::Range;
use crate::render::waybar::Markup;

/// Cryptocurrency prices for your bar, terminal and desktop.
#[derive(Debug, Parser)]
#[command(name = "cryptobar", version, about, long_about = None)]
pub struct Cli {
    /// Path to an alternative configuration file.
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Emit a JSON line for a Waybar module (default behaviour).
    Bar {
        /// Stay in the foreground and emit again on every interval.
        #[arg(long)]
        watch: bool,
        /// Text markup: `pango` for Waybar, `none` for Quickshell, Ags, Eww
        /// and any bar that displays plain text.
        #[arg(long, default_value = "pango", value_name = "STYLE")]
        markup: Markup,
    },
    /// Move to the next cryptocurrency.
    Next,
    /// Move back to the previous cryptocurrency.
    Prev,
    /// Show the cryptocurrency at the given index in the configured list.
    Select {
        /// Zero-based index.
        index: usize,
    },
    /// Force an immediate refresh of the data.
    Refresh,
    /// Draw a chart in the terminal.
    Chart {
        /// CoinGecko id; defaults to the cryptocurrency currently displayed.
        coin: Option<String>,
        /// Time range: 1h, 24h, 7d, 30d, 1y.
        #[arg(long, default_value = "7d")]
        range: Range,
        /// Open the chart in a new terminal window.
        #[arg(long)]
        window: bool,
    },
    /// Open the graphical window.
    Popup,
    /// Value the balances of the watched public addresses.
    Portfolio,
    /// Search a cryptocurrency by name or symbol.
    Coins {
        /// Search term.
        term: String,
    },
    /// Manage the configuration file.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Write a commented configuration file if none exists.
    Init {
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Print the path of the configuration file.
    Path,
    /// Check that the configuration is valid.
    Check,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    #[test]
    fn the_clap_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn clap_validates_the_bar_markup() {
        assert!(Cli::try_parse_from(["cryptobar", "bar", "--markup", "bold"]).is_err());
        let cli = Cli::try_parse_from(["cryptobar", "bar", "--markup", "none"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Bar {
                markup: Markup::None,
                ..
            })
        ));
    }

    #[test]
    fn without_a_subcommand_the_field_is_empty() {
        let cli = Cli::try_parse_from(["cryptobar"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn the_chart_range_is_validated() {
        assert!(Cli::try_parse_from(["cryptobar", "chart", "--range", "42x"]).is_err());
        let cli = Cli::try_parse_from(["cryptobar", "chart", "--range", "24h"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Chart {
                range: Range::H24,
                ..
            })
        ));
    }
}
