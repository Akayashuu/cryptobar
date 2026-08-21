//! Chargement, validation et écriture du fichier de configuration.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::{CoinId, Currency};
use crate::paths;
use crate::render::format::Decimals;

/// Plancher de l'intervalle de rafraîchissement. En dessous, l'API publique
/// CoinGecko renvoie des 429 et finit par bannir temporairement l'adresse.
const MIN_REFRESH_INTERVAL: u64 = 15;

const DEFAULT_CONFIG: &str = r#"# Configuration de cryptobar — https://github.com/Akayashuu/cryptobar
# Identifiants CoinGecko ; cherchez-les avec `cryptobar coins --search <terme>`.
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
waybar_signal = 20
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
}

fn default_refresh() -> u64 {
    60
}
fn default_true() -> bool {
    true
}
fn default_sparkline_width() -> usize {
    8
}
fn default_terminal() -> String {
    "kitty".to_owned()
}
fn default_signal() -> u8 {
    20
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
        assert_eq!(cfg.waybar_signal, 20);
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
