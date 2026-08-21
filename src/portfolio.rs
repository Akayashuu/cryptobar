//! Valorisation du portefeuille.
//!
//! Fonctions pures : les quantités viennent des soldes lus sur les adresses
//! publiques suivies, les prix des cotations déjà en cache. Rien n'est stocké,
//! rien n'est envoyé — cryptobar ne connaît ni exchange ni clé d'API, et les
//! adresses ne quittent jamais la machine.
//!
//! Le prix d'il y a 24 h est reconstitué depuis la variation relative plutôt
//! que par une requête historique : `p₀ = p / (1 + v/100)`. C'est exact à la
//! précision de la variation publiée, et ça ne coûte aucun appel réseau.

use std::collections::BTreeMap;

use crate::model::{CoinId, Quote};
use crate::render::badge;

/// Une ligne du portefeuille.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub symbol: String,
    /// Identifiant de la cryptomonnaie, pour retrouver la ligne côté interface.
    pub id: CoinId,
    pub name: String,
    pub quantity: f64,
    pub price: f64,
    /// Quantité × prix, dans la devise de cotation.
    pub value: f64,
    /// Part du total, dans `0.0..=1.0`.
    pub share: f64,
    /// Gain ou perte sur 24 h, dans la devise de cotation.
    pub change_24h: Option<f64>,
    /// Couleur de marque, pour l'anneau de répartition.
    pub color: String,
}

/// Portefeuille valorisé.
#[derive(Debug, Clone, PartialEq)]
pub struct Portfolio {
    /// Positions triées par valeur décroissante : l'œil va d'abord au poids
    /// lourd, dans le tableau comme dans l'anneau.
    pub positions: Vec<Position>,
    pub total: f64,
    /// Gain ou perte sur 24 h, dans la devise de cotation.
    pub change_24h: Option<f64>,
    /// La même chose en pourcentage du portefeuille d'il y a 24 h.
    pub change_24h_pct: Option<f64>,
}

/// Valorise le portefeuille, ou `None` si aucune adresse suivie n'a de solde.
///
/// Une pièce déclarée mais sans cotation est ignorée plutôt que valorisée à
/// zéro : un total faussement effondré serait pire qu'un total partiel.
#[must_use]
pub fn compute(quotes: &[Quote], onchain: &BTreeMap<CoinId, f64>) -> Option<Portfolio> {
    let mut positions: Vec<Position> = onchain
        .iter()
        .filter(|(_, quantity)| quantity.is_finite() && **quantity > 0.0)
        .filter_map(|(id, quantity)| {
            let quote = quotes.iter().find(|quote| quote.id == *id)?;
            let value = quote.price * quantity;
            if !value.is_finite() {
                return None;
            }
            Some(Position {
                symbol: quote.symbol.clone(),
                id: quote.id.clone(),
                name: quote.name.clone(),
                quantity: *quantity,
                price: quote.price,
                value,
                share: 0.0,
                change_24h: gain_24h(quote.price, quote.change_24h).map(|gain| gain * quantity),
                color: badge::of(&quote.symbol).color,
            })
        })
        .collect();

    if positions.is_empty() {
        return None;
    }

    let total: f64 = positions.iter().map(|position| position.value).sum();
    if total > 0.0 {
        for position in &mut positions {
            position.share = position.value / total;
        }
    }
    positions.sort_by(|a, b| b.value.total_cmp(&a.value));

    // Un portefeuille dont aucune ligne ne publie de variation n'a pas de
    // variation : mieux vaut « — » qu'un zéro qui se lirait comme « stable ».
    let change_24h = positions
        .iter()
        .any(|position| position.change_24h.is_some())
        .then(|| positions.iter().filter_map(|p| p.change_24h).sum::<f64>());
    let change_24h_pct = change_24h.and_then(|gain| {
        let before = total - gain;
        (before > 0.0).then(|| gain / before * 100.0)
    });

    Some(Portfolio {
        positions,
        total,
        change_24h,
        change_24h_pct,
    })
}

/// Gain sur 24 h pour une unité, depuis le prix courant et la variation en
/// pourcentage.
fn gain_24h(price: f64, change_pct: Option<f64>) -> Option<f64> {
    let change = change_pct.filter(|change| change.is_finite())?;
    let factor = 1.0 + change / 100.0;
    // Une variation de -100 % annulerait le dénominateur : on préfère ne rien
    // afficher qu'un infini.
    if factor.abs() < f64::EPSILON {
        return None;
    }
    let before = price / factor;
    let gain = price - before;
    gain.is_finite().then_some(gain)
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

    fn held(entries: &[(&str, f64)]) -> BTreeMap<CoinId, f64> {
        entries
            .iter()
            .map(|(id, quantity)| (CoinId::parse(id).unwrap(), *quantity))
            .collect()
    }

    #[test]
    fn sans_solde_il_ny_a_pas_de_portefeuille() {
        assert!(compute(&[quote("bitcoin", "BTC", 100.0, None)], &BTreeMap::new()).is_none());
    }

    #[test]
    fn un_solde_nul_est_ignore() {
        assert!(
            compute(
                &[quote("bitcoin", "BTC", 100.0, None)],
                &held(&[("bitcoin", 0.0)])
            )
            .is_none()
        );
    }

    #[test]
    fn le_total_est_la_somme_des_lignes() {
        let portfolio = compute(
            &[
                quote("bitcoin", "BTC", 100.0, None),
                quote("ethereum", "ETH", 5.0, None),
            ],
            &held(&[("bitcoin", 2.0), ("ethereum", 10.0)]),
        )
        .unwrap();
        assert!((portfolio.total - 250.0).abs() < 1e-9);
        assert_eq!(portfolio.positions.len(), 2);
    }

    #[test]
    fn les_lignes_sont_triees_par_valeur_decroissante() {
        let portfolio = compute(
            &[
                quote("bitcoin", "BTC", 10.0, None),
                quote("ethereum", "ETH", 5.0, None),
            ],
            &held(&[("bitcoin", 1.0), ("ethereum", 10.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.positions.first().unwrap().symbol, "ETH");
        assert!((portfolio.positions.first().unwrap().share - 50.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn une_piece_sans_cotation_est_ignoree_plutot_que_valorisee_a_zero() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 100.0, None)],
            &held(&[("bitcoin", 1.0), ("ethereum", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.positions.len(), 1);
        assert!((portfolio.total - 100.0).abs() < 1e-9);
    }

    #[test]
    fn une_quantite_non_finie_est_ignoree() {
        assert!(
            compute(
                &[quote("bitcoin", "BTC", 100.0, None)],
                &held(&[("bitcoin", f64::NAN)])
            )
            .is_none()
        );
    }

    #[test]
    fn le_gain_24h_se_deduit_de_la_variation() {
        // 110 aujourd'hui après +10 % : 100 hier, donc +10 par unité.
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 110.0, Some(10.0))],
            &held(&[("bitcoin", 3.0)]),
        )
        .unwrap();
        assert!((portfolio.change_24h.unwrap() - 30.0).abs() < 1e-9);
        assert!((portfolio.change_24h_pct.unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn une_perte_est_negative() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, Some(-10.0))],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert!(portfolio.change_24h.unwrap() < 0.0);
        assert!((portfolio.change_24h_pct.unwrap() + 10.0).abs() < 1e-9);
    }

    #[test]
    fn sans_aucune_variation_publiee_le_gain_est_absent() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, None)],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.change_24h, None);
        assert_eq!(portfolio.change_24h_pct, None);
    }

    #[test]
    fn une_variation_non_finie_ne_produit_pas_dinfini() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, Some(f64::NAN))],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.change_24h, None);
    }

    #[test]
    fn une_chute_de_cent_pour_cent_ne_divise_pas_par_zero() {
        let portfolio = compute(
            &[quote("bitcoin", "BTC", 90.0, Some(-100.0))],
            &held(&[("bitcoin", 1.0)]),
        )
        .unwrap();
        assert_eq!(portfolio.change_24h, None);
    }

    #[test]
    fn les_parts_somment_a_un() {
        let portfolio = compute(
            &[
                quote("bitcoin", "BTC", 60.0, None),
                quote("ethereum", "ETH", 10.0, None),
            ],
            &held(&[("bitcoin", 1.0), ("ethereum", 4.0)]),
        )
        .unwrap();
        let sum: f64 = portfolio.positions.iter().map(|p| p.share).sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }
}
