use anyhow::{Context as _, Result, bail};
use toml_edit::{Array, DocumentMut, Item, Value};

use crate::config::Config;
use crate::model::CoinId;

use super::editor::Editor;

impl Editor {
    /// Appends a cryptocurrency to the watched list.
    ///
    /// # Errors
    /// Fails when it is already there, or for the reasons of [`Self::apply`].
    pub fn add_coin(&self, id: &CoinId) -> Result<Config> {
        self.apply(|document| {
            let array = coins_array(document)?;
            if array
                .iter()
                .filter_map(Value::as_str)
                .any(|existing| existing == id.as_str())
            {
                bail!("{id} is already watched");
            }
            array.push(id.as_str());
            // `push` glues the value to the closing bracket; a space separates
            // it from the rest, as in the hand-written file.
            if let Some(last) = array.iter_mut().last() {
                last.decor_mut().set_prefix(" ");
            }
            Ok(())
        })
    }

    /// Removes a cryptocurrency from the watched list.
    ///
    /// # Errors
    /// Fails when it is missing, when it is the last one, or for the reasons of
    /// [`Self::apply`].
    pub fn remove_coin(&self, id: &str) -> Result<Config> {
        self.apply(|document| {
            let array = coins_array(document)?;
            let position = array
                .iter()
                .position(|value| value.as_str() == Some(id))
                .with_context(|| format!("{id} is not watched"))?;
            if array.len() <= 1 {
                bail!("at least one cryptocurrency has to stay watched");
            }
            array.remove(position);
            Ok(())
        })
    }
}

/// The `coins` array, created when missing.
fn coins_array(document: &mut DocumentMut) -> Result<&mut Array> {
    if !document.get("coins").is_some_and(Item::is_array) {
        document.insert("coins", Item::Value(Value::Array(Array::new())));
    }
    document
        .get_mut("coins")
        .and_then(Item::as_array_mut)
        .context("`coins` is not an array")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config_edit::fixtures::{editor, seed};

    #[test]
    fn adding_a_coin_preserves_the_comments() {
        let (_guard, editor) = editor();
        seed(&editor, "# my comment\ncoins = [\"bitcoin\"]\n");
        let config = editor.add_coin(&CoinId::parse("solana").unwrap()).unwrap();
        assert_eq!(config.coins.len(), 2);
        let raw = std::fs::read_to_string(editor.path()).unwrap();
        assert!(raw.contains("# my comment"), "lost comment: {raw}");
        assert!(raw.contains("\"solana\""));
    }

    #[test]
    fn adding_a_duplicate_is_rejected() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.add_coin(&CoinId::parse("bitcoin").unwrap()).is_err());
    }

    #[test]
    fn removing_the_last_coin_is_rejected_and_leaves_the_file_alone() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.remove_coin("bitcoin").is_err());
        assert_eq!(
            std::fs::read_to_string(editor.path()).unwrap(),
            "coins = [\"bitcoin\"]\n"
        );
    }

    #[test]
    fn removing_a_missing_coin_is_rejected() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\", \"solana\"]\n");
        assert!(editor.remove_coin("ethereum").is_err());
    }

    #[test]
    fn removing_a_coin_deletes_it() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\", \"solana\"]\n");
        let config = editor.remove_coin("bitcoin").unwrap();
        assert_eq!(config.coins.len(), 1);
        assert_eq!(config.coins.first().map(CoinId::as_str), Some("solana"));
    }
}
