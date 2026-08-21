//! Point d'entrée : analyse des arguments et aiguillage vers les commandes.

use anyhow::{Context as _, Result};
use clap::Parser as _;

use cryptobar::app::App;
use cryptobar::cli::{Cli, Command, ConfigAction};
use cryptobar::commands::{bar, chart, nav};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // `config` n'a pas besoin d'un contexte complet : elle doit fonctionner
    // même quand la configuration est absente ou invalide.
    if let Some(Command::Config { action }) = &cli.command {
        return run_config(action, cli.config.clone());
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
        Command::Popup | Command::Coins { .. } => {
            anyhow::bail!("commande pas encore disponible dans cette version")
        }
        Command::Config { .. } => unreachable!("traité avant l'initialisation du contexte"),
    }
}

fn run_config(action: &ConfigAction, override_path: Option<std::path::PathBuf>) -> Result<()> {
    let path = match override_path {
        Some(path) => path,
        None => cryptobar::paths::config_file()?,
    };
    match action {
        ConfigAction::Path => println!("{}", path.display()),
        ConfigAction::Init { force } => {
            cryptobar::config::Config::write_default(&path, *force)?;
            println!("configuration écrite dans {}", path.display());
        }
        ConfigAction::Check => {
            cryptobar::config::Config::load_from(&path)?;
            println!("configuration valide : {}", path.display());
        }
    }
    Ok(())
}
