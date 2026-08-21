//! Sous-commande `coins` : recherche d'identifiants CoinGecko.
//!
//! Le fichier de configuration attend des identifiants, pas des symboles.
//! Cette commande fait le pont pour l'utilisateur.

use std::fmt::Write as _;
use std::io::Write as _;

use anyhow::{Context as _, Result};

use crate::app::App;
use crate::source::SearchHit;

/// Nombre de résultats affichés : au-delà, la liste cesse d'être utile.
const MAX_RESULTS: usize = 15;

/// Recherche `term` et affiche les identifiants correspondants.
///
/// # Errors
/// Échoue si la recherche réseau échoue — elle n'est pas cachée.
pub fn run(app: &App, term: &str) -> Result<()> {
    let hits = app.source.search(term).context("recherche impossible")?;
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "{}", render(&hits, term))?;
    stdout.flush()?;
    Ok(())
}

/// Met en forme les résultats. Fonction pure : testable sans réseau.
#[must_use]
pub fn render(hits: &[SearchHit], term: &str) -> String {
    if hits.is_empty() {
        return format!("aucun résultat pour « {term} »\n");
    }
    let shown: Vec<&SearchHit> = hits.iter().take(MAX_RESULTS).collect();
    let id_width = shown
        .iter()
        .map(|h| h.id.as_str().chars().count())
        .max()
        .unwrap_or(0);
    let symbol_width = shown
        .iter()
        .map(|h| h.symbol.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for hit in &shown {
        // L'écriture dans une `String` est infaillible.
        let _ = writeln!(
            out,
            "{:<id_width$}  {:<symbol_width$}  {}",
            hit.id.as_str(),
            hit.symbol.to_uppercase(),
            hit.name,
        );
    }
    if hits.len() > shown.len() {
        let _ = writeln!(
            out,
            "… {} autres résultats",
            hits.len().saturating_sub(shown.len())
        );
    }
    out.push_str(
        "\nAjoutez l'identifiant de la première colonne à `coins` dans la configuration.\n",
    );
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::CoinId;

    fn hit(id: &str, symbol: &str, name: &str) -> SearchHit {
        SearchHit {
            id: CoinId::parse(id).unwrap(),
            symbol: symbol.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn une_recherche_vide_le_dit_explicitement() {
        let out = render(&[], "zzz");
        assert!(out.contains("aucun résultat"));
        assert!(out.contains("zzz"));
    }

    #[test]
    fn chaque_resultat_montre_identifiant_symbole_et_nom() {
        let out = render(&[hit("bitcoin", "btc", "Bitcoin")], "btc");
        let first = out.lines().next().unwrap();
        assert!(first.contains("bitcoin"));
        assert!(first.contains("BTC"));
        assert!(first.contains("Bitcoin"));
    }

    #[test]
    fn les_colonnes_sont_alignees() {
        let hits = vec![
            hit("btc", "btc", "Bitcoin"),
            hit("ethereum", "eth", "Ethereum"),
        ];
        let out = render(&hits, "b");
        let positions: Vec<_> = out
            .lines()
            .take(2)
            .map(|l| l.find("BTC").or(l.find("ETH")))
            .collect();
        assert_eq!(positions.first(), positions.get(1));
    }

    #[test]
    fn la_liste_est_tronquee_et_le_reste_est_annonce() {
        let hits: Vec<SearchHit> = (0..MAX_RESULTS + 5)
            .map(|_| hit("bitcoin", "btc", "B"))
            .collect();
        let out = render(&hits, "b");
        assert!(out.contains("5 autres résultats"));
    }
}
