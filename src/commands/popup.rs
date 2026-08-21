//! Sous-commande `popup` : ouverture de la fenêtre graphique.

use anyhow::Result;

use crate::app::App;

/// Ouvre la fenêtre à partir des données en cache.
///
/// # Errors
/// Échoue si GTK ne peut pas s'initialiser.
#[cfg(feature = "popup")]
pub fn run(app: &App) -> Result<()> {
    let (quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        Err(_) => (Vec::new(), true),
    };
    crate::ui::popup::show(quotes, app.active_index(), &app.config, stale)
}

/// Variante compilée sans GTK : la commande existe mais explique son absence.
///
/// # Errors
/// Échoue systématiquement.
#[cfg(not(feature = "popup"))]
pub fn run(_app: &App) -> Result<()> {
    anyhow::bail!("binaire compilé sans la fonctionnalité `popup`")
}
