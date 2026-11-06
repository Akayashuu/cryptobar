use toml_edit::{DocumentMut, Item, Value};

use crate::config::Config;

use super::editor::Editor;
use super::error::EditError;

impl Editor {
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
    ) -> Result<Config, EditError> {
        let candidate = crate::onchain::Wallet {
            chain,
            address: address.trim().to_owned(),
            label: label.map(str::to_owned),
            enabled: true,
        };
        candidate.validate()?;
        let label = label
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_owned);
        self.apply(move |document| {
            let array = wallets_array(document)?;
            if array.iter().any(|entry| {
                entry.get("address").and_then(Item::as_str) == Some(candidate.address.as_str())
            }) {
                return Err(EditError::WalletAlreadyWatched);
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
    pub fn remove_wallet(&self, index: usize) -> Result<Config, EditError> {
        self.apply(move |document| {
            let array = wallets_array(document)?;
            if index >= array.len() {
                return Err(EditError::NoWallet(index));
            }
            array.remove(index);
            Ok(())
        })
    }
}

fn wallets_array(document: &mut DocumentMut) -> Result<&mut toml_edit::ArrayOfTables, EditError> {
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
        .ok_or(EditError::WalletsNotArray)
}

#[cfg(test)]
mod tests {
    use crate::config_edit::fixtures::{editor, seed};

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
        editor
            .add_wallet(
                crate::onchain::Chain::Bitcoin,
                "abandon abandon abandon ability",
                None,
            )
            .unwrap_err();
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
        editor
            .add_wallet(crate::onchain::Chain::Ethereum, address, None)
            .unwrap_err();
    }

    #[test]
    fn removing_a_missing_wallet_is_an_error() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        editor.remove_wallet(0).unwrap_err();
    }
}
