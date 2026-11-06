use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use toml_edit::DocumentMut;

use crate::config::{Config, DEFAULT_CONFIG};

/// Editor bound to a configuration file.
#[derive(Debug, Clone)]
pub struct Editor {
    path: PathBuf,
}

impl Editor {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current document, or the default template when the file is missing.
    fn document(&self) -> Result<DocumentMut> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DEFAULT_CONFIG.to_owned(),
            Err(e) => {
                return Err(e).with_context(|| format!("reading {}", self.path.display()));
            }
        };
        raw.parse::<DocumentMut>()
            .with_context(|| format!("malformed TOML: {}", self.path.display()))
    }

    /// Applies a transformation, validates it, then writes it atomically.
    ///
    /// # Errors
    /// Fails when the file is unreadable, when the transformation produces an
    /// invalid configuration, or when the write fails. In the first two cases
    /// the file is left untouched.
    pub fn apply<F>(&self, mutate: F) -> Result<Config>
    where
        F: FnOnce(&mut DocumentMut) -> Result<()>,
    {
        let mut document = self.document()?;
        mutate(&mut document)?;
        let rendered = document.to_string();
        let config = Config::parse(&rendered)?;
        self.write(&rendered)?;
        Ok(config)
    }

    /// Atomic write: the file is never observed half-written, even if the
    /// machine stops at the wrong moment.
    fn write(&self, contents: &str) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            crate::paths::ensure_dir(parent)?;
        }
        let tmp = self.path.with_extension("toml.tmp");
        {
            let mut file = std::fs::File::create(&tmp)
                .with_context(|| format!("writing {}", tmp.display()))?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("replacing {}", self.path.display()))?;
        Ok(())
    }
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
