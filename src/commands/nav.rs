//! Sous-commandes de navigation : `next`, `prev` et `refresh`.

use anyhow::Result;

use crate::app::App;

/// Déplace la sélection de `delta` positions puis réveille Waybar.
///
/// # Errors
/// Échoue si l'état ne peut pas être écrit.
pub fn step(app: &App, delta: isize) -> Result<()> {
    let mut state = app.state();
    // On part de l'indice effectif : après une rotation automatique, « suivant »
    // doit suivre ce que l'utilisateur voit, pas l'indice figé sur disque.
    state.select(app.active_index(), app.config.coins.len());
    state.step(delta, app.config.coins.len());
    app.save_state(&state)?;
    app.notify();
    Ok(())
}

/// Fige la sélection sur un indice explicite puis réveille Waybar.
///
/// # Errors
/// Échoue si l'état ne peut pas être écrit.
pub fn select(app: &App, index: usize) -> Result<()> {
    let mut state = app.state();
    state.select(index, app.config.coins.len());
    app.save_state(&state)?;
    app.notify();
    Ok(())
}

/// Vide le cache pour forcer une requête au prochain affichage.
///
/// # Errors
/// Échoue si les fichiers de cache ne peuvent pas être supprimés.
pub fn refresh(app: &App) -> Result<()> {
    app.source.invalidate()?;
    app.notify();
    Ok(())
}
