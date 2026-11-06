use toml_edit::{DocumentMut, Item, Value};

use crate::config::Config;
use crate::model::CoinId;

use super::editor::Editor;
use super::error::EditError;

impl Editor {
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
    ) -> Result<Config, EditError> {
        if above.is_none() && below.is_none() && change_pct.is_none() {
            return Err(EditError::AlertWithoutCondition);
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
    pub fn remove_alert(&self, index: usize) -> Result<Config, EditError> {
        self.apply(move |document| {
            let array = alerts_array(document)?;
            if index >= array.len() {
                return Err(EditError::NoAlert(index));
            }
            array.remove(index);
            Ok(())
        })
    }
}

fn alerts_array(document: &mut DocumentMut) -> Result<&mut toml_edit::ArrayOfTables, EditError> {
    if !document.get("alerts").is_some_and(Item::is_array_of_tables) {
        document.insert(
            "alerts",
            Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
        );
    }
    document
        .get_mut("alerts")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or(EditError::AlertsNotArray)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_edit::fixtures::{editor, seed};
    use std::fs;

    #[test]
    fn an_alert_is_added_then_removed() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let id = CoinId::parse("bitcoin").unwrap();
        let config = editor.add_alert(&id, Some(100.0), None, None).unwrap();
        assert_eq!(config.alerts.len(), 1);
        assert_eq!(config.alerts.first().unwrap().above, Some(100.0));
        let after_removal = editor.remove_alert(0).unwrap();
        assert_eq!(after_removal.alerts.len(), 0);
    }

    #[test]
    fn an_alert_without_a_condition_is_rejected() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let id = CoinId::parse("bitcoin").unwrap();
        editor.add_alert(&id, None, None, None).unwrap_err();
    }

    #[test]
    fn an_alert_on_an_unwatched_coin_leaves_the_file_untouched() {
        let (_guard, editor) = editor();
        let original = "coins = [\"bitcoin\"]\n";
        seed(&editor, original);
        let id = CoinId::parse("solana").unwrap();
        editor.add_alert(&id, Some(1.0), None, None).unwrap_err();
        assert_eq!(fs::read_to_string(editor.path()).unwrap(), original);
    }

    #[test]
    fn removing_a_missing_alert_is_rejected() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        editor.remove_alert(0).unwrap_err();
    }
}
