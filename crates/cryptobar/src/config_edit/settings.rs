use anyhow::Result;
use toml_edit::{Item, Value};

use crate::config::Config;
use crate::model::Currency;

use super::editor::Editor;

impl Editor {
    /// # Errors
    /// See [`Self::apply`].
    pub fn set_currency(&self, currency: &Currency) -> Result<Config> {
        self.set_value("currency", currency.as_str().into())
    }

    /// Pins the decimals, `None` meaning "auto".
    ///
    /// # Errors
    /// See [`Self::apply`].
    pub fn set_decimals(&self, decimals: Option<u8>) -> Result<Config> {
        let value = match decimals {
            Some(n) => Value::from(i64::from(n)),
            None => Value::from("auto"),
        };
        self.set_value("decimals", value)
    }

    /// # Errors
    /// See [`Self::apply`].
    pub fn set_integer(&self, key: &str, value: i64) -> Result<Config> {
        self.set_value(key, value.into())
    }

    /// # Errors
    /// See [`Self::apply`].
    pub fn set_bool(&self, key: &str, value: bool) -> Result<Config> {
        self.set_value(key, value.into())
    }

    /// # Errors
    /// See [`Self::apply`].
    pub fn set_string(&self, key: &str, value: &str) -> Result<Config> {
        self.set_value(key, value.into())
    }

    fn set_value(&self, key: &str, value: Value) -> Result<Config> {
        let key = key.to_owned();
        self.apply(move |document| {
            // A key already present keeps its position and its comment: only
            // the value is replaced.
            let decor = document
                .get(&key)
                .and_then(Item::as_value)
                .map(|existing| existing.decor().clone());
            let mut value = value;
            if let Some(decor) = decor {
                *value.decor_mut() = decor;
            } else {
                value.decor_mut().set_prefix(" ");
            }
            document.insert(&key, Item::Value(value));
            Ok(())
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::config_edit::fixtures::{editor, seed};

    #[test]
    fn replacing_a_value_keeps_the_line_comment() {
        let (_guard, editor) = editor();
        seed(
            &editor,
            "coins = [\"bitcoin\"]\nrefresh_interval = 60 # every minute\n",
        );
        let config = editor.set_integer("refresh_interval", 120).unwrap();
        assert_eq!(config.refresh_interval, 120);
        let raw = std::fs::read_to_string(editor.path()).unwrap();
        assert!(raw.contains("# every minute"), "lost decor: {raw}");
    }

    #[test]
    fn an_invalid_value_leaves_the_file_untouched() {
        let (_guard, editor) = editor();
        let original = "coins = [\"bitcoin\"]\nrefresh_interval = 60\n";
        seed(&editor, original);
        assert!(editor.set_integer("refresh_interval", 1).is_err());
        assert_eq!(std::fs::read_to_string(editor.path()).unwrap(), original);
    }

    #[test]
    fn a_missing_key_is_added() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let config = editor.set_bool("sparkline", false).unwrap();
        assert!(!config.sparkline);
    }

    #[test]
    fn decimals_toggle_between_auto_and_a_fixed_value() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert_eq!(
            editor.set_decimals(Some(3)).unwrap().decimals(),
            crate::render::format::Decimals::Fixed(3)
        );
        assert_eq!(
            editor.set_decimals(None).unwrap().decimals(),
            crate::render::format::Decimals::Auto
        );
    }
}
