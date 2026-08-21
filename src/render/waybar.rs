//! Sérialisation du protocole des modules personnalisés Waybar.
//!
//! Waybar attend une ligne de JSON contenant `text`, `tooltip` et `class`.
//! `text` et `tooltip` sont interprétés comme du balisage Pango : toute donnée
//! venant du réseau doit donc être échappée.
//!
//! D'autres barres — Quickshell, Ags, Eww — consomment le même JSON mais
//! affichent du texte brut. [`Markup::None`] produit alors une sortie sans
//! aucune balise ni entité.

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
    /// Champs additionnels ignorés par Waybar, utiles aux barres capables
    /// de composer leur propre mise en page (Quickshell en vertical, où le
    /// texte complet ne tiendrait pas dans la largeur disponible).
    pub symbol: String,
    pub price: String,
    pub percent: String,
    pub sparkline: String,
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

/// Traitement du balisage dans la sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Markup {
    /// Balises Pango et entités échappées : ce qu'attend Waybar.
    #[default]
    Pango,
    /// Texte brut, pour les barres qui n'interprètent aucun balisage.
    None,
}

impl Markup {
    /// Applique l'échappement correspondant à une donnée non fiable.
    fn escape(self, raw: &str) -> String {
        match self {
            Self::Pango => escape_pango(raw),
            // Les caractères de contrôle sont retirés dans tous les cas :
            // ils casseraient la ligne JSON attendue.
            Self::None => raw
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect(),
        }
    }

    /// Ouvre et ferme une mise en gras, ou rien du tout.
    fn bold(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<b>{raw}</b>"),
            Self::None => raw.to_owned(),
        }
    }

    /// Met en forme une note discrète.
    fn italic(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<i>{raw}</i>"),
            Self::None => raw.to_owned(),
        }
    }
}

impl std::str::FromStr for Markup {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pango" => Ok(Self::Pango),
            "none" | "plain" => Ok(Self::None),
            other => Err(format!(
                "balisage inconnu : {other} ; attendu `pango` ou `none`"
            )),
        }
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
pub fn render(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
) -> Module {
    let Some(quote) = quotes.get(active).or_else(|| quotes.first()) else {
        return Module {
            text: "crypto —".to_owned(),
            tooltip: "Aucune donnée disponible".to_owned(),
            class: "error".to_owned(),
            symbol: "—".to_owned(),
            price: String::new(),
            percent: String::new(),
            sparkline: String::new(),
        };
    };

    let decimals = config.decimals();
    let symbol = markup.escape(&quote.symbol.to_uppercase());
    let price = markup.escape(&format::price(quote.price, &config.currency, decimals));
    let percent = markup.escape(&format::percent(quote.change_24h));
    let spark = if config.sparkline && !quote.sparkline.is_empty() {
        sparkline::sparkline(&quote.sparkline, config.sparkline_width)
    } else {
        String::new()
    };

    let mut text = format!("{symbol} {price}");
    if !spark.is_empty() {
        text.push(' ');
        text.push_str(&spark);
    }
    text.push(' ');
    text.push_str(&percent);

    Module {
        text,
        tooltip: tooltip(quotes, active, config, stale, markup),
        class: class(quote, stale),
        symbol,
        price,
        percent,
        sparkline: spark,
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

fn tooltip(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
) -> String {
    let decimals = config.decimals();
    let mut lines = Vec::with_capacity(quotes.len().saturating_add(2));
    for (index, quote) in quotes.iter().enumerate() {
        let marker = if index == active { "▸" } else { " " };
        lines.push(format!(
            "{marker} {}  {}   1h {}  24h {}  7j {}",
            markup.bold(&markup.escape(&quote.symbol.to_uppercase())),
            markup.escape(&format::price(quote.price, &config.currency, decimals)),
            markup.escape(&format::percent(quote.change_1h)),
            markup.escape(&format::percent(quote.change_24h)),
            markup.escape(&format::percent(quote.change_7d)),
        ));
    }
    if stale {
        lines.push(String::new());
        lines.push(markup.italic("données périmées — réseau indisponible"));
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
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::Pango,
        );
        assert!(module.text.contains("BTC"));
        assert!(module.text.contains("42"));
        assert!(module.text.contains("1,5"), "texte : {}", module.text);
    }

    #[test]
    fn la_classe_suit_la_tendance() {
        let cfg = config();
        assert_eq!(
            render(
                &[quote("btc", 1.0, Some(1.0))],
                0,
                &cfg,
                false,
                Markup::Pango
            )
            .class,
            "up"
        );
        assert_eq!(
            render(
                &[quote("btc", 1.0, Some(-1.0))],
                0,
                &cfg,
                false,
                Markup::Pango
            )
            .class,
            "down"
        );
    }

    #[test]
    fn letat_perime_sajoute_a_la_classe_et_au_tooltip() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            true,
            Markup::Pango,
        );
        assert!(module.class.contains("stale"));
        assert!(module.tooltip.contains("périmées"));
    }

    #[test]
    fn une_liste_vide_donne_un_module_derreur_et_non_un_panic() {
        let module = render(&[], 0, &config(), false, Markup::Pango);
        assert_eq!(module.class, "error");
    }

    #[test]
    fn un_indice_hors_bornes_retombe_sur_la_premiere_cotation() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            9,
            &config(),
            false,
            Markup::Pango,
        );
        assert!(module.text.contains("BTC"));
    }

    #[test]
    fn le_balisage_pango_est_echappe() {
        let module = render(
            &[quote("<b>&x", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
        );
        assert!(!module.text.contains("<b>"));
        assert!(module.text.contains("&lt;"));
        assert!(module.text.contains("&amp;"));
    }

    #[test]
    fn le_tooltip_marque_la_cotation_active() {
        let quotes = vec![quote("btc", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let tooltip = render(&quotes, 1, &config(), false, Markup::Pango).tooltip;
        let second = tooltip.lines().nth(1).unwrap();
        assert!(second.starts_with('▸'));
    }

    #[test]
    fn la_sortie_est_du_json_sur_une_seule_ligne() {
        let line = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
        )
        .to_line()
        .unwrap();
        assert_eq!(line.lines().count(), 1);
        assert!(line.starts_with('{'));
    }

    #[test]
    fn le_mode_sans_balisage_ne_produit_ni_balise_ni_entite() {
        let quotes = vec![quote("<b>&x", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let module = render(&quotes, 0, &config(), true, Markup::None);
        for field in [&module.text, &module.tooltip] {
            // Aucune balise n'est ajoutée par le rendu : ni <b> ni <i>.
            assert!(!field.contains("<b>"), "balise résiduelle : {field}");
            assert!(!field.contains("<i>"), "balise résiduelle : {field}");
            assert!(!field.contains("&amp;"), "entité résiduelle : {field}");
        }
        // La donnée réseau ressort littérale : c'est au consommateur de
        // l'afficher en texte brut (Quickshell : `textFormat: Text.PlainText`).
        assert!(module.text.contains("<B>&X"), "texte : {}", module.text);
    }

    #[test]
    fn le_mode_sans_balisage_neutralise_quand_meme_les_controles() {
        let module = render(
            &[quote("a\nb", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::None,
        );
        assert!(!module.text.contains('\n'));
    }

    #[test]
    fn le_balisage_est_parse_depuis_une_chaine() {
        assert_eq!("pango".parse(), Ok(Markup::Pango));
        assert_eq!("none".parse(), Ok(Markup::None));
        assert!("gras".parse::<Markup>().is_err());
    }

    #[test]
    fn les_champs_structures_accompagnent_le_texte() {
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::None,
        );
        assert_eq!(module.symbol, "BTC");
        assert!(module.price.contains("42"));
        assert!(module.percent.contains("1,5"));
        assert!(!module.sparkline.is_empty());
        // Le texte reste la concaténation des champs : aucune divergence
        // possible entre les deux représentations.
        assert_eq!(
            module.text,
            format!(
                "{} {} {} {}",
                module.symbol, module.price, module.sparkline, module.percent
            )
        );
    }

    #[test]
    fn les_caracteres_de_controle_sont_neutralises() {
        assert_eq!(escape_pango("a\nb"), "a b");
    }
}
