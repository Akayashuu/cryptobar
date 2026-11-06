//! Runtime state shared across the one-shot invocations of the binary.
//!
//! In `exec` mode every Waybar tick spawns a new process: the index of the
//! displayed cryptocurrency therefore has to live on disk. It is stored under
//! `$XDG_RUNTIME_DIR`, volatile by nature, losing it on reboot is harmless.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::{self, PathsError};

/// Failures of the runtime state file.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// The XDG directories could not be resolved.
    #[error("could not locate the runtime directory")]
    Paths(#[from] PathsError),
    /// The state directory could not be created.
    #[error("could not create the state directory {}", path.display())]
    Directory {
        /// Directory that could not be created.
        path: PathBuf,
        /// Why the directory could not be prepared.
        source: PathsError,
    },
    /// The state file could not be read. The commands carry on from the default
    /// state and leave the file alone.
    #[error("could not read the state file {}, starting from the default state", path.display())]
    Read {
        /// State file that could not be read.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The state file exists but does not parse. Commands report it once, then
    /// replace the file with the default state: the state is a volatile cache.
    #[error("the state file {} is corrupt, starting from the default state", path.display())]
    Corrupt {
        /// State file that does not parse.
        path: PathBuf,
        /// The parse failure, with its position in the file.
        #[source]
        source: serde_json::Error,
    },
    /// The state file could not be written.
    #[error("could not write the state file {}", path.display())]
    Write {
        /// State file that could not be written.
        path: PathBuf,
        /// The operating system error.
        source: io::Error,
    },
    /// The state could not be encoded as JSON.
    #[error("could not encode the state")]
    Encode(#[from] serde_json::Error),
}

/// A state read from disk, with the failures met on the way.
#[derive(Debug)]
pub struct LoadedState {
    /// The state to use: the stored one, or the default one when the file was
    /// corrupt or cannot be read.
    pub state: State,
    /// Failures to report once: a corrupt file that was replaced, a replacement
    /// that failed, or a file that could not be read.
    pub notices: Vec<StateError>,
    /// Whether a save may overwrite the file the state was read from.
    pub access: StateAccess,
}

impl LoadedState {
    /// Whether the state may be saved over the file it was read from.
    #[must_use]
    pub fn is_writable(&self) -> bool {
        self.access == StateAccess::Writable
    }
}

/// Whether a save may overwrite the state file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateAccess {
    /// The file holds the state, or was just replaced by the default one.
    Writable,
    /// The file could not be read: it is left as it is.
    Unreadable,
}

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
    pub fired: BTreeSet<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl State {
    /// Reads the state. An absent file is the default state.
    ///
    /// # Errors
    /// Fails with [`StateError::Corrupt`] when the file does not parse, so the
    /// caller decides how to recover, and with [`StateError::Read`] when the
    /// file cannot be read.
    pub fn load(path: &Path) -> Result<Self, StateError> {
        match fs::read_to_string(path) {
            Ok(raw) => match serde_json::from_str(&raw) {
                Ok(state) => Ok(state),
                Err(source) => Err(StateError::Corrupt {
                    path: path.to_path_buf(),
                    source,
                }),
            },
            Err(absent) if absent.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(failure) => Err(StateError::Read {
                path: path.to_path_buf(),
                source: failure,
            }),
        }
    }

    /// Writes the state atomically.
    ///
    /// # Errors
    /// Fails when the directory cannot be created, the state cannot be encoded,
    /// or the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        if let Some(parent) = path.parent() {
            paths::ensure_dir(parent).map_err(|source| StateError::Directory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(self)?;
        let written = write_synced(&tmp, &bytes).and_then(|()| fs::rename(&tmp, path));
        written.map_err(|source| StateError::Write {
            path: path.to_path_buf(),
            source,
        })
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
        let index = self.index.checked_rem(len).unwrap_or(0);
        if rotate_every == 0 {
            return index;
        }
        let elapsed = now().saturating_sub(self.rotated_at);
        if self.manual && elapsed < manual_hold {
            return index;
        }
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
    pub fn default_path() -> Result<PathBuf, StateError> {
        Ok(paths::runtime_dir()?.join("state.json"))
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use std::error::Error;

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
            fired: BTreeSet::new(),
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
        let state = State {
            index: 0,
            rotated_at: 0,
            manual: false,
            fired: BTreeSet::new(),
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
            fired: BTreeSet::new(),
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
            fired: BTreeSet::new(),
        };
        state.save(&path).unwrap();
        assert_eq!(State::load(&path).unwrap(), state);
    }

    #[test]
    fn a_missing_state_is_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        assert_eq!(State::load(&path).unwrap(), State::default());
    }

    #[test]
    fn a_corrupt_state_is_an_error_and_left_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, b"garbage").unwrap();
        assert!(matches!(
            State::load(&path),
            Err(StateError::Corrupt { .. })
        ));
        assert_eq!(fs::read(&path).unwrap(), b"garbage");
    }

    #[test]
    fn a_corrupt_state_keeps_the_parse_position_as_its_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, b"garbage").unwrap();
        let failure = State::load(&path).unwrap_err();
        assert!(
            Error::source(&failure)
                .is_some_and(|cause| cause.to_string().contains("line 1 column 1"))
        );
    }
}
