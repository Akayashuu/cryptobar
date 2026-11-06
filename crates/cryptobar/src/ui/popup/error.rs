use std::io;
use std::path::PathBuf;

use crate::paths::PathsError;

/// Failures of the graphical window.
#[derive(Debug, thiserror::Error)]
pub enum PopupError {
    /// The window closed with an error.
    #[error("the window exited with an error")]
    Exited,
    /// A chart could not be drawn.
    #[error("could not draw the chart")]
    Draw(#[from] cairo::Error),
    /// The export directory could not be resolved.
    #[error("could not resolve the export directory")]
    ExportDir(#[from] PathsError),
    /// The export could not be written.
    #[error("could not write the export {}", path.display())]
    Export {
        /// File that could not be written.
        path: PathBuf,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// The chart could not be encoded as PNG.
    #[error("could not encode the PNG {}", path.display())]
    Png {
        /// File that could not be written.
        path: PathBuf,
        /// The Cairo encoding failure.
        #[source]
        source: cairo::IoError,
    },
}
