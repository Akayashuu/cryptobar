//! Surgical editing of the configuration file.
//!
//! The graphical window writes into the same `config.toml` the user edits by
//! hand. Rewriting the file from the serialised struct would wipe its comments
//! and its ordering; the TOML document is therefore edited in place with
//! `toml_edit`, which only touches the targeted value.
//!
//! Every change is validated by [`Config::parse`] **before** it reaches the
//! disk: a rejected setting leaves the file untouched.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use toml_edit::{Array, DocumentMut, Item, Value};

use crate::config::{Config, DEFAULT_CONFIG};
use crate::model::{CoinId, Currency};

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

    /// Adds an alert on a cryptocurrency.
    ///
    /// # Errors
    /// Fails when no condition is provided, or for the reasons of
    /// [`Self::apply`].
    pub fn add_alert(
        &self,
        id: &CoinId,
        above: Option<f64>,
        below: Option<f64>,
        change_pct: Option<f64>,
    ) -> Result<Config> {
        if above.is_none() && below.is_none() && change_pct.is_none() {
            bail!("an alert has to define at least one condition");
        }
        let key = id.as_str().to_owned();
        self.apply(move |document| {
            let array = alerts_array(document)?;
            let mut entry = toml_edit::Table::new();
            entry.insert("coin", Item::Value(Value::from(key)));
            for (field, value) in [
                ("above", above),
                ("below", below),
                ("change_pct", change_pct),
            ] {
                if let Some(value) = value {
                    entry.insert(field, Item::Value(Value::from(value)));
                }
            }
            array.push(entry);
            Ok(())
        })
    }

    /// Removes the alert at the given index.
    ///
    /// # Errors
    /// Fails when the index is out of the list, or for the reasons of
    /// [`Self::apply`].
    pub fn remove_alert(&self, index: usize) -> Result<Config> {
        self.apply(move |document| {
            let array = alerts_array(document)?;
            if index >= array.len() {
                bail!("no alert at index {index}");
            }
            array.remove(index);
            Ok(())
        })
    }

    /// Adds a wallet watched read-only.
    ///
    /// The address is validated before any write: a private key or a recovery
    /// phrase never reaches the disk.
    ///
    /// # Errors
    /// Fails when the address is not a public address of the given chain, or
    /// for the reasons of [`Self::apply`].
    pub fn add_wallet(
        &self,
        chain: crate::onchain::Chain,
        address: &str,
        label: Option<&str>,
    ) -> Result<Config> {
        let candidate = crate::onchain::Wallet {
            chain,
            address: address.trim().to_owned(),
            label: label.map(str::to_owned),
            enabled: true,
        };
        if let Err(reason) = candidate.validate() {
            bail!("{reason}");
        }
        let label = label
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_owned);
        self.apply(move |document| {
            let array = wallets_array(document)?;
            if array.iter().any(|entry| {
                entry.get("address").and_then(Item::as_str) == Some(candidate.address.as_str())
            }) {
                bail!("this address is already watched");
            }
            let mut entry = toml_edit::Table::new();
            entry.insert("chain", Item::Value(Value::from(chain.slug())));
            entry.insert("address", Item::Value(Value::from(candidate.address)));
            if let Some(label) = label {
                entry.insert("label", Item::Value(Value::from(label)));
            }
            array.push(entry);
            Ok(())
        })
    }

    /// Removes the watched wallet at the given index.
    ///
    /// # Errors
    /// Fails when the index is out of the list, or for the reasons of
    /// [`Self::apply`].
    pub fn remove_wallet(&self, index: usize) -> Result<Config> {
        self.apply(move |document| {
            let array = wallets_array(document)?;
            if index >= array.len() {
                bail!("no wallet at index {index}");
            }
            array.remove(index);
            Ok(())
        })
    }

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

/// The `[[wallets]]` array of tables, created when needed.
fn wallets_array(document: &mut DocumentMut) -> Result<&mut toml_edit::ArrayOfTables> {
    if !document
        .get("wallets")
        .is_some_and(Item::is_array_of_tables)
    {
        document.insert(
            "wallets",
            Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
        );
    }
    document
        .get_mut("wallets")
        .and_then(Item::as_array_of_tables_mut)
        .context("`wallets` is not an array of tables")
}

fn alerts_array(document: &mut DocumentMut) -> Result<&mut toml_edit::ArrayOfTables> {
    if !document.get("alerts").is_some_and(Item::is_array_of_tables) {
        document.insert(
            "alerts",
            Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
        );
    }
    document
        .get_mut("alerts")
        .and_then(Item::as_array_of_tables_mut)
        .context("`alerts` is not an array of tables")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn editor() -> (tempfile::TempDir, Editor) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        (dir, Editor::new(path))
    }

    fn seed(editor: &Editor, contents: &str) {
        std::fs::write(editor.path(), contents).unwrap();
    }

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

    #[test]
    fn an_alert_is_added_then_removed() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let id = CoinId::parse("bitcoin").unwrap();
        let config = editor.add_alert(&id, Some(100.0), None, None).unwrap();
        assert_eq!(config.alerts.len(), 1);
        assert_eq!(config.alerts.first().unwrap().above, Some(100.0));
        let config = editor.remove_alert(0).unwrap();
        assert_eq!(config.alerts.len(), 0);
    }

    #[test]
    fn a_public_address_is_added_then_removed() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let config = editor
            .add_wallet(
                crate::onchain::Chain::Bitcoin,
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                Some("Ledger"),
            )
            .unwrap();
        assert_eq!(config.wallets.len(), 1);
        assert_eq!(config.wallets.first().unwrap().title(), "Ledger");
        let config = editor.remove_wallet(0).unwrap();
        assert_eq!(config.wallets.len(), 0);
    }

    #[test]
    fn a_recovery_phrase_never_touches_the_disk() {
        let (_guard, editor) = editor();
        let original = "coins = [\"bitcoin\"]\n";
        seed(&editor, original);
        assert!(
            editor
                .add_wallet(
                    crate::onchain::Chain::Bitcoin,
                    "abandon abandon abandon ability",
                    None,
                )
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(editor.path()).unwrap(), original);
    }

    #[test]
    fn the_same_address_is_not_watched_twice() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"ethereum\"]\n");
        let address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        editor
            .add_wallet(crate::onchain::Chain::Ethereum, address, None)
            .unwrap();
        assert!(
            editor
                .add_wallet(crate::onchain::Chain::Ethereum, address, None)
                .is_err()
        );
    }

    #[test]
    fn removing_a_missing_wallet_is_an_error() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.remove_wallet(0).is_err());
    }

    #[test]
    fn an_alert_without_a_condition_is_rejected() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let id = CoinId::parse("bitcoin").unwrap();
        assert!(editor.add_alert(&id, None, None, None).is_err());
    }

    #[test]
    fn an_alert_on_an_unwatched_coin_leaves_the_file_untouched() {
        let (_guard, editor) = editor();
        let original = "coins = [\"bitcoin\"]\n";
        seed(&editor, original);
        let id = CoinId::parse("solana").unwrap();
        assert!(editor.add_alert(&id, Some(1.0), None, None).is_err());
        assert_eq!(std::fs::read_to_string(editor.path()).unwrap(), original);
    }

    #[test]
    fn removing_a_missing_alert_is_rejected() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.remove_alert(0).is_err());
    }

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
