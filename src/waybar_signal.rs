//! Envoi du signal temps réel qui force Waybar à rafraîchir le module.
//!
//! Waybar écoute `SIGRTMIN+n` pour un module configuré avec `"signal": n`.
//! On résout le PID en lisant `/proc` et on envoie le signal via `rustix`,
//! sans passer par un shell ni par `pkill`.

use std::io;

use rustix::process::{Pid, Signal};
use rustix_libc_wrappers::process::SignalExt as _;

/// Erreurs d'envoi du signal.
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("aucun processus waybar trouvé")]
    NotFound,
    #[error("numéro de signal invalide : SIGRTMIN+{0}")]
    InvalidSignal(u8),
    #[error("lecture de /proc impossible : {0}")]
    Proc(#[from] io::Error),
}

/// Liste les PID des processus dont le `comm` vaut `name`.
fn pids_by_name(name: &str) -> io::Result<Vec<i32>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc")?.flatten() {
        let file_name = entry.file_name();
        let Some(pid) = file_name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        // `comm` est tronqué à 15 caractères par le noyau ; « waybar » y tient.
        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        if comm.trim() == name {
            found.push(pid);
        }
    }
    Ok(found)
}

/// Envoie `SIGRTMIN+offset` à toutes les instances de Waybar.
///
/// Retourne le nombre de processus effectivement signalés.
///
/// # Errors
/// [`SignalError::NotFound`] si aucune instance ne tourne,
/// [`SignalError::InvalidSignal`] si `offset` sort de la plage temps réel.
pub fn notify_waybar(offset: u8) -> Result<usize, SignalError> {
    let signal = Signal::rt(i32::from(offset)).ok_or(SignalError::InvalidSignal(offset))?;

    let pids = pids_by_name("waybar")?;
    if pids.is_empty() {
        return Err(SignalError::NotFound);
    }

    let mut sent = 0usize;
    for raw in pids {
        let Some(pid) = Pid::from_raw(raw) else {
            continue;
        };
        // Un échec ponctuel (processus disparu entre-temps) ne doit pas
        // empêcher de signaler les autres instances.
        if rustix::process::kill_process(pid, signal).is_ok() {
            sent = sent.saturating_add(1);
        }
    }
    if sent == 0 {
        Err(SignalError::NotFound)
    } else {
        Ok(sent)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn un_offset_hors_plage_est_rejete() {
        assert!(matches!(
            notify_waybar(u8::MAX),
            Err(SignalError::InvalidSignal(_))
        ));
    }

    #[test]
    fn les_offsets_valides_sont_acceptes_par_rustix() {
        // La plage 1..=30 est celle autorisée par la validation de configuration.
        for offset in 1_i32..=30 {
            assert!(Signal::rt(offset).is_some());
        }
    }

    #[test]
    fn le_processus_courant_est_visible_dans_proc() {
        // Valide la mécanique de lecture de /proc sans dépendre de Waybar.
        let comm = std::fs::read_to_string("/proc/self/comm").unwrap();
        let pids = pids_by_name(comm.trim()).unwrap();
        assert!(pids.contains(&std::process::id().cast_signed()));
    }
}
