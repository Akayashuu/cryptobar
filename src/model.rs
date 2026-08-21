//! Types du domaine. Aucune E/S, aucune dépendance vers le réseau ou l'affichage.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Devises supportées par CoinGecko que nous exposons, avec leur symbole d'affichage.
const CURRENCIES: &[(&str, &str)] = &[
    ("eur", "€"),
    ("usd", "$"),
    ("gbp", "£"),
    ("chf", "CHF"),
    ("cad", "CA$"),
    ("jpy", "¥"),
    ("btc", "₿"),
    ("eth", "Ξ"),
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("l'identifiant de la cryptomonnaie est vide")]
    EmptyCoinId,
    #[error(
        "identifiant de cryptomonnaie invalide : {0:?} (attendu : minuscules, chiffres et tirets, \
         comme « bitcoin » — cherchez avec `cryptobar coins --search`)"
    )]
    InvalidCoinId(String),
    #[error("devise non supportée : {0:?} (supportées : eur, usd, gbp, chf, cad, jpy, btc, eth)")]
    UnsupportedCurrency(String),
    #[error("plage inconnue : {0:?} (attendu : 1h, 24h, 7d, 30d, 1y)")]
    UnknownRange(String),
}

/// Identifiant CoinGecko validé. Le constructeur est le seul point d'entrée, ce qui
/// garantit qu'aucune chaîne non filtrée ne peut atteindre une URL ou un chemin de cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CoinId(String);

impl CoinId {
    /// # Errors
    /// Renvoie [`ModelError::EmptyCoinId`] si l'entrée est vide une fois nettoyée,
    /// [`ModelError::InvalidCoinId`] si elle contient autre chose que des minuscules,
    /// des chiffres et des tirets.
    pub fn parse(raw: &str) -> Result<Self, ModelError> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Err(ModelError::EmptyCoinId);
        }
        let valid = trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid || trimmed.starts_with('-') || trimmed.ends_with('-') || trimmed.len() > 64 {
            return Err(ModelError::InvalidCoinId(raw.to_owned()));
        }
        Ok(Self(trimmed))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Identifiant écrit en dur dans le code, donc déjà valide. Réservé aux
    /// constantes du programme : tout ce qui vient de l'extérieur passe par
    /// [`CoinId::parse`].
    #[must_use]
    pub(crate) fn trusted(raw: &'static str) -> Self {
        Self(raw.to_owned())
    }
}

impl TryFrom<String> for CoinId {
    type Error = ModelError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl fmt::Display for CoinId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Devise de cotation validée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Currency(String);

impl Currency {
    /// # Errors
    /// Renvoie [`ModelError::UnsupportedCurrency`] si la devise n'est pas dans la liste supportée.
    pub fn parse(raw: &str) -> Result<Self, ModelError> {
        let lowered = raw.trim().to_ascii_lowercase();
        if CURRENCIES.iter().any(|(code, _)| *code == lowered) {
            Ok(Self(lowered))
        } else {
            Err(ModelError::UnsupportedCurrency(raw.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Toutes les devises de cotation supportées, dans l'ordre d'affichage.
    #[must_use]
    pub fn all() -> Vec<Self> {
        CURRENCIES
            .iter()
            .map(|(code, _)| Self((*code).to_owned()))
            .collect()
    }

    #[must_use]
    pub fn symbol(&self) -> &'static str {
        CURRENCIES
            .iter()
            .find(|(code, _)| *code == self.0)
            .map_or("", |(_, symbol)| *symbol)
    }
}

impl TryFrom<String> for Currency {
    type Error = ModelError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl Default for Currency {
    fn default() -> Self {
        Self("eur".to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Up,
    Down,
    Flat,
}

impl Trend {
    #[must_use]
    pub fn class(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Flat => "flat",
        }
    }
}

/// Cotation d'une cryptomonnaie à un instant donné.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub id: CoinId,
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change_1h: Option<f64>,
    pub change_24h: Option<f64>,
    pub change_7d: Option<f64>,
    pub sparkline: Vec<f64>,
    /// URL du logo officiel, telle que fournie par la source. Conservée dans
    /// le cache pour pouvoir télécharger l'image plus tard, hors du chemin
    /// critique d'un tick de barre.
    #[serde(default)]
    pub image: Option<String>,
    /// Capitalisation boursière dans la devise de cotation.
    #[serde(default)]
    pub market_cap: Option<f64>,
    /// Volume échangé sur 24 h dans la devise de cotation.
    #[serde(default)]
    pub volume_24h: Option<f64>,
    /// Rang par capitalisation, 1 étant le plus élevé.
    #[serde(default)]
    pub rank: Option<u32>,
    /// Plus haut historique dans la devise de cotation.
    #[serde(default)]
    pub ath: Option<f64>,
    /// Date du plus haut historique, en ISO 8601 telle que fournie par la source.
    #[serde(default)]
    pub ath_date: Option<String>,
    /// Chemin local du logo une fois téléchargé. Rempli à l'exécution et
    /// jamais persisté : un chemin de cache n'a pas à survivre au cache.
    #[serde(skip)]
    pub icon: Option<String>,
}

impl Quote {
    /// Tendance affichée dans la barre, basée sur la variation 24 h.
    /// Une variation absente ou non finie est traitée comme plate.
    #[must_use]
    pub fn trend(&self) -> Trend {
        match self.change_24h {
            Some(change) if change.is_finite() && change > 0.0 => Trend::Up,
            Some(change) if change.is_finite() && change < 0.0 => Trend::Down,
            _ => Trend::Flat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Range {
    H1,
    H24,
    D7,
    D30,
    Y1,
}

impl Range {
    /// Valeur du paramètre `days` de l'API CoinGecko.
    #[must_use]
    pub fn days(self) -> &'static str {
        match self {
            Self::H1 => "0.0417",
            Self::H24 => "1",
            Self::D7 => "7",
            Self::D30 => "30",
            Self::Y1 => "365",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::H1 => "1 h",
            Self::H24 => "24 h",
            Self::D7 => "7 j",
            Self::D30 => "30 j",
            Self::Y1 => "1 an",
        }
    }

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::H1 => "1h",
            Self::H24 => "24h",
            Self::D7 => "7d",
            Self::D30 => "30d",
            Self::Y1 => "1y",
        }
    }

    /// Durée couverte par la plage, en minutes. Sert à dater un point de la
    /// courbe : les séries ne portent que des valeurs, pas d'horodatage.
    #[must_use]
    pub fn span_minutes(self) -> f64 {
        match self {
            Self::H1 => 60.0,
            Self::H24 => 1_440.0,
            Self::D7 => 10_080.0,
            Self::D30 => 43_200.0,
            Self::Y1 => 525_600.0,
        }
    }
}

impl FromStr for Range {
    type Err = ModelError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "1h" => Ok(Self::H1),
            "24h" | "1d" => Ok(Self::H24),
            "7d" | "1w" => Ok(Self::D7),
            "30d" | "1m" => Ok(Self::D30),
            "1y" | "365d" => Ok(Self::Y1),
            other => Err(ModelError::UnknownRange(other.to_owned())),
        }
    }
}

/// Série de prix pour une plage donnée, ordonnée du plus ancien au plus récent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub id: CoinId,
    pub range: Range,
    pub points: Vec<f64>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn coin_id_accepte_les_identifiants_coingecko() {
        assert_eq!(CoinId::parse("bitcoin").unwrap().as_str(), "bitcoin");
        assert_eq!(CoinId::parse("  Bitcoin ").unwrap().as_str(), "bitcoin");
        assert_eq!(
            CoinId::parse("matic-network").unwrap().as_str(),
            "matic-network"
        );
    }

    #[test]
    fn coin_id_refuse_ce_qui_nest_pas_un_identifiant() {
        assert!(matches!(CoinId::parse(""), Err(ModelError::EmptyCoinId)));
        assert!(matches!(CoinId::parse("   "), Err(ModelError::EmptyCoinId)));
        assert!(CoinId::parse("bit coin").is_err());
        assert!(CoinId::parse("bitcoin;rm -rf /").is_err());
        assert!(CoinId::parse("../etc/passwd").is_err());
    }

    #[test]
    fn currency_normalise_et_valide() {
        assert_eq!(Currency::parse("EUR").unwrap().as_str(), "eur");
        assert_eq!(Currency::parse("eur").unwrap().symbol(), "€");
        assert_eq!(Currency::parse("usd").unwrap().symbol(), "$");
        assert!(Currency::parse("xyz").is_err());
    }

    #[test]
    fn trend_depuis_la_variation_24h() {
        let q = |c: Option<f64>| Quote {
            id: CoinId::parse("bitcoin").unwrap(),
            symbol: "btc".into(),
            name: "Bitcoin".into(),
            price: 1.0,
            change_1h: None,
            change_24h: c,
            change_7d: None,
            sparkline: Vec::new(),
            image: None,
            market_cap: None,
            volume_24h: None,
            rank: None,
            ath: None,
            ath_date: None,
            icon: None,
        };
        assert_eq!(q(Some(2.0)).trend(), Trend::Up);
        assert_eq!(q(Some(-2.0)).trend(), Trend::Down);
        assert_eq!(q(Some(0.0)).trend(), Trend::Flat);
        assert_eq!(q(None).trend(), Trend::Flat);
        assert_eq!(q(Some(f64::NAN)).trend(), Trend::Flat);
    }

    #[test]
    fn range_parse_et_expose_ses_jours() {
        assert_eq!("7d".parse::<Range>().unwrap(), Range::D7);
        assert_eq!("24H".parse::<Range>().unwrap(), Range::H24);
        assert_eq!(Range::H1.days(), "0.0417");
        assert_eq!(Range::Y1.days(), "365");
        assert!("42d".parse::<Range>().is_err());
    }
}
