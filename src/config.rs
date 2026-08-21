//! Chargement, validation et écriture du fichier de configuration.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::{CoinId, Currency};
use crate::paths;
use crate::render::format::Decimals;

/// Plancher de l'intervalle de rafraîchissement. En dessous, l'API publique
/// CoinGecko renvoie des 429 et finit par bannir temporairement l'adresse.
pub const MIN_REFRESH_INTERVAL: u64 = 15;

pub(crate) const DEFAULT_CONFIG: &str = r#"# Configuration de cryptobar — https://github.com/Akayashuu/cryptobar
# Identifiants CoinGecko ; cherchez-les avec `cryptobar coins <terme>`.
coins = ["bitcoin", "ethereum", "solana"]

# Devise de cotation : eur, usd, gbp, chf, cad, jpy, btc, eth
currency = "eur"

# Intervalle de rafraîchissement des prix, en secondes (minimum 15).
refresh_interval = 60

# Rotation automatique entre les coins, en secondes. 0 = rotation manuelle à la molette.
cycle_interval = 0

# Sparkline inline dans la barre.
sparkline = true
sparkline_width = 8

# Décimales : "auto" s'adapte à l'ordre de grandeur, ou un entier pour les figer.
decimals = "auto"

# Émulateur de terminal utilisé par le clic droit.
terminal = "kitty"

# Signal temps réel utilisé pour rafraîchir Waybar : SIGRTMIN+<n>, 1..=30.
waybar_signal = 8

# Portefeuilles suivis en lecture seule. On ne renseigne qu'une ADRESSE
# PUBLIQUE : cryptobar lit le solde sur la chaîne et l'ajoute au portefeuille.
# Aucune clé privée, aucune phrase de récupération — elles sont refusées.
# Chaînes acceptées : "bitcoin", "ethereum", "solana".
# [[wallets]]
# chain = "bitcoin"
# address = "bc1q…"
# label = "Ledger"

# Alertes de prix. Chaque bloc surveille une pièce ; « above » et « below »
# s'expriment dans la devise ci-dessus, « change_pct » en pourcentage absolu de
# variation sur 24 h. Une alerte se réarme dès que la condition retombe.
# [[alerts]]
# coin = "bitcoin"
# above = 100000
#
# [[alerts]]
# coin = "ethereum"
# change_pct = 5
"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DecimalsSetting {
    Fixed(u8),
    Auto(String),
}

impl Default for DecimalsSetting {
    fn default() -> Self {
        Self::Auto("auto".to_owned())
    }
}

/// Seuil de notification sur une cryptomonnaie suivie.
///
/// Les trois conditions sont indépendantes et cumulables : un même bloc peut
/// notifier au franchissement d'un plafond, d'un plancher, et d'une amplitude
/// de variation sur 24 h.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alert {
    pub coin: CoinId,
    #[serde(default)]
    pub above: Option<f64>,
    #[serde(default)]
    pub below: Option<f64>,
    #[serde(default)]
    pub change_pct: Option<f64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub coins: Vec<CoinId>,
    #[serde(default)]
    pub currency: Currency,
    #[serde(default = "default_refresh")]
    pub refresh_interval: u64,
    #[serde(default)]
    pub cycle_interval: u64,
    #[serde(default = "default_true")]
    pub sparkline: bool,
    #[serde(default = "default_sparkline_width")]
    pub sparkline_width: usize,
    #[serde(default)]
    pub decimals: DecimalsSetting,
    #[serde(default = "default_terminal")]
    pub terminal: String,
    #[serde(default = "default_signal")]
    pub waybar_signal: u8,
    /// Portefeuilles suivis en lecture seule, par adresse publique.
    #[serde(default)]
    pub wallets: Vec<crate::onchain::Wallet>,
    /// Seuils de notification. `[[alerts]]` étant un tableau de tables, il doit
    /// rester le dernier champ sérialisé ; il l'est déjà par construction.
    #[serde(default)]
    pub alerts: Vec<Alert>,
}

fn default_refresh() -> u64 {
    60
}
pub(crate) fn default_true() -> bool {
    true
}
fn default_sparkline_width() -> usize {
    8
}
fn default_terminal() -> String {
    "kitty".to_owned()
}
fn default_signal() -> u8 {
    8
}

impl Alert {
    fn validate(&self, coins: &[CoinId]) -> Result<()> {
        if !coins.contains(&self.coin) {
            bail!(
                "l'alerte porte sur `{}`, qui n'est pas dans `coins` : ajoutez-la d'abord",
                self.coin
            );
        }
        for (label, value) in [
            ("above", self.above),
            ("below", self.below),
            ("change_pct", self.change_pct),
        ] {
            if let Some(value) = value
                && (!value.is_finite() || value <= 0.0)
            {
                bail!("`alerts.{label}` = {value} doit être un nombre strictement positif");
            }
        }
        if self.above.is_none() && self.below.is_none() && self.change_pct.is_none() {
            bail!(
                "l'alerte sur `{}` ne définit aucune condition : renseignez `above`, `below` ou \
                 `change_pct`",
                self.coin
            );
        }
        if let (Some(above), Some(below)) = (self.above, self.below)
            && below >= above
        {
            bail!(
                "l'alerte sur `{}` a `below` = {below} au-dessus de `above` = {above} : elle se \
                 déclencherait en permanence",
                self.coin
            );
        }
        Ok(())
    }
}

impl Config {
    /// Charge la configuration depuis le chemin XDG, en la créant si elle est absente.
    ///
    /// # Errors
    /// Échoue si la configuration est illisible ou invalide.
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;
        if !path.exists() {
            if let Some(parent) = path.parent() {
                paths::ensure_dir(parent)?;
            }
            Self::write_default(&path, false)?;
        }
        Self::load_from(&path)
    }

    /// Charge et valide la configuration depuis un chemin explicite.
    ///
    /// # Errors
    /// Échoue si le fichier est illisible ou invalide.
    pub fn load_from(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("lecture de la configuration {}", path.display()))?;
        Self::parse(&raw).with_context(|| format!("configuration invalide : {}", path.display()))
    }

    /// Parse et valide un contenu TOML.
    ///
    /// # Errors
    /// Échoue si le TOML est mal formé ou si une valeur est hors contrat.
    pub fn parse(raw: &str) -> Result<Self> {
        let config: Self = toml::from_str(raw)?;
        config.validate()?;
        Ok(config)
    }

    /// Écrit la configuration par défaut, sans jamais écraser un fichier existant.
    ///
    /// # Errors
    /// Échoue si le fichier existe déjà ou s'il ne peut pas être écrit.
    pub fn write_default(path: &Path, force: bool) -> Result<()> {
        if let Some(parent) = path.parent() {
            paths::ensure_dir(parent)?;
        }
        if path.exists() && !force {
            bail!(
                "{} existe déjà, utilisez --force pour l'écraser",
                path.display()
            );
        }
        std::fs::write(path, DEFAULT_CONFIG)
            .with_context(|| format!("écriture de {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
        if self.coins.is_empty() {
            bail!("`coins` doit contenir au moins une cryptomonnaie");
        }
        let mut seen = std::collections::HashSet::new();
        for coin in &self.coins {
            if !seen.insert(coin.as_str()) {
                bail!("`coins` contient un doublon : {coin}");
            }
        }
        if self.refresh_interval < MIN_REFRESH_INTERVAL {
            bail!(
                "`refresh_interval` = {} est trop court ; minimum {MIN_REFRESH_INTERVAL} s pour \
                 rester dans le quota de l'API publique CoinGecko",
                self.refresh_interval
            );
        }
        if !(1..=30).contains(&self.waybar_signal) {
            bail!(
                "`waybar_signal` = {} est hors plage ; attendu 1..=30 (SIGRTMIN+n)",
                self.waybar_signal
            );
        }
        if !(1..=64).contains(&self.sparkline_width) {
            bail!(
                "`sparkline_width` = {} est hors plage ; attendu 1..=64",
                self.sparkline_width
            );
        }
        for wallet in &self.wallets {
            if let Err(reason) = wallet.validate() {
                bail!("`wallets.address` = {:?} : {reason}", wallet.address);
            }
        }
        for alert in &self.alerts {
            alert.validate(&self.coins)?;
        }
        if let DecimalsSetting::Auto(value) = &self.decimals
            && value != "auto"
        {
            bail!("`decimals` doit valoir \"auto\" ou un entier, pas {value:?}");
        }
        Ok(())
    }

    /// Traduit le réglage de décimales en valeur utilisable par le formateur.
    #[must_use]
    pub fn decimals(&self) -> Decimals {
        match &self.decimals {
            DecimalsSetting::Fixed(n) => Decimals::Fixed(*n),
            DecimalsSetting::Auto(_) => Decimals::Auto,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn config_minimale_utilise_les_valeurs_par_defaut() {
        let cfg = Config::parse(r#"coins = ["bitcoin"]"#).unwrap();
        assert_eq!(cfg.currency.as_str(), "eur");
        assert_eq!(cfg.refresh_interval, 60);
        assert_eq!(cfg.cycle_interval, 0);
        assert!(cfg.sparkline);
        assert_eq!(cfg.sparkline_width, 8);
        assert_eq!(cfg.waybar_signal, 8);
    }

    #[test]
    fn liste_de_coins_vide_refusee() {
        let err = Config::parse("coins = []").unwrap_err().to_string();
        assert!(err.contains("au moins une"), "message peu clair : {err}");
    }

    #[test]
    fn doublon_de_coin_refuse() {
        let err = Config::parse(r#"coins = ["bitcoin", "Bitcoin"]"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("bitcoin"), "message peu clair : {err}");
    }

    #[test]
    fn intervalle_trop_court_refuse_pour_proteger_le_quota() {
        let err = Config::parse("coins = [\"bitcoin\"]\nrefresh_interval = 5")
            .unwrap_err()
            .to_string();
        assert!(err.contains("15"), "message peu clair : {err}");
    }

    #[test]
    fn signal_hors_plage_refuse() {
        assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 0").is_err());
        assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 31").is_err());
        assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 30").is_ok());
    }

    #[test]
    fn devise_invalide_refusee_avec_le_champ_fautif() {
        let err = Config::parse("coins = [\"bitcoin\"]\ncurrency = \"xyz\"")
            .unwrap_err()
            .to_string();
        assert!(err.contains("currency"), "message peu clair : {err}");
    }

    #[test]
    fn decimales_auto_ou_fixes() {
        assert_eq!(
            Config::parse("coins = [\"bitcoin\"]").unwrap().decimals(),
            Decimals::Auto
        );
        assert_eq!(
            Config::parse("coins = [\"bitcoin\"]\ndecimals = 3")
                .unwrap()
                .decimals(),
            Decimals::Fixed(3)
        );
    }

    #[test]
    fn champ_inconnu_refuse_pour_attraper_les_fautes_de_frappe() {
        let err = Config::parse("coins = [\"bitcoin\"]\nrefresh_intervall = 60")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("refresh_intervall"),
            "message peu clair : {err}"
        );
    }

    #[test]
    fn config_par_defaut_ecrite_puis_relue_est_valide() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::write_default(&path, false).unwrap();
        let cfg = Config::load_from(&path).unwrap();
        assert!(!cfg.coins.is_empty());
    }

    #[test]
    fn write_default_refuse_decraser() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::write_default(&path, false).unwrap();
        assert!(Config::write_default(&path, false).is_err());
    }
}
