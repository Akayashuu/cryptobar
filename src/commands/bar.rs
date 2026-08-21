//! Sous-commande `bar` : émission du module Waybar.

use std::io::Write as _;
use std::time::Duration;

use anyhow::Result;

use crate::app::App;
use crate::render::waybar;

/// Émet une ligne JSON sur la sortie standard.
///
/// # Errors
/// Échoue si l'écriture sur la sortie standard échoue. Une panne réseau ne
/// fait jamais échouer la commande tant qu'un cache existe.
pub fn run(app: &App, watch: bool) -> Result<()> {
    if watch {
        loop {
            emit(app)?;
            std::thread::sleep(Duration::from_secs(app.config.refresh_interval.max(1)));
        }
    } else {
        emit(app)
    }
}

fn emit(app: &App) -> Result<()> {
    let (quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        // Aucune donnée du tout : on affiche un module dégradé plutôt que
        // de laisser un trou dans la barre.
        Err(_) => (Vec::new(), true),
    };
    let module = waybar::render(&quotes, app.active_index(), &app.config, stale);
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", module.to_line()?)?;
    stdout.flush()?;
    Ok(())
}
