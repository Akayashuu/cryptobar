//! Point d'entrée : analyse des arguments et aiguillage vers les commandes.

use anyhow::{Context as _, Result};
use clap::Parser as _;

use cryptobar::app::App;
use cryptobar::cli::{Cli, Command};
use cryptobar::commands::{bar, chart, coins, config_cmd, nav};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // `config` n'a pas besoin d'un contexte complet : elle doit fonctionner
    // même quand la configuration est absente ou invalide.
    if let Some(Command::Config { action }) = &cli.command {
        return config_cmd::run(action, cli.config.clone());
    }

    let app = App::new(cli.config).context("initialisation impossible")?;

    match cli.command.unwrap_or(Command::Bar { watch: false }) {
        Command::Bar { watch } => bar::run(&app, watch),
        Command::Next => nav::step(&app, 1),
        Command::Prev => nav::step(&app, -1),
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
        Command::Popup => anyhow::bail!("commande pas encore disponible dans cette version"),
        Command::Config { .. } => unreachable!("traité avant l'initialisation du contexte"),
    }
}
