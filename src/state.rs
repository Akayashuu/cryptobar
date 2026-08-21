//! État d'exécution partagé entre les invocations ponctuelles du binaire.
//!
//! En mode `exec`, chaque tick Waybar lance un nouveau processus : l'indice de
//! la cryptomonnaie affichée doit donc vivre sur disque. Il est stocké dans
//! `$XDG_RUNTIME_DIR`, volatile par nature — le perdre au redémarrage est
//! sans conséquence.

use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// État persistant minimal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Indice de la cryptomonnaie affichée dans la liste configurée.
    pub index: usize,
    /// Instant du dernier changement d'indice, pour la rotation automatique.
    pub rotated_at: u64,
    /// Indice figé par une action manuelle : la rotation automatique reprend
    /// après `manual_hold` secondes.
    pub manual: bool,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl State {
    /// Lit l'état, ou retourne l'état par défaut si le fichier est absent ou illisible.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Écrit l'état de façon atomique.
    ///
    /// # Errors
    /// Remonte les erreurs d'E/S sur la création du répertoire ou l'écriture.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            crate::paths::ensure_dir(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        {
            let mut file = File::create(&tmp)?;
            let bytes = serde_json::to_vec(self)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, path)
    }

    /// Avance de `delta` positions dans une liste de `len` éléments et fige la
    /// sélection. `delta` peut être négatif.
    pub fn step(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.index = 0;
            return;
        }
        let len_i = isize::try_from(len).unwrap_or(isize::MAX);
        let current = isize::try_from(self.index).unwrap_or(0);
        // `rem_euclid` avec un modulo strictement positif : toujours dans 0..len.
        let next = current.saturating_add(delta).rem_euclid(len_i);
        self.index = usize::try_from(next).unwrap_or(0);
        self.rotated_at = now();
        self.manual = true;
    }

    /// Sélectionne un indice explicite et fige la sélection.
    pub fn select(&mut self, index: usize, len: usize) {
        self.index = if len == 0 {
            0
        } else {
            index.min(len.saturating_sub(1))
        };
        self.rotated_at = now();
        self.manual = true;
    }

    /// Indice à afficher maintenant, rotation automatique comprise.
    ///
    /// `rotate_every` à zéro désactive la rotation. `manual_hold` est la durée
    /// pendant laquelle une sélection manuelle reste figée.
    #[must_use]
    pub fn effective_index(&self, len: usize, rotate_every: u64, manual_hold: u64) -> usize {
        if len == 0 {
            return 0;
        }
        // `checked_rem` évite toute arithmétique non prouvée : `len` est non nul ici,
        // mais on laisse le compilateur le vérifier plutôt que de l'affirmer.
        let index = self.index.checked_rem(len).unwrap_or(0);
        if rotate_every == 0 {
            return index;
        }
        let elapsed = now().saturating_sub(self.rotated_at);
        if self.manual && elapsed < manual_hold {
            return index;
        }
        // La rotation est dérivée du temps écoulé : aucun état à réécrire à
        // chaque tick, ce qui garde le mode `exec` sans effet de bord.
        let steps = elapsed.checked_div(rotate_every).unwrap_or(0);
        let len_u64 = u64::try_from(len).unwrap_or(u64::MAX);
        let wrapped = steps.checked_rem(len_u64).unwrap_or(0);
        let offset = usize::try_from(wrapped).unwrap_or(0);
        index.saturating_add(offset).checked_rem(len).unwrap_or(0)
    }

    /// Chemin par défaut du fichier d'état.
    ///
    /// # Errors
    /// Remonte l'erreur de résolution du répertoire d'exécution.
    pub fn default_path() -> anyhow::Result<PathBuf> {
        Ok(crate::paths::runtime_dir()?.join("state.json"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn step_avance_et_boucle() {
        let mut state = State::default();
        state.step(1, 3);
        assert_eq!(state.index, 1);
        state.step(2, 3);
        assert_eq!(state.index, 0);
    }

    #[test]
    fn step_negatif_boucle_par_la_fin() {
        let mut state = State::default();
        state.step(-1, 3);
        assert_eq!(state.index, 2);
    }

    #[test]
    fn step_sur_liste_vide_ne_panique_pas() {
        let mut state = State {
            index: 5,
            ..State::default()
        };
        state.step(1, 0);
        assert_eq!(state.index, 0);
    }

    #[test]
    fn select_borne_lindice() {
        let mut state = State::default();
        state.select(99, 3);
        assert_eq!(state.index, 2);
        assert!(state.manual);
    }

    #[test]
    fn rotation_desactivee_rend_lindice_brut() {
        let state = State {
            index: 1,
            rotated_at: 0,
            manual: false,
        };
        assert_eq!(state.effective_index(3, 0, 60), 1);
    }

    #[test]
    fn une_selection_manuelle_recente_fige_la_rotation() {
        let mut state = State::default();
        state.step(1, 3);
        assert_eq!(state.effective_index(3, 1, 3600), 1);
    }

    #[test]
    fn la_rotation_automatique_derive_du_temps_ecoule() {
        // `rotated_at` très ancien : la rotation a eu le temps de tourner.
        let state = State {
            index: 0,
            rotated_at: 0,
            manual: false,
        };
        let rotated = state.effective_index(3, 5, 60);
        assert!(rotated < 3);
    }

    #[test]
    fn effective_index_borne_un_indice_hors_liste() {
        let state = State {
            index: 42,
            rotated_at: now(),
            manual: true,
        };
        assert_eq!(state.effective_index(3, 0, 60), 0);
    }

    #[test]
    fn sauvegarde_et_relecture_conservent_letat() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("state.json");
        let state = State {
            index: 2,
            rotated_at: 123,
            manual: true,
        };
        state.save(&path).unwrap();
        assert_eq!(State::load(&path), state);
    }

    #[test]
    fn un_etat_absent_ou_corrompu_retombe_sur_le_defaut() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        assert_eq!(State::load(&path), State::default());
        std::fs::write(&path, b"nawak").unwrap();
        assert_eq!(State::load(&path), State::default());
    }
}
