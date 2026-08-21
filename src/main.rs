//! Entry point: parse the arguments and dispatch to the commands.

use anyhow::{Context as _, Result};
use clap::Parser as _;

use cryptobar::app::App;
use cryptobar::cli::{Cli, Command};
use cryptobar::commands::{bar, chart, coins, config_cmd, nav, popup, wallet};
use cryptobar::render::waybar::Markup;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // `config` does not need a full context: it has to work even when the
    // configuration file is missing or invalid.
    if let Some(Command::Config { action }) = &cli.command {
        return config_cmd::run(action, cli.config.clone());
    }

    let app = App::new(cli.config).context("could not initialise")?;

    match cli.command.unwrap_or(Command::Bar {
        watch: false,
        markup: Markup::default(),
    }) {
        Command::Bar { watch, markup } => bar::run(app, watch, markup),
        Command::Next => nav::step(&app, 1),
        Command::Prev => nav::step(&app, -1),
        Command::Select { index } => nav::select(&app, index),
        Command::Refresh => nav::refresh(&app),
        Command::Chart {
            coin,
            range,
            window,
        } => {
            if window {
                chart::spawn_terminal(&app, coin.as_deref(), range)
            } else {
                chart::run(&app, coin.as_deref(), range)
            }
        }
        Command::Coins { term } => coins::run(&app, &term),
        Command::Popup => popup::run(app),
        Command::Portfolio => wallet::run(&app),
        Command::Config { .. } => unreachable!("handled before the context is built"),
    }
}
