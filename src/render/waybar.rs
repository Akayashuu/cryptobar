//! Sérialisation du protocole des modules personnalisés Waybar.
//!
//! Waybar attend une ligne de JSON contenant `text`, `tooltip` et `class`.
//! `text` et `tooltip` sont interprétés comme du balisage Pango : toute donnée
//! venant du réseau doit donc être échappée.

use serde::Serialize;

use crate::config::Config;
use crate::model::Quote;

use super::{format, sparkline};

/// Charge utile d'un module Waybar.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Module {
    pub text: String,
    pub tooltip: String,
    pub class: String,
}

impl Module {
    /// Sérialise en une ligne de JSON.
    ///
    /// # Errors
    /// Impossible en pratique : la structure ne contient que des chaînes.
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Échappe les caractères que Pango interpréterait comme du balisage.
#[must_use]
pub fn escape_pango(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            // Les caractères de contrôle casseraient la ligne JSON attendue.
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Construit le module à partir de la cotation active et de la liste complète.
#[must_use]
pub fn render(quotes: &[Quote], active: usize, config: &Config, stale: bool) -> Module {
    let Some(quote) = quotes.get(active).or_else(|| quotes.first()) else {
        return Module {
            text: "crypto —".to_owned(),
            tooltip: "Aucune donnée disponible".to_owned(),
            class: "error".to_owned(),
        };
    };

    let decimals = config.decimals();
    let mut text = format!(
        "{} {}",
        escape_pango(&quote.symbol.to_uppercase()),
        escape_pango(&format::price(quote.price, &config.currency, decimals))
    );
    if config.sparkline && !quote.sparkline.is_empty() {
        text.push(' ');
        text.push_str(&sparkline::sparkline(
            &quote.sparkline,
            config.sparkline_width,
        ));
    }
    text.push(' ');
    text.push_str(&escape_pango(&format::percent(quote.change_24h)));

    Module {
        text,
        tooltip: tooltip(quotes, active, config, stale),
        class: class(quote, stale),
    }
}

fn class(quote: &Quote, stale: bool) -> String {
    let trend = quote.trend().class();
    if stale {
        format!("{trend} stale")
    } else {
        trend.to_owned()
    }
}

fn tooltip(quotes: &[Quote], active: usize, config: &Config, stale: bool) -> String {
    let decimals = config.decimals();
    let mut lines = Vec::with_capacity(quotes.len().saturating_add(2));
    for (index, quote) in quotes.iter().enumerate() {
        let marker = if index == active { "▸" } else { " " };
        lines.push(format!(
            "{marker} <b>{}</b>  {}   1h {}  24h {}  7j {}",
            escape_pango(&quote.symbol.to_uppercase()),
            escape_pango(&format::price(quote.price, &config.currency, decimals)),
            escape_pango(&format::percent(quote.change_1h)),
            escape_pango(&format::percent(quote.change_24h)),
            escape_pango(&format::percent(quote.change_7d)),
        ));
    }
    if stale {
        lines.push(String::new());
        lines.push("<i>données périmées — réseau indisponible</i>".to_owned());
    }
    lines.join("\n")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::CoinId;

    fn config() -> Config {
        Config::parse("coins = [\"bitcoin\", \"ethereum\"]").unwrap()
    }

    fn quote(symbol: &str, price: f64, change: Option<f64>) -> Quote {
        Quote {
            id: CoinId::parse("bitcoin").unwrap(),
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            price,
            change_1h: Some(0.5),
            change_24h: change,
            change_7d: Some(-2.0),
            sparkline: vec![1.0, 2.0, 3.0, 2.0],
        }
    }

    #[test]
    fn le_texte_contient_symbole_prix_et_variation() {
        let module = render(&[quote("btc", 42_000.0, Some(1.5))], 0, &config(), false);
        assert!(module.text.contains("BTC"));
        assert!(module.text.contains("42"));
        assert!(module.text.contains("1,5"), "texte : {}", module.text);
    }

    #[test]
    fn la_classe_suit_la_tendance() {
        let cfg = config();
        assert_eq!(
            render(&[quote("btc", 1.0, Some(1.0))], 0, &cfg, false).class,
            "up"
        );
        assert_eq!(
            render(&[quote("btc", 1.0, Some(-1.0))], 0, &cfg, false).class,
            "down"
        );
    }

    #[test]
    fn letat_perime_sajoute_a_la_classe_et_au_tooltip() {
        let module = render(&[quote("btc", 1.0, Some(1.0))], 0, &config(), true);
        assert!(module.class.contains("stale"));
        assert!(module.tooltip.contains("périmées"));
    }

    #[test]
    fn une_liste_vide_donne_un_module_derreur_et_non_un_panic() {
        let module = render(&[], 0, &config(), false);
        assert_eq!(module.class, "error");
    }

    #[test]
    fn un_indice_hors_bornes_retombe_sur_la_premiere_cotation() {
        let module = render(&[quote("btc", 1.0, Some(1.0))], 9, &config(), false);
        assert!(module.text.contains("BTC"));
    }

    #[test]
    fn le_balisage_pango_est_echappe() {
        let module = render(&[quote("<b>&x", 1.0, Some(1.0))], 0, &config(), false);
        assert!(!module.text.contains("<b>"));
        assert!(module.text.contains("&lt;"));
        assert!(module.text.contains("&amp;"));
    }

    #[test]
    fn le_tooltip_marque_la_cotation_active() {
        let quotes = vec![quote("btc", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let tooltip = render(&quotes, 1, &config(), false).tooltip;
        let second = tooltip.lines().nth(1).unwrap();
        assert!(second.starts_with('▸'));
    }

    #[test]
    fn la_sortie_est_du_json_sur_une_seule_ligne() {
        let line = render(&[quote("btc", 1.0, Some(1.0))], 0, &config(), false)
            .to_line()
            .unwrap();
        assert_eq!(line.lines().count(), 1);
        assert!(line.starts_with('{'));
    }

    #[test]
    fn les_caracteres_de_controle_sont_neutralises() {
        assert_eq!(escape_pango("a\nb"), "a b");
    }
}
