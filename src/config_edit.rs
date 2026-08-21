//! Édition chirurgicale du fichier de configuration.
//!
//! La fenêtre graphique écrit dans le même `config.toml` que l'utilisateur
//! édite à la main. Réécrire le fichier depuis la structure sérialisée
//! effacerait ses commentaires et son ordre ; on modifie donc le document
//! TOML en place avec `toml_edit`, ce qui ne touche que la valeur visée.
//!
//! Toute modification est validée par [`Config::parse`] **avant** d'atteindre
//! le disque : un réglage refusé laisse le fichier intact.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use toml_edit::{Array, DocumentMut, Item, Value};

use crate::config::{Config, DEFAULT_CONFIG};
use crate::model::{CoinId, Currency};

/// Éditeur attaché à un fichier de configuration.
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

    /// Document courant, ou le modèle par défaut si le fichier est absent.
    fn document(&self) -> Result<DocumentMut> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DEFAULT_CONFIG.to_owned(),
            Err(e) => {
                return Err(e).with_context(|| format!("lecture de {}", self.path.display()));
            }
        };
        raw.parse::<DocumentMut>()
            .with_context(|| format!("TOML mal formé : {}", self.path.display()))
    }

    /// Applique une transformation, la valide, puis l'écrit atomiquement.
    ///
    /// # Errors
    /// Échoue si le fichier est illisible, si la transformation produit une
    /// configuration invalide, ou si l'écriture échoue. Dans les deux premiers
    /// cas le fichier n'est pas touché.
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

    /// Écriture atomique : le fichier n'est jamais observé à moitié écrit,
    /// même si la machine s'arrête au mauvais moment.
    fn write(&self, contents: &str) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            crate::paths::ensure_dir(parent)?;
        }
        let tmp = self.path.with_extension("toml.tmp");
        {
            let mut file = std::fs::File::create(&tmp)
                .with_context(|| format!("écriture de {}", tmp.display()))?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("remplacement de {}", self.path.display()))?;
        Ok(())
    }

    /// Ajoute une cryptomonnaie à la fin de la liste suivie.
    ///
    /// # Errors
    /// Échoue si elle y figure déjà, ou pour les raisons de [`Self::apply`].
    pub fn add_coin(&self, id: &CoinId) -> Result<Config> {
        self.apply(|document| {
            let array = coins_array(document)?;
            if array
                .iter()
                .filter_map(Value::as_str)
                .any(|existing| existing == id.as_str())
            {
                bail!("{id} est déjà suivie");
            }
            array.push(id.as_str());
            // `push` colle la valeur au crochet fermant ; un espace la sépare
            // du reste comme dans le fichier écrit à la main.
            if let Some(last) = array.iter_mut().last() {
                last.decor_mut().set_prefix(" ");
            }
            Ok(())
        })
    }

    /// Retire une cryptomonnaie de la liste suivie.
    ///
    /// # Errors
    /// Échoue si elle est absente, si elle est la dernière, ou pour les
    /// raisons de [`Self::apply`].
    pub fn remove_coin(&self, id: &str) -> Result<Config> {
        self.apply(|document| {
            let array = coins_array(document)?;
            let position = array
                .iter()
                .position(|value| value.as_str() == Some(id))
                .with_context(|| format!("{id} n'est pas suivie"))?;
            if array.len() <= 1 {
                bail!("au moins une cryptomonnaie doit rester suivie");
            }
            array.remove(position);
            Ok(())
        })
    }

    /// Ajoute une alerte sur une cryptomonnaie.
    ///
    /// # Errors
    /// Échoue si aucune condition n'est fournie, ou pour les raisons de
    /// [`Self::apply`].
    pub fn add_alert(
        &self,
        id: &CoinId,
        above: Option<f64>,
        below: Option<f64>,
        change_pct: Option<f64>,
    ) -> Result<Config> {
        if above.is_none() && below.is_none() && change_pct.is_none() {
            bail!("une alerte doit définir au moins une condition");
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

    /// Retire l'alerte d'indice donné.
    ///
    /// # Errors
    /// Échoue si l'indice est hors de la liste, ou pour les raisons de
    /// [`Self::apply`].
    pub fn remove_alert(&self, index: usize) -> Result<Config> {
        self.apply(move |document| {
            let array = alerts_array(document)?;
            if index >= array.len() {
                bail!("aucune alerte à l'indice {index}");
            }
            array.remove(index);
            Ok(())
        })
    }

    /// Ajoute un portefeuille suivi en lecture seule.
    ///
    /// L'adresse est validée avant toute écriture : une clé privée ou une
    /// phrase de récupération n'atteint jamais le disque.
    ///
    /// # Errors
    /// Échoue si l'adresse n'est pas une adresse publique de la chaîne
    /// indiquée, ou pour les raisons de [`Self::apply`].
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
                bail!("cette adresse est déjà suivie");
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

    /// Retire le portefeuille suivi d'indice donné.
    ///
    /// # Errors
    /// Échoue si l'indice est hors de la liste, ou pour les raisons de
    /// [`Self::apply`].
    pub fn remove_wallet(&self, index: usize) -> Result<Config> {
        self.apply(move |document| {
            let array = wallets_array(document)?;
            if index >= array.len() {
                bail!("aucun portefeuille à l'indice {index}");
            }
            array.remove(index);
            Ok(())
        })
    }

    /// # Errors
    /// Voir [`Self::apply`].
    pub fn set_currency(&self, currency: &Currency) -> Result<Config> {
        self.set_value("currency", currency.as_str().into())
    }

    /// Fixe les décimales, `None` valant « auto ».
    ///
    /// # Errors
    /// Voir [`Self::apply`].
    pub fn set_decimals(&self, decimals: Option<u8>) -> Result<Config> {
        let value = match decimals {
            Some(n) => Value::from(i64::from(n)),
            None => Value::from("auto"),
        };
        self.set_value("decimals", value)
    }

    /// # Errors
    /// Voir [`Self::apply`].
    pub fn set_integer(&self, key: &str, value: i64) -> Result<Config> {
        self.set_value(key, value.into())
    }

    /// # Errors
    /// Voir [`Self::apply`].
    pub fn set_bool(&self, key: &str, value: bool) -> Result<Config> {
        self.set_value(key, value.into())
    }

    /// # Errors
    /// Voir [`Self::apply`].
    pub fn set_string(&self, key: &str, value: &str) -> Result<Config> {
        self.set_value(key, value.into())
    }

    fn set_value(&self, key: &str, value: Value) -> Result<Config> {
        let key = key.to_owned();
        self.apply(move |document| {
            // Une clé déjà présente garde sa position et son commentaire :
            // seule la valeur est remplacée.
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

/// Tableau `coins`, créé s'il manque.
fn coins_array(document: &mut DocumentMut) -> Result<&mut Array> {
    if !document.get("coins").is_some_and(Item::is_array) {
        document.insert("coins", Item::Value(Value::Array(Array::new())));
    }
    document
        .get_mut("coins")
        .and_then(Item::as_array_mut)
        .context("`coins` n'est pas un tableau")
}

/// Tableau de tables `alerts`, créé s'il manque.
/// Tableau de tables `[[wallets]]`, créé au besoin.
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
        .context("`wallets` n'est pas un tableau de tables")
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
        .context("`alerts` n'est pas un tableau de tables")
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
    fn ajouter_une_piece_preserve_les_commentaires() {
        let (_guard, editor) = editor();
        seed(&editor, "# mon commentaire\ncoins = [\"bitcoin\"]\n");
        let config = editor.add_coin(&CoinId::parse("solana").unwrap()).unwrap();
        assert_eq!(config.coins.len(), 2);
        let raw = std::fs::read_to_string(editor.path()).unwrap();
        assert!(
            raw.contains("# mon commentaire"),
            "commentaire perdu : {raw}"
        );
        assert!(raw.contains("\"solana\""));
    }

    #[test]
    fn ajouter_un_doublon_est_refuse() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.add_coin(&CoinId::parse("bitcoin").unwrap()).is_err());
    }

    #[test]
    fn retirer_la_derniere_piece_est_refuse_et_ne_touche_pas_le_fichier() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.remove_coin("bitcoin").is_err());
        assert_eq!(
            std::fs::read_to_string(editor.path()).unwrap(),
            "coins = [\"bitcoin\"]\n"
        );
    }

    #[test]
    fn retirer_une_piece_absente_est_refuse() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\", \"solana\"]\n");
        assert!(editor.remove_coin("ethereum").is_err());
    }

    #[test]
    fn retirer_une_piece_la_supprime() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\", \"solana\"]\n");
        let config = editor.remove_coin("bitcoin").unwrap();
        assert_eq!(config.coins.len(), 1);
        assert_eq!(config.coins.first().map(CoinId::as_str), Some("solana"));
    }

    #[test]
    fn remplacer_une_valeur_garde_le_commentaire_de_la_ligne() {
        let (_guard, editor) = editor();
        seed(
            &editor,
            "coins = [\"bitcoin\"]\nrefresh_interval = 60 # toutes les minutes\n",
        );
        let config = editor.set_integer("refresh_interval", 120).unwrap();
        assert_eq!(config.refresh_interval, 120);
        let raw = std::fs::read_to_string(editor.path()).unwrap();
        assert!(raw.contains("# toutes les minutes"), "décor perdu : {raw}");
    }

    #[test]
    fn une_valeur_invalide_laisse_le_fichier_intact() {
        let (_guard, editor) = editor();
        let original = "coins = [\"bitcoin\"]\nrefresh_interval = 60\n";
        seed(&editor, original);
        assert!(editor.set_integer("refresh_interval", 1).is_err());
        assert_eq!(std::fs::read_to_string(editor.path()).unwrap(), original);
    }

    #[test]
    fn une_cle_absente_est_ajoutee() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let config = editor.set_bool("sparkline", false).unwrap();
        assert!(!config.sparkline);
    }

    #[test]
    fn les_decimales_basculent_entre_auto_et_valeur_fixe() {
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
    fn une_alerte_est_ajoutee_puis_retiree() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let id = CoinId::parse("bitcoin").unwrap();
        let config = editor.add_alert(&id, Some(100.0), None, None).unwrap();
        assert_eq!(config.alerts.len(), 1);
        assert_eq!(config.alerts.first().unwrap().above, Some(100.0));
        let config = editor.remove_alert(0).unwrap();
        assert!(config.alerts.is_empty());
    }

    #[test]
    fn une_adresse_publique_sajoute_puis_se_retire() {
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
        assert!(config.wallets.is_empty());
    }

    #[test]
    fn une_phrase_de_recuperation_ne_touche_jamais_le_disque() {
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
    fn la_meme_adresse_ne_se_suit_pas_deux_fois() {
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
    fn retirer_un_portefeuille_inexistant_est_une_erreur() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.remove_wallet(0).is_err());
    }

    #[test]
    fn une_alerte_sans_condition_est_refusee() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        let id = CoinId::parse("bitcoin").unwrap();
        assert!(editor.add_alert(&id, None, None, None).is_err());
    }

    #[test]
    fn une_alerte_sur_une_piece_non_suivie_laisse_le_fichier_intact() {
        let (_guard, editor) = editor();
        let original = "coins = [\"bitcoin\"]\n";
        seed(&editor, original);
        let id = CoinId::parse("solana").unwrap();
        assert!(editor.add_alert(&id, Some(1.0), None, None).is_err());
        assert_eq!(std::fs::read_to_string(editor.path()).unwrap(), original);
    }

    #[test]
    fn retirer_une_alerte_inexistante_est_refuse() {
        let (_guard, editor) = editor();
        seed(&editor, "coins = [\"bitcoin\"]\n");
        assert!(editor.remove_alert(0).is_err());
    }

    #[test]
    fn un_fichier_absent_part_du_modele_par_defaut() {
        let (_guard, editor) = editor();
        let config = editor
            .set_currency(&Currency::parse("usd").unwrap())
            .unwrap();
        assert_eq!(config.currency.as_str(), "usd");
        assert!(!config.coins.is_empty());
        assert!(editor.path().exists());
    }
}
