use gtk4::prelude::*;

/// Splits a `#rrggbb` colour into `0.0..=1.0` components.
///
/// An unexpected string yields a neutral grey: the ring stays readable.
pub(super) fn rgb(color: &str) -> (f64, f64, f64) {
    let parse = |range: std::ops::Range<usize>| {
        color
            .get(range)
            .and_then(|slice| u8::from_str_radix(slice, 16).ok())
            .map(|byte| f64::from(byte) / 255.0)
    };
    match (parse(1..3), parse(3..5), parse(5..7)) {
        (Some(red), Some(green), Some(blue)) => (red, green, blue),
        _ => (0.5, 0.5, 0.5),
    }
}

/// Reads a number typed the English way as well as the French way.
///
/// A lone comma is a decimal point; a comma next to a point is a thousands
/// separator and is dropped.
fn parse_number(text: &str) -> Option<f64> {
    let compact = text.replace(['\u{202f}', '\u{a0}', ' '], "");
    let normalised = if compact.contains('.') {
        compact.replace(',', "")
    } else {
        compact.replace(',', ".")
    };
    normalised
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Numeric content of a field, or `None` when it is empty or unreadable.
pub(super) fn field(entry: &gtk4::Entry) -> Option<f64> {
    let text = entry.text().to_string();
    if text.trim().is_empty() {
        return None;
    }
    parse_number(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quantity_is_typed_the_english_way_as_well_as_the_french_way() {
        assert_eq!(parse_number("0,25"), Some(0.25));
        assert_eq!(parse_number(" 1 234.5 "), Some(1234.5));
        assert_eq!(parse_number("1,234.5"), Some(1234.5));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number(""), None);
    }

    #[test]
    fn an_unreadable_colour_yields_a_grey() {
        assert_eq!(rgb("#ffffff"), (1.0, 1.0, 1.0));
        assert_eq!(rgb("#000000"), (0.0, 0.0, 0.0));
        assert_eq!(rgb("blue"), (0.5, 0.5, 0.5));
    }
}
