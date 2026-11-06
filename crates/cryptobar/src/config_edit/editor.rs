use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use crate::config::{Config, DEFAULT_CONFIG};
use crate::paths;

use super::error::EditError;

/// Editor bound to a configuration file.
#[derive(Debug, Clone)]
pub struct Editor {
    path: PathBuf,
}

impl Editor {
    /// Editor for the configuration file at `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Path of the configuration file being edited.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn document(&self) -> Result<DocumentMut, EditError> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(absent) if absent.kind() == io::ErrorKind::NotFound => DEFAULT_CONFIG.to_owned(),
            Err(source) => {
                return Err(EditError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        };
        raw.parse::<DocumentMut>()
            .map_err(|source| EditError::Malformed {
                path: self.path.clone(),
                source,
            })
    }

    /// Applies a transformation, validates it, then writes it atomically.
    ///
    /// # Errors
    /// Fails when the file is unreadable, when the transformation produces an
    /// invalid configuration, or when the write fails. In the first two cases
    /// the file is left untouched.
    pub fn apply<F>(&self, mutate: F) -> Result<Config, EditError>
    where
        F: FnOnce(&mut DocumentMut) -> Result<(), EditError>,
    {
        let mut document = self.document()?;
        mutate(&mut document)?;
        let rendered = document.to_string();
        let config = Config::parse(&rendered)?;
        self.write(&rendered)?;
        Ok(config)
    }

    fn write(&self, contents: &str) -> Result<(), EditError> {
        if let Some(parent) = self.path.parent() {
            paths::ensure_dir(parent).map_err(|source| EditError::Directory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let tmp = self.path.with_extension("toml.tmp");
        write_temporary(&tmp, contents)?;
        std::fs::rename(&tmp, &self.path).map_err(|source| EditError::Replace {
            path: self.path.clone(),
            source,
        })
    }
}

fn write_temporary(tmp: &Path, contents: &str) -> Result<(), EditError> {
    let to_error = |source: io::Error| EditError::Write {
        path: tmp.to_path_buf(),
        source,
    };
    let mut file = std::fs::File::create(tmp).map_err(to_error)?;
    file.write_all(contents.as_bytes()).map_err(to_error)?;
    file.sync_all().map_err(to_error)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::config_edit::fixtures::editor;
    use crate::model::Currency;

    #[test]
    fn a_missing_file_starts_from_the_default_template() {
        let (_guard, editor) = editor();
        let config = editor
            .set_currency(&Currency::parse("usd").unwrap())
            .unwrap();
        assert_eq!(config.currency.as_str(), "usd");
        assert_ne!(config.coins.len(), 0);
        assert!(editor.path().exists());
    }
}
