//! Sous-commande `bar` : émission du module Waybar.

use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;

use crate::app::App;
use crate::render::waybar::{self, Markup};

/// Pas de la boucle d'attente en mode suivi. Deux `stat` par pas : le coût est
/// invisible, et c'est ce qui rend un clic ou un réglage instantané dans la
/// barre au lieu d'attendre le prochain intervalle.
const POLL: Duration = Duration::from_millis(400);

/// Émet une ligne JSON sur la sortie standard.
///
/// # Errors
/// Échoue si l'écriture sur la sortie standard échoue. Une panne réseau ne
/// fait jamais échouer la commande tant qu'un cache existe.
pub fn run(app: App, watch: bool, markup: Markup) -> Result<()> {
    if watch {
        watch_loop(app, markup)
    } else {
        emit(&app, markup, false)
    }
}

/// Réémet à chaque intervalle, et sans attendre dès que la sélection change ou
/// que la configuration est modifiée — par la fenêtre graphique, par exemple.
fn watch_loop(mut app: App, markup: Markup) -> Result<()> {
    let state_path = crate::state::State::default_path()?;
    let mut config_stamp = stamp(app.config_path());
    let mut state_stamp = stamp(&state_path);

    loop {
        // Le mode suivi n'est pas dans le chemin critique d'un tick : c'est
        // le bon endroit pour compléter le cache de logos.
        emit(&app, markup, true)?;

        let deadline =
            Instant::now().checked_add(Duration::from_secs(app.config.refresh_interval.max(1)));
        let mut config_changed = false;
        loop {
            std::thread::sleep(POLL);
            let now_config = stamp(app.config_path());
            let now_state = stamp(&state_path);
            if now_config != config_stamp {
                config_stamp = now_config;
                config_changed = true;
                break;
            }
            if now_state != state_stamp {
                state_stamp = now_state;
                break;
            }
            if deadline.is_none_or(|deadline| Instant::now() >= deadline) {
                break;
            }
        }

        if config_changed {
            // Une configuration devenue invalide ne doit pas tuer la barre :
            // on continue avec la dernière version connue et l'utilisateur
            // voit son erreur au prochain enregistrement.
            if let Ok(reloaded) = app.reload() {
                app = reloaded;
            }
        }
    }
}

/// Empreinte de modification d'un fichier. `None` couvre aussi bien l'absence
/// que l'illisibilité : dans les deux cas, rien à signaler tant que ça ne change pas.
fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

/// Notifie les alertes nouvellement franchies et mémorise l'ensemble courant.
///
/// N'est appelé qu'en mode suivi : un tick `exec` ponctuel ne doit pas faire
/// sonner une alerte que le mode suivi vient déjà de traiter, et une barre qui
/// tourne en `exec` toutes les secondes ne doit pas transformer un
/// franchissement en avalanche de bulles.
fn dispatch_alerts(app: &App, quotes: &[crate::model::Quote]) {
    if app.config.alerts.is_empty() {
        return;
    }
    let triggers = crate::alerts::evaluate(&app.config, quotes);
    let mut state = app.state();
    let (fresh, current) = crate::alerts::diff(&triggers, &state.fired);
    for trigger in &fresh {
        crate::alerts::notify(trigger);
    }
    if current != state.fired {
        state.fired = current;
        // L'échec d'écriture n'est pas fatal : au pire une alerte est
        // renotifiée au prochain tick.
        let _ = app.save_state(&state);
    }
}

/// `live` distingue le mode suivi du tick ponctuel : lui seul complète le
/// cache de logos et fait sonner les alertes.
fn emit(app: &App, markup: Markup, live: bool) -> Result<()> {
    let (mut quotes, stale) = match app
        .source
        .markets_cached(&app.config.coins, &app.config.currency)
    {
        Ok(cached) => (cached.value, cached.stale),
        // Aucune donnée du tout : on affiche un module dégradé plutôt que
        // de laisser un trou dans la barre.
        Err(_) => (Vec::new(), true),
    };
    if live {
        app.fetch_icons(&mut quotes);
        dispatch_alerts(app, &quotes);
    } else {
        app.attach_icons(&mut quotes);
    }
    // En mode suivi, le processus vit assez longtemps pour interroger les
    // chaînes ; un tick ponctuel se contente du cache, pour rester instantané.
    let watcher = app.watcher();
    let balances = if live {
        watcher.refresh(&app.config.wallets)
    } else {
        watcher.cached(&app.config.wallets)
    };
    let onchain = crate::onchain::totals(&balances);
    let module = waybar::render(
        &quotes,
        app.active_index(),
        &app.config,
        stale,
        markup,
        &onchain,
    );
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", module.to_line()?)?;
    stdout.flush()?;
    Ok(())
}
