use std::str::FromStr;

/// How markup is handled in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Markup {
    /// Pango tags and escaped entities: what Waybar expects.
    #[default]
    Pango,
    /// Plain text, for bars that interpret no markup at all.
    None,
}

impl Markup {
    /// Applies the escaping matching untrusted data.
    pub(super) fn escape(self, raw: &str) -> String {
        match self {
            Self::Pango => escape_pango(raw),
            Self::None => raw
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .collect(),
        }
    }

    /// Opens and closes a bold run, or nothing at all.
    pub(super) fn bold(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<b>{raw}</b>"),
            Self::None => raw.to_owned(),
        }
    }

    /// Formats a quiet note.
    pub(super) fn italic(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<i>{raw}</i>"),
            Self::None => raw.to_owned(),
        }
    }
}

impl FromStr for Markup {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pango" => Ok(Self::Pango),
            "none" | "plain" => Ok(Self::None),
            other => Err(format!(
                "unknown markup: {other}; expected `pango` or `none`"
            )),
        }
    }
}

/// Escapes the characters Pango would read as markup.
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
            character if character.is_control() => out.push(' '),
            character => out.push(character),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::waybar::fixtures::{config, no_onchain, quote};
    use crate::render::waybar::render;

    #[test]
    fn pango_markup_is_escaped() {
        let module = render(
            &[quote("<b>&x", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
            &no_onchain(),
        );
        assert!(!module.text.contains("<b>"));
        assert!(module.text.contains("&lt;"));
        assert!(module.text.contains("&amp;"));
    }

    #[test]
    fn the_markup_free_mode_yields_no_tag_and_no_entity() {
        let quotes = vec![quote("<b>&x", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let module = render(&quotes, 0, &config(), true, Markup::None, &no_onchain());
        for field in [&module.text, &module.tooltip] {
            assert!(!field.contains("<b>"), "leftover tag: {field}");
            assert!(!field.contains("<i>"), "leftover tag: {field}");
            assert!(!field.contains("&amp;"), "leftover entity: {field}");
        }
        assert!(module.text.contains("<B>&X"), "text: {}", module.text);
    }

    #[test]
    fn the_markup_free_mode_still_neutralises_controls() {
        let module = render(
            &[quote("a\nb", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::None,
            &no_onchain(),
        );
        assert!(!module.text.contains('\n'));
    }

    #[test]
    fn the_markup_parses_from_a_string() {
        assert_eq!("pango".parse(), Ok(Markup::Pango));
        assert_eq!("none".parse(), Ok(Markup::None));
        "bold".parse::<Markup>().unwrap_err();
    }

    #[test]
    fn control_characters_are_neutralised() {
        assert_eq!(escape_pango("a\nb"), "a b");
    }
}
