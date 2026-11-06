//! Runtime state shared across the one-shot invocations of the binary.
//!
//! In `exec` mode every Waybar tick spawns a new process: the index of the
//! displayed cryptocurrency therefore has to live on disk. It is stored under
//! `$XDG_RUNTIME_DIR`, volatile by nature — losing it on reboot is harmless.

use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Minimal persistent state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Index of the displayed cryptocurrency in the configured list.
    pub index: usize,
    /// Time of the last index change, for the automatic rotation.
    pub rotated_at: u64,
    /// Index pinned by a manual action: the automatic rotation resumes after
    /// `manual_hold` seconds.
    pub manual: bool,
    /// Alert conditions already notified. A key disappears as soon as its
    /// condition becomes false again, which re-arms the alert.
    pub fired: std::collections::BTreeSet<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl State {
    /// Reads the state, or returns the default when the file is missing or unreadable.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Writes the state atomically.
    ///
    /// # Errors
    /// Propagates the I/O errors of the directory creation or of the write.
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

    /// Moves `delta` positions through a list of `len` items and pins the
    /// selection. `delta` may be negative.
    pub fn step(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.index = 0;
            return;
        }
        let len_i = isize::try_from(len).unwrap_or(isize::MAX);
        let current = isize::try_from(self.index).unwrap_or(0);
        // `rem_euclid` with a strictly positive modulus: always within 0..len.
        let next = current.saturating_add(delta).rem_euclid(len_i);
        self.index = usize::try_from(next).unwrap_or(0);
        self.rotated_at = now();
        self.manual = true;
    }

    /// Selects an explicit index and pins the selection.
    pub fn select(&mut self, index: usize, len: usize) {
        self.index = if len == 0 {
            0
        } else {
            index.min(len.saturating_sub(1))
        };
        self.rotated_at = now();
        self.manual = true;
    }

    /// Index to display right now, automatic rotation included.
    ///
    /// A `rotate_every` of zero disables the rotation. `manual_hold` is how long
    /// a manual selection stays pinned.
    #[must_use]
    pub fn effective_index(&self, len: usize, rotate_every: u64, manual_hold: u64) -> usize {
        if len == 0 {
            return 0;
        }
        // `checked_rem` avoids any unproven arithmetic: `len` is non-zero here,
        // but let the compiler check it rather than asserting it.
        let index = self.index.checked_rem(len).unwrap_or(0);
        if rotate_every == 0 {
            return index;
        }
        let elapsed = now().saturating_sub(self.rotated_at);
        if self.manual && elapsed < manual_hold {
            return index;
        }
        // The rotation is derived from the elapsed time: no state to rewrite on
        // every tick, which keeps `exec` mode free of side effects.
        let steps = elapsed.checked_div(rotate_every).unwrap_or(0);
        let len_u64 = u64::try_from(len).unwrap_or(u64::MAX);
        let wrapped = steps.checked_rem(len_u64).unwrap_or(0);
        let offset = usize::try_from(wrapped).unwrap_or(0);
        index.saturating_add(offset).checked_rem(len).unwrap_or(0)
    }

    /// Default path of the state file.
    ///
    /// # Errors
    /// Propagates the runtime directory resolution error.
    pub fn default_path() -> anyhow::Result<PathBuf> {
        Ok(crate::paths::runtime_dir()?.join("state.json"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn step_advances_and_wraps() {
        let mut state = State::default();
        state.step(1, 3);
        assert_eq!(state.index, 1);
        state.step(2, 3);
        assert_eq!(state.index, 0);
    }

    #[test]
    fn a_negative_step_wraps_through_the_end() {
        let mut state = State::default();
        state.step(-1, 3);
        assert_eq!(state.index, 2);
    }

    #[test]
    fn step_on_an_empty_list_does_not_panic() {
        let mut state = State {
            index: 5,
            ..State::default()
        };
        state.step(1, 0);
        assert_eq!(state.index, 0);
    }

    #[test]
    fn select_clamps_the_index() {
        let mut state = State::default();
        state.select(99, 3);
        assert_eq!(state.index, 2);
        assert!(state.manual);
    }

    #[test]
    fn a_disabled_rotation_yields_the_raw_index() {
        let state = State {
            index: 1,
            rotated_at: 0,
            manual: false,
            fired: std::collections::BTreeSet::new(),
        };
        assert_eq!(state.effective_index(3, 0, 60), 1);
    }

    #[test]
    fn a_recent_manual_selection_pins_the_rotation() {
        let mut state = State::default();
        state.step(1, 3);
        assert_eq!(state.effective_index(3, 1, 3600), 1);
    }

    #[test]
    fn the_automatic_rotation_derives_from_the_elapsed_time() {
        // Very old `rotated_at`: the rotation had time to advance.
        let state = State {
            index: 0,
            rotated_at: 0,
            manual: false,
            fired: std::collections::BTreeSet::new(),
        };
        let rotated = state.effective_index(3, 5, 60);
        assert!(rotated < 3);
    }

    #[test]
    fn effective_index_clamps_an_out_of_list_index() {
        let state = State {
            index: 42,
            rotated_at: now(),
            manual: true,
            fired: std::collections::BTreeSet::new(),
        };
        assert_eq!(state.effective_index(3, 0, 60), 0);
    }

    #[test]
    fn saving_and_reloading_preserve_the_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("state.json");
        let state = State {
            index: 2,
            rotated_at: 123,
            manual: true,
            fired: std::collections::BTreeSet::new(),
        };
        state.save(&path).unwrap();
        assert_eq!(State::load(&path), state);
    }

    #[test]
    fn a_missing_or_corrupt_state_falls_back_to_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        assert_eq!(State::load(&path), State::default());
        std::fs::write(&path, b"garbage").unwrap();
        assert_eq!(State::load(&path), State::default());
    }
}
