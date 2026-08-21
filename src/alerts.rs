//! Évaluation des seuils de prix.
//!
//! Le module est volontairement pur : il transforme une configuration et un
//! lot de cotations en déclenchements. La notification, elle, est un effet de
//! bord isolé dans [`notify`], et la mémoire de ce qui a déjà sonné vit dans
//! l'état d'exécution. Cette séparation rend la règle anti-répétition
//! testable sans réseau ni serveur de notifications.
//!
//! Règle de réarmement : une alerte reste « tirée » tant que sa condition est
//! vraie, et se réarme dès qu'elle redevient fausse. Un prix qui oscille
//! autour du seuil ne notifie donc qu'aux franchissements, et un prix qui
//! reste au-dessus ne notifie qu'une fois.

use std::collections::BTreeSet;

use crate::config::{Alert, Config};
use crate::model::{Currency, Quote};
use crate::render::format::{self, Decimals};

/// Condition franchie, sous une forme comparable d'un tick à l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Above,
    Below,
    Change,
}

impl Kind {
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Above => "above",
            Self::Below => "below",
            Self::Change => "change",
        }
    }
}

/// Une condition vraie à cet instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Identité stable de la condition, indépendante du prix courant : c'est
    /// elle qui est mémorisée pour ne pas notifier deux fois la même chose.
    pub key: String,
    pub symbol: String,
    pub kind: Kind,
    /// Titre de la notification, court et lisible dans un coin d'écran.
    pub title: String,
    pub body: String,
}

/// Clé d'une condition. Le seuil est intégré : modifier le seuil dans la
/// configuration réarme naturellement l'alerte.
fn key(coin: &str, kind: Kind, threshold: f64) -> String {
    format!("{coin}:{}:{threshold}", kind.slug())
}

/// Toutes les conditions vraies pour ce lot de cotations.
///
/// Les alertes désactivées et celles dont la pièce n'a pas de cotation sont
/// ignorées : une panne réseau ne doit ni notifier ni réarmer quoi que ce soit.
#[must_use]
pub fn evaluate(config: &Config, quotes: &[Quote]) -> Vec<Trigger> {
    let decimals = config.decimals();
    config
        .alerts
        .iter()
        .filter(|alert| alert.enabled)
        .filter_map(|alert| {
            let quote = quotes.iter().find(|quote| quote.id == alert.coin)?;
            Some(triggers_for(alert, quote, &config.currency, decimals))
        })
        .flatten()
        .collect()
}

fn triggers_for(
    alert: &Alert,
    quote: &Quote,
    currency: &Currency,
    decimals: Decimals,
) -> Vec<Trigger> {
    let coin = alert.coin.as_str();
    let price = format::price(quote.price, currency, decimals);
    let mut triggers = Vec::new();

    if let Some(threshold) = alert.above.filter(|threshold| quote.price >= *threshold) {
        triggers.push(Trigger {
            key: key(coin, Kind::Above, threshold),
            symbol: quote.symbol.clone(),
            kind: Kind::Above,
            title: format!(
                "{} au-dessus de {}",
                quote.symbol,
                seuil(threshold, currency, decimals)
            ),
            body: format!("{} vaut {price}.", quote.name),
        });
    }
    if let Some(threshold) = alert.below.filter(|threshold| quote.price <= *threshold) {
        triggers.push(Trigger {
            key: key(coin, Kind::Below, threshold),
            symbol: quote.symbol.clone(),
            kind: Kind::Below,
            title: format!(
                "{} sous {}",
                quote.symbol,
                seuil(threshold, currency, decimals)
            ),
            body: format!("{} vaut {price}.", quote.name),
        });
    }
    if let Some(threshold) = alert.change_pct {
        // L'amplitude est absolue : une chute de 6 % compte autant qu'une
        // hausse de 6 %, ce qui est ce qu'on veut d'une alerte de volatilité.
        let amplitude = quote
            .change_24h
            .filter(|change| change.is_finite())
            .map(f64::abs);
        if amplitude.is_some_and(|amplitude| amplitude >= threshold) {
            triggers.push(Trigger {
                key: key(coin, Kind::Change, threshold),
                symbol: quote.symbol.clone(),
                kind: Kind::Change,
                title: format!(
                    "{} bouge de {}",
                    quote.symbol,
                    format::percent(quote.change_24h)
                ),
                body: format!("{} vaut {price} après 24 h.", quote.name),
            });
        }
    }
    triggers
}

fn seuil(value: f64, currency: &Currency, decimals: Decimals) -> String {
    format::price(value, currency, decimals)
}

/// Sépare les conditions vraies en « nouvelles » et « déjà connues », et rend
/// l'ensemble à mémoriser pour le prochain tick.
#[must_use]
pub fn diff(triggers: &[Trigger], fired: &BTreeSet<String>) -> (Vec<Trigger>, BTreeSet<String>) {
    let current: BTreeSet<String> = triggers.iter().map(|t| t.key.clone()).collect();
    let fresh = triggers
        .iter()
        .filter(|trigger| !fired.contains(&trigger.key))
        .cloned()
        .collect();
    (fresh, current)
}

/// Envoie une notification de bureau. L'absence de `notify-send` ou de démon
/// de notification n'est pas une erreur : la barre continue d'afficher
/// l'alerte, seule la bulle manque.
pub fn notify(trigger: &Trigger) {
    let urgency = if trigger.kind == Kind::Change {
        "normal"
    } else {
        "critical"
    };
    let _ = std::process::Command::new("notify-send")
        .arg("--app-name=cryptobar")
        .arg("--icon=network-transmit-receive")
        .arg(format!("--urgency={urgency}"))
        // Une alerte par pièce et par condition : les bulles se remplacent au
        // lieu de s'empiler si le prix oscille autour du seuil.
        .arg(format!(
            "--hint=string:x-canonical-private-synchronous:cryptobar-{}",
            trigger.key
        ))
        .arg(&trigger.title)
        .arg(&trigger.body)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|mut child| child.wait());
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::CoinId;

    fn quote(id: &str, symbol: &str, price: f64, change: Option<f64>) -> Quote {
        Quote {
            id: CoinId::parse(id).unwrap(),
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            price,
            change_1h: None,
            change_24h: change,
            change_7d: None,
            sparkline: Vec::new(),
            image: None,
            market_cap: None,
            volume_24h: None,
            rank: None,
            ath: None,
            ath_date: None,
            icon: None,
        }
    }

    fn cfg(alerts: &str) -> Config {
        Config::parse(&format!("coins = [\"bitcoin\"]\n{alerts}")).unwrap()
    }

    #[test]
    fn un_plafond_franchi_declenche() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]);
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers.first().unwrap().kind, Kind::Above);
    }

    #[test]
    fn un_plafond_non_atteint_ne_declenche_pas() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        assert!(evaluate(&config, &[quote("bitcoin", "BTC", 99_000.0, None)]).is_empty());
    }

    #[test]
    fn un_plancher_franchi_declenche() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nbelow = 50000\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 49_000.0, None)]);
        assert_eq!(triggers.first().unwrap().kind, Kind::Below);
    }

    #[test]
    fn lamplitude_compte_dans_les_deux_sens() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nchange_pct = 5\n");
        assert_eq!(
            evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(-6.0))]).len(),
            1
        );
        assert_eq!(
            evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(6.0))]).len(),
            1
        );
        assert!(evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(2.0))]).is_empty());
    }

    #[test]
    fn une_variation_absente_ne_declenche_pas() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nchange_pct = 5\n");
        assert!(evaluate(&config, &[quote("bitcoin", "BTC", 1.0, None)]).is_empty());
        assert!(evaluate(&config, &[quote("bitcoin", "BTC", 1.0, Some(f64::NAN))]).is_empty());
    }

    #[test]
    fn une_alerte_desactivee_est_ignoree() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 1\nenabled = false\n");
        assert!(evaluate(&config, &[quote("bitcoin", "BTC", 100.0, None)]).is_empty());
    }

    #[test]
    fn sans_cotation_rien_ne_declenche_ni_ne_se_rearme() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 1\n");
        assert!(evaluate(&config, &[]).is_empty());
    }

    #[test]
    fn une_condition_deja_tiree_ne_renotifie_pas() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]);
        let (fresh, fired) = diff(&triggers, &BTreeSet::new());
        assert_eq!(fresh.len(), 1);
        let (again, still) = diff(&triggers, &fired);
        assert!(again.is_empty());
        assert_eq!(still, fired);
    }

    #[test]
    fn une_condition_retombee_se_rearme() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let (_, fired) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &BTreeSet::new(),
        );
        // Le prix repasse sous le seuil : plus aucune condition vraie.
        let (_, cleared) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 90_000.0, None)]),
            &fired,
        );
        assert!(cleared.is_empty());
        // Puis il repasse au-dessus : ça doit notifier de nouveau.
        let (fresh, _) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &cleared,
        );
        assert_eq!(fresh.len(), 1);
    }

    #[test]
    fn changer_le_seuil_rearme_lalerte() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\n");
        let (_, fired) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &BTreeSet::new(),
        );
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100500\n");
        let (fresh, _) = diff(
            &evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, None)]),
            &fired,
        );
        assert_eq!(fresh.len(), 1);
    }

    #[test]
    fn un_bloc_peut_cumuler_plusieurs_conditions() {
        let config = cfg("[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\nchange_pct = 5\n");
        let triggers = evaluate(&config, &[quote("bitcoin", "BTC", 101_000.0, Some(9.0))]);
        assert_eq!(triggers.len(), 2);
    }
}
