//! Everything that can end a run with a failure.

use cryptobar::app::AppError;
use cryptobar::commands::CommandError;

#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error(transparent)]
    Start(#[from] AppError),
    #[error(transparent)]
    Command(#[from] CommandError),
}
