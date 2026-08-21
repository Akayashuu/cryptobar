//! Sending the real-time signal that forces Waybar to refresh the module.
//!
//! Waybar listens on `SIGRTMIN+n` for a module configured with `"signal": n`.
//! The PID is resolved by reading `/proc` and the signal is sent through
//! `rustix`, without going through a shell or `pkill`.

use std::io;

use rustix::process::{Pid, Signal};
use rustix_libc_wrappers::process::SignalExt as _;

/// Signal delivery errors.
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("no waybar process found")]
    NotFound,
    #[error("invalid signal number: SIGRTMIN+{0}")]
    InvalidSignal(u8),
    #[error("could not read /proc: {0}")]
    Proc(#[from] io::Error),
}

/// Lists the PIDs of the processes whose `comm` equals `name`.
fn pids_by_name(name: &str) -> io::Result<Vec<i32>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc")?.flatten() {
        let file_name = entry.file_name();
        let Some(pid) = file_name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        // `comm` is truncated to 15 characters by the kernel; "waybar" fits.
        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        if comm.trim() == name {
            found.push(pid);
        }
    }
    Ok(found)
}

/// Sends `SIGRTMIN+offset` to every Waybar instance.
///
/// Returns the number of processes actually signalled.
///
/// # Errors
/// [`SignalError::NotFound`] when no instance is running,
/// [`SignalError::InvalidSignal`] when `offset` falls outside the real-time range.
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
        // A one-off failure (process gone in the meantime) must not prevent
        // signalling the other instances.
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
    fn an_out_of_range_offset_is_rejected() {
        assert!(matches!(
            notify_waybar(u8::MAX),
            Err(SignalError::InvalidSignal(_))
        ));
    }

    #[test]
    fn valid_offsets_are_accepted_by_rustix() {
        // The 1..=30 range is the one allowed by the configuration validation.
        for offset in 1_i32..=30 {
            assert!(Signal::rt(offset).is_some());
        }
    }

    #[test]
    fn the_current_process_is_visible_in_proc() {
        // Exercises the /proc reading machinery without depending on Waybar.
        let comm = std::fs::read_to_string("/proc/self/comm").unwrap();
        let pids = pids_by_name(comm.trim()).unwrap();
        assert!(pids.contains(&std::process::id().cast_signed()));
    }
}
