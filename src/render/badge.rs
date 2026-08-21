//! Pastilles d'identité : un glyphe et une couleur de marque par cryptomonnaie.
//!
//! Aucune image n'est embarquée ni installée. Chaque pastille se réduit à un
//! caractère Unicode et à une couleur, ce que toutes les barres et toutes les
//! boîtes à outils graphiques savent dessiner sans dépendance : un disque
//! coloré, le glyphe par-dessus. Les monnaies inconnues retombent sur leur
//! initiale et sur une couleur déterministe, si bien qu'un symbole donné garde
//! toujours la même apparence.

/// Identité visuelle d'une cryptomonnaie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    /// Un seul caractère à dessiner au centre du disque.
    pub glyph: String,
    /// Couleur du disque, au format `#rrggbb`.
    pub color: String,
    /// Couleur lisible par-dessus [`Badge::color`], au format `#rrggbb`.
    pub foreground: String,
}

/// Symboles reconnus : glyphe monétaire quand il en existe un, forme
/// géométrique proche du logo sinon, et couleur officielle de la marque.
const KNOWN: &[(&str, &str, &str)] = &[
    ("BTC", "₿", "#f7931a"),
    ("ETH", "Ξ", "#627eea"),
    ("USDT", "₮", "#26a17b"),
    ("USDC", "$", "#2775ca"),
    ("BNB", "◈", "#f3ba2f"),
    ("SOL", "◎", "#14f195"),
    ("XRP", "✕", "#00a5df"),
    ("ADA", "₳", "#0033ad"),
    ("DOGE", "Ð", "#c2a633"),
    ("TRX", "▽", "#ef0027"),
    ("TON", "◆", "#0098ea"),
    ("AVAX", "▲", "#e84142"),
    ("DOT", "●", "#e6007a"),
    ("LINK", "⬡", "#2a5ada"),
    ("MATIC", "⬢", "#8247e5"),
    ("POL", "⬢", "#8247e5"),
    ("LTC", "Ł", "#345d9d"),
    ("XMR", "ɱ", "#ff6600"),
    ("ATOM", "⚛", "#6f7390"),
    ("SHIB", "🐕", "#f00500"),
    ("XLM", "✧", "#7d00ff"),
    ("ETC", "Ξ", "#328332"),
    ("BCH", "₿", "#0ac18e"),
    ("NEAR", "◇", "#00c1de"),
    ("APT", "▣", "#4ba8a3"),
    ("ARB", "◐", "#12aaff"),
    ("OP", "◉", "#ff0420"),
    ("SUI", "◍", "#4da2ff"),
];

/// Palette de repli, choisie pour rester lisible sur fond clair comme sombre.
const FALLBACK: &[&str] = &[
    "#e07a5f", "#3d8ac0", "#81b29a", "#c9a227", "#9b5de5", "#f15bb5", "#00b4a6", "#8d99ae",
];

/// Pastille d'un symbole, quel qu'il soit.
///
/// La casse du symbole est sans importance.
#[must_use]
pub fn of(symbol: &str) -> Badge {
    let upper = symbol.to_uppercase();
    let (glyph, color) = KNOWN
        .iter()
        .find(|(known, _, _)| *known == upper)
        .map_or_else(
            || (initial(&upper), fallback_color(&upper).to_owned()),
            |(_, glyph, color)| ((*glyph).to_owned(), (*color).to_owned()),
        );
    let foreground = readable_over(&color).to_owned();
    Badge {
        glyph,
        color,
        foreground,
    }
}

/// Première lettre du symbole, ou un disque plein s'il est vide.
fn initial(upper: &str) -> String {
    upper
        .chars()
        .next()
        .map_or_else(|| "●".to_owned(), String::from)
}

/// Couleur stable dérivée du symbole : le même symbole donne toujours la même
/// teinte, y compris d'une machine à l'autre — FNV-1a est spécifié, là où le
/// hachage de la bibliothèque standard ne garantit rien entre deux exécutions.
fn fallback_color(upper: &str) -> &'static str {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let hash = upper.bytes().fold(OFFSET, |accumulator, byte| {
        (accumulator ^ u64::from(byte)).wrapping_mul(PRIME)
    });
    let modulus = u64::try_from(FALLBACK.len()).unwrap_or(1).max(1);
    let index = usize::try_from(hash.rem_euclid(modulus)).unwrap_or(0);
    FALLBACK.get(index).copied().unwrap_or("#8d99ae")
}

/// Noir ou blanc, suivant ce qui contraste le mieux avec la couleur donnée.
///
/// La luminance relative suit la pondération de la recommandation ITU-R
/// BT.601, suffisante ici et bien moins coûteuse qu'une conversion sRGB
/// linéaire complète.
fn readable_over(hex: &str) -> &'static str {
    let Some((red, green, blue)) = components(hex) else {
        return "#ffffff";
    };
    let luminance = 299_u32
        .saturating_mul(u32::from(red))
        .saturating_add(587_u32.saturating_mul(u32::from(green)))
        .saturating_add(114_u32.saturating_mul(u32::from(blue)));
    if luminance > 150_000 {
        "#101010"
    } else {
        "#ffffff"
    }
}

/// Décompose `#rrggbb` en trois octets.
fn components(hex: &str) -> Option<(u8, u8, u8)> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(digits.get(0..2)?, 16).ok()?;
    let green = u8::from_str_radix(digits.get(2..4)?, 16).ok()?;
    let blue = u8::from_str_radix(digits.get(4..6)?, 16).ok()?;
    Some((red, green, blue))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn les_symboles_connus_portent_leur_glyphe_et_leur_couleur() {
        let bitcoin = of("btc");
        assert_eq!(bitcoin.glyph, "₿");
        assert_eq!(bitcoin.color, "#f7931a");
    }

    #[test]
    fn la_casse_du_symbole_est_sans_importance() {
        assert_eq!(of("Eth"), of("ETH"));
    }

    #[test]
    fn un_symbole_inconnu_retombe_sur_son_initiale() {
        let badge = of("wxyz");
        assert_eq!(badge.glyph, "W");
        assert!(badge.color.starts_with('#'));
    }

    #[test]
    fn la_couleur_de_repli_est_stable() {
        assert_eq!(of("wxyz").color, of("WXYZ").color);
    }

    #[test]
    fn un_symbole_vide_ne_panique_pas() {
        assert_eq!(of("").glyph, "●");
    }

    #[test]
    fn le_texte_contraste_avec_le_disque() {
        // Or clair : le noir doit l'emporter ; bleu foncé : le blanc.
        assert_eq!(of("bnb").foreground, "#101010");
        assert_eq!(of("ada").foreground, "#ffffff");
    }
}
