//! Sending the real-time signal that forces Waybar to refresh the module.
//!
//! Waybar listens on `SIGRTMIN+n` for a module configured with `"signal": n`.
//! The PID is resolved by reading `/proc` and the signal is sent through
//! `rustix`, without going through a shell or `pkill`.

use std::fs;
use std::io;

use rustix::io::Errno;
use rustix::process::{self, Pid, Signal};
use rustix_libc_wrappers::process::SignalExt as _;

/// Signal delivery errors.
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    /// No Waybar process is running: the normal state of a desktop without Waybar.
    #[error("no waybar process is running")]
    NotRunning,
    /// The kernel refused to signal a Waybar process, for instance one owned by another user.
    #[error("the signal to waybar process {pid} was refused")]
    Refused {
        /// Process that refused the signal.
        pid: i32,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The signal number is outside `1..=30`.
    #[error("invalid signal number: SIGRTMIN+{0}")]
    InvalidSignal(u8),
    /// The process table could not be read.
    #[error("could not read /proc: {0}")]
    Proc(#[from] io::Error),
}

fn pids_by_name(name: &str) -> io::Result<Vec<i32>> {
    let mut found = Vec::new();
    for entry in fs::read_dir("/proc")?.flatten() {
        let file_name = entry.file_name();
        let Some(pid) = file_name.to_str().and_then(|text| text.parse::<i32>().ok()) else {
            continue;
        };
        let Ok(comm) = fs::read_to_string(entry.path().join("comm")) else {
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
/// Returns the number of processes signalled. A process that exited between the
/// scan and the signal is not a refusal and is not counted.
///
/// # Errors
/// [`SignalError::NotRunning`] when no instance is running or all of them
/// exited, [`SignalError::Refused`] when the kernel refused every signal,
/// [`SignalError::InvalidSignal`] when `offset` falls outside the real-time range.
pub fn notify_waybar(offset: u8) -> Result<usize, SignalError> {
    notify_named("waybar", offset)
}

fn notify_named(name: &str, offset: u8) -> Result<usize, SignalError> {
    let signal = Signal::rt(i32::from(offset)).ok_or(SignalError::InvalidSignal(offset))?;

    let mut delivered = 0usize;
    let mut refused = None;
    for raw in pids_by_name(name)? {
        let Some(pid) = Pid::from_raw(raw) else {
            continue;
        };
        match process::kill_process(pid, signal) {
            Ok(()) => delivered = delivered.saturating_add(1),
            Err(Errno::SRCH) => {}
            Err(errno) => {
                refused = refused.or(Some(SignalError::Refused {
                    pid: raw,
                    source: io::Error::from(errno),
                }));
            }
        }
    }
    match (delivered, refused) {
        (0, Some(failure)) => Err(failure),
        (0, None) => Err(SignalError::NotRunning),
        (sent, _) => Ok(sent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process;

    #[test]
    fn a_process_that_is_not_running_is_reported_as_such() {
        assert!(matches!(
            notify_named("cryptobar-no-such-process", 1),
            Err(SignalError::NotRunning)
        ));
    }

    #[test]
    fn an_out_of_range_offset_is_rejected() {
        assert!(matches!(
            notify_waybar(u8::MAX),
            Err(SignalError::InvalidSignal(_))
        ));
    }

    #[test]
    fn valid_offsets_are_accepted_by_rustix() {
        for offset in 1_i32..=30 {
            assert!(Signal::rt(offset).is_some());
        }
    }

    #[test]
    fn the_current_process_is_visible_in_proc() {
        let comm = fs::read_to_string("/proc/self/comm").unwrap();
        let pids = pids_by_name(comm.trim()).unwrap();
        assert!(pids.contains(&process::id().cast_signed()));
    }
}
