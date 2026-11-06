//! Process exit codes, mapped exhaustively from every failure.

use std::process::ExitCode;

use cryptobar::app::AppError;
use cryptobar::commands::CommandError;

use crate::failure::Failure;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Success,
    DataError,
    Unavailable,
    Software,
    IoError,
    Config,
}

impl Exit {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::DataError => 65,
            Self::Unavailable => 69,
            Self::Software => 70,
            Self::IoError => 74,
            Self::Config => 78,
        }
    }
}

impl From<&Failure> for Exit {
    fn from(failure: &Failure) -> Self {
        match failure {
            Failure::Start(error) => Self::from(error),
            Failure::Command(error) => Self::from(error),
        }
    }
}

impl From<&AppError> for Exit {
    fn from(error: &AppError) -> Self {
        match error {
            AppError::Paths(_) | AppError::Config(_) => Self::Config,
            AppError::State(_) | AppError::Signal(_) | AppError::Alert(_) => Self::IoError,
            AppError::Icon(_) => Self::Unavailable,
        }
    }
}

impl From<&CommandError> for Exit {
    fn from(error: &CommandError) -> Self {
        match error {
            CommandError::App(inner) => Self::from(inner),
            CommandError::State(_)
            | CommandError::Write(_)
            | CommandError::Executable(_)
            | CommandError::Signal(_) => Self::IoError,
            CommandError::Paths(_) | CommandError::Config(_) | CommandError::Edit(_) => {
                Self::Config
            }
            CommandError::NoCoin | CommandError::NoTerminal => Self::Config,
            CommandError::Source(_)
            | CommandError::Search(_)
            | CommandError::Series { .. }
            | CommandError::Launch { .. } => Self::Unavailable,
            CommandError::Input(_) => Self::DataError,
            CommandError::Render(_) => Self::Software,
            #[cfg(feature = "popup")]
            CommandError::Popup(_) => Self::Software,
            #[cfg(not(feature = "popup"))]
            CommandError::PopupUnavailable => Self::Unavailable,
        }
    }
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        Self::from(exit.code())
    }
}
