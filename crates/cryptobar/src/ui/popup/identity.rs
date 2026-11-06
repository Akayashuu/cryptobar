use gtk4::prelude::*;
use gtk4::{Align, Label};

use crate::model::Quote;
use crate::render::badge;

pub(super) fn identity_widget(quote: &Quote, size: i32) -> gtk4::Widget {
    identity_of(quote.icon.as_deref(), &quote.symbol, size)
}

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

pub(super) fn badge_css(quotes: &[Quote]) -> String {
    let mut sheet = String::new();
    for quote in quotes {
        let identity = badge::of(&quote.symbol);
        let rule = format!(
            ".badge-{key} {{ background-color: {color}; color: {foreground}; }}\n\
             .dot-{key} {{ color: {color}; }}\n",
            key = css_key(&quote.symbol),
            color = identity.color,
            foreground = identity.foreground
        );
        sheet.push_str(&rule);
    }
    sheet
}

pub(super) fn css_key(symbol: &str) -> String {
    symbol
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}
