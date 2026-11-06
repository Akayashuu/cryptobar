//! Number formatting for display. Pure functions, no I/O.

use crate::model::Currency;

/// Thousands separator: narrow no-break space, readable in a narrow bar.
const THIN_SPACE: char = '\u{202f}';
/// Marker for unavailable or non-finite values.
const MISSING: &str = "—";
/// Decimals ceiling: beyond it the display stops being readable.
const MAX_DECIMALS: u8 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decimals {
    /// Number of decimals derived from the order of magnitude.
    Auto,
    Fixed(u8),
}

/// Number of decimals used in automatic mode, so that an asset at €0.00004
/// stays as readable as one at €108,420.
fn auto_decimals(value: f64) -> u8 {
    let magnitude = value.abs();
    if magnitude >= 1_000.0 {
        0
    } else if magnitude >= 1.0 {
        2
    } else if magnitude >= 0.01 {
        4
    } else if magnitude > 0.0 {
        // Four significant digits after the leading zeros.
        #[allow(clippy::cast_possible_truncation)]
        let leading_zeros = (-magnitude.log10().floor()) as i32;
        u8::try_from(leading_zeros.clamp(0, 7).saturating_add(3)).unwrap_or(10)
    } else {
        2
    }
}

/// Inserts the thousands separator into the integer part of an already
/// formatted number.
fn group_thousands(rendered: &str) -> String {
    let (int_part, rest) = match rendered.split_once('.') {
        Some((head, tail)) => (head, Some(tail)),
        None => (rendered, None),
    };
    let (sign, digits) = match int_part.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", int_part),
    };

    // Division by a non-zero constant: the capacity is an estimate, not an invariant.
    #[allow(clippy::integer_division)]
    let capacity = digits.len().saturating_add(digits.len() / 3);
    let mut grouped = String::with_capacity(capacity);
    for (index, ch) in digits.chars().enumerate() {
        let remaining = digits.len().saturating_sub(index);
        if index > 0 && remaining % 3 == 0 {
            grouped.push(THIN_SPACE);
        }
        grouped.push(ch);
    }

    let mut out = String::with_capacity(grouped.len().saturating_add(4));
    out.push_str(sign);
    out.push_str(&grouped);
    if let Some(tail) = rest {
        out.push('.');
        out.push_str(tail);
    }
    out
}

/// Formats a price with its currency. A non-finite value yields `—`.
#[must_use]
pub fn price(value: f64, currency: &Currency, decimals: Decimals) -> String {
    if !value.is_finite() {
        return MISSING.to_owned();
    }
    let places = match decimals {
        Decimals::Auto => auto_decimals(value),
        Decimals::Fixed(n) => n.min(MAX_DECIMALS),
    };
    let rendered = format!("{value:.*}", usize::from(places));
    // In automatic mode, trailing zeros add nothing in a narrow bar.
    let rendered = if matches!(decimals, Decimals::Auto) && rendered.contains('.') {
        rendered
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    } else {
        rendered
    };
    let symbol = currency.symbol();
    let grouped = group_thousands(&rendered);
    if symbol.is_empty() {
        grouped
    } else {
        format!("{grouped}{THIN_SPACE}{symbol}")
    }
}

/// Formats a held quantity, without a currency. Unlike a price, a crypto
/// quantity is often fractional over several decimals: up to eight are kept,
/// without useless zeros.
#[must_use]
pub fn quantity(value: f64) -> String {
    if !value.is_finite() {
        return MISSING.to_owned();
    }
    let rendered = format!("{value:.8}");
    let trimmed = rendered.trim_end_matches('0').trim_end_matches('.');
    group_thousands(trimmed)
}

/// Formats a large amount compactly — market cap, volume — where the exact
/// figure adds nothing but the order of magnitude does.
#[must_use]
pub fn compact(value: f64, currency: &Currency) -> String {
    if !value.is_finite() {
        return MISSING.to_owned();
    }
    let magnitude = value.abs();
    let (scaled, suffix) = if magnitude >= 1e12 {
        (value / 1e12, " T")
    } else if magnitude >= 1e9 {
        (value / 1e9, " B")
    } else if magnitude >= 1e6 {
        (value / 1e6, " M")
    } else if magnitude >= 1e3 {
        (value / 1e3, " k")
    } else {
        (value, "")
    };
    let scaled_text = format!("{scaled:.2}");
    let rendered = scaled_text.trim_end_matches('0').trim_end_matches('.');
    let symbol = currency.symbol();
    format!("{rendered}{suffix}{THIN_SPACE}{symbol}")
}

/// Formats a change in percent, always signed. `None` or non-finite yields `—`.
#[must_use]
pub fn percent(value: Option<f64>) -> String {
    match value {
        Some(v) if v.is_finite() => {
            let rendered = format!("{:.1}", v.abs());
            let sign = if v < 0.0 { '-' } else { '+' };
            format!("{sign}{rendered}{THIN_SPACE}%")
        }
        _ => MISSING.to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::Currency;

    fn eur() -> Currency {
        Currency::parse("eur").unwrap()
    }

    #[test]
    fn quantities_carry_no_useless_zeros() {
        assert_eq!(quantity(2.0), "2");
        assert_eq!(quantity(0.5), "0.5");
        assert_eq!(quantity(0.000_123_45), "0.00012345");
        assert_eq!(quantity(1234.5), "1\u{202f}234.5");
        assert_eq!(quantity(f64::NAN), "—");
    }

    #[test]
    fn compact_amounts_by_order_of_magnitude() {
        assert_eq!(compact(1_500_000_000.0, &eur()), "1.5 B\u{202f}€");
        assert_eq!(compact(2_000_000.0, &eur()), "2 M\u{202f}€");
        assert_eq!(compact(999.0, &eur()), "999\u{202f}€");
        assert_eq!(compact(f64::INFINITY, &eur()), "—");
    }

    #[test]
    fn prices_use_a_thin_space_thousands_separator() {
        assert_eq!(
            price(108_420.0, &eur(), Decimals::Auto),
            "108\u{202f}420\u{202f}€"
        );
        assert_eq!(
            price(3_912.5, &eur(), Decimals::Auto),
            "3\u{202f}912\u{202f}€"
        );
        assert_eq!(price(942.37, &eur(), Decimals::Auto), "942.37\u{202f}€");
    }

    #[test]
    fn micro_asset_prices_stay_readable() {
        assert_eq!(
            price(0.000_042_1, &eur(), Decimals::Auto),
            "0.0000421\u{202f}€"
        );
        assert_eq!(price(0.123_4, &eur(), Decimals::Auto), "0.1234\u{202f}€");
    }

    #[test]
    fn prices_with_pinned_decimals() {
        assert_eq!(
            price(108_420.0, &eur(), Decimals::Fixed(2)),
            "108\u{202f}420.00\u{202f}€"
        );
        assert_eq!(price(1.5, &eur(), Decimals::Fixed(0)), "2\u{202f}€");
    }

    #[test]
    fn a_non_finite_price_yields_a_dash() {
        assert_eq!(price(f64::NAN, &eur(), Decimals::Auto), "—");
        assert_eq!(price(f64::INFINITY, &eur(), Decimals::Auto), "—");
    }

    #[test]
    fn percentages_signed_and_missing() {
        assert_eq!(percent(Some(2.14)), "+2.1\u{202f}%");
        assert_eq!(percent(Some(-0.44)), "-0.4\u{202f}%");
        assert_eq!(percent(Some(0.0)), "+0.0\u{202f}%");
        assert_eq!(percent(None), "—");
        assert_eq!(percent(Some(f64::NAN)), "—");
    }

    proptest::proptest! {
        #[test]
        fn price_never_panics(value in proptest::num::f64::ANY, d in 0u8..12) {
            let _ = price(value, &eur(), Decimals::Auto);
            let _ = price(value, &eur(), Decimals::Fixed(d));
        }

        #[test]
        fn percent_never_panics(value in proptest::num::f64::ANY) {
            let _ = percent(Some(value));
        }
    }
}
