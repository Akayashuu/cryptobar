//! Identity badges: one glyph and one brand colour per cryptocurrency.
//!
//! No image is embedded or installed. Each badge boils down to a Unicode
//! character and a colour, which every bar and every graphical toolkit can draw
//! without a dependency: a coloured disc with the glyph on top. Unknown coins
//! fall back to their initial and to a deterministic colour, so a given symbol
//! always keeps the same appearance.

/// Visual identity of a cryptocurrency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    /// A single character to draw at the centre of the disc.
    pub glyph: String,
    /// Disc colour, in `#rrggbb` form.
    pub color: String,
    /// Colour readable on top of [`Badge::color`], in `#rrggbb` form.
    pub foreground: String,
}

/// Known symbols: a currency glyph when one exists, a geometric shape close to
/// the logo otherwise, plus the official brand colour.
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

/// Fallback palette, picked to stay readable on light and dark backgrounds alike.
const FALLBACK: &[&str] = &[
    "#e07a5f", "#3d8ac0", "#81b29a", "#c9a227", "#9b5de5", "#f15bb5", "#00b4a6", "#8d99ae",
];

/// Badge of a symbol, whatever it is.
///
/// The case of the symbol does not matter.
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

/// First letter of the symbol, or a filled disc when it is empty.
fn initial(upper: &str) -> String {
    upper
        .chars()
        .next()
        .map_or_else(|| "●".to_owned(), String::from)
}

/// Stable colour derived from the symbol: the same symbol always yields the
/// same hue, across machines too — FNV-1a is specified, whereas the standard
/// library hash guarantees nothing between two runs.
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

/// Black or white, whichever contrasts best with the given colour.
///
/// The relative luminance follows the ITU-R BT.601 weighting, good enough here
/// and far cheaper than a full linear sRGB conversion.
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

/// Splits `#rrggbb` into three bytes.
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
    fn known_symbols_carry_their_glyph_and_colour() {
        let bitcoin = of("btc");
        assert_eq!(bitcoin.glyph, "₿");
        assert_eq!(bitcoin.color, "#f7931a");
    }

    #[test]
    fn the_case_of_the_symbol_does_not_matter() {
        assert_eq!(of("Eth"), of("ETH"));
    }

    #[test]
    fn an_unknown_symbol_falls_back_to_its_initial() {
        let badge = of("wxyz");
        assert_eq!(badge.glyph, "W");
        assert!(badge.color.starts_with('#'));
    }

    #[test]
    fn the_fallback_colour_is_stable() {
        assert_eq!(of("wxyz").color, of("WXYZ").color);
    }

    #[test]
    fn an_empty_symbol_does_not_panic() {
        assert_eq!(of("").glyph, "●");
    }

    #[test]
    fn the_text_contrasts_with_the_disc() {
        // Light gold: black has to win; dark blue: white.
        assert_eq!(of("bnb").foreground, "#101010");
        assert_eq!(of("ada").foreground, "#ffffff");
    }
}
