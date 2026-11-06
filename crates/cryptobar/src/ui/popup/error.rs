use std::path::PathBuf;

use crate::paths::PathsError;

#[derive(Debug, thiserror::Error)]
pub enum PopupError {
    #[error("the window exited with an error")]
    Exited,
    #[error("could not draw the chart")]
    Draw(#[from] cairo::Error),
    #[error("could not resolve the export directory")]
    ExportDir(#[from] PathsError),
    #[error("could not write the export {}", path.display())]
    Export {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not encode the PNG {}", path.display())]
    Png {
        path: PathBuf,
        #[source]
        source: cairo::IoError,
    },
}
