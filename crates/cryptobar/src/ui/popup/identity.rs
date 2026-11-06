use std::fmt::Write as _;

use gtk4::prelude::*;
use gtk4::{Align, Label};

use crate::model::Quote;
use crate::render::badge;

/// Visual identity of a cryptocurrency: its official logo when the cache holds
/// it, otherwise a coloured badge carrying its glyph.
pub(super) fn identity_widget(quote: &Quote, size: i32) -> gtk4::Widget {
    identity_of(quote.icon.as_deref(), &quote.symbol, size)
}

/// Same thing from the raw fields, for what is not a quote — a search result,
/// for instance.
pub(super) fn identity_of(icon: Option<&str>, symbol: &str, size: i32) -> gtk4::Widget {
    if let Some(path) = icon {
        let image = gtk4::Image::from_file(path);
        image.set_pixel_size(size);
        image.set_valign(Align::Center);
        return image.upcast();
    }
    let identity = badge::of(symbol);
    let label = Label::new(Some(&identity.glyph));
    label.add_css_class("badge");
    label.add_css_class(&format!("badge-{}", css_key(symbol)));
    if size <= 28 {
        label.add_css_class("small");
    }
    label.set_valign(Align::Center);
    label.upcast()
}

/// One CSS class per displayed cryptocurrency, carrying its brand colour.
pub(super) fn badge_css(quotes: &[Quote]) -> String {
    let mut sheet = String::new();
    for quote in quotes {
        let identity = badge::of(&quote.symbol);
        let _ = writeln!(
            sheet,
            ".badge-{key} {{ background-color: {color}; color: {foreground}; }}\n\
             .dot-{key} {{ color: {color}; }}",
            key = css_key(&quote.symbol),
            color = identity.color,
            foreground = identity.foreground
        );
    }
    sheet
}

/// Reduces a symbol to a safe CSS class suffix.
pub(super) fn css_key(symbol: &str) -> String {
    symbol
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
