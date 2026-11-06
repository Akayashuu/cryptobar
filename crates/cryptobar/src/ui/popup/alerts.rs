use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation};

use crate::model::{CoinId, Currency, Quote};
use crate::render::format;

use super::model::Ctx;
use super::parse::field;
use super::widgets::clear;

impl Ctx {
    /// Alerts card: those already set, and what it takes to add one.
    pub(super) fn draw_alerts(self: &Rc<Self>, currency: &Currency, decimals: format::Decimals) {
        clear(&self.widgets.alerts);
        let model = self.model.borrow();
        let ids = model.app.config.coins.clone();
        let alerts = model.app.config.alerts.clone();
        let quotes = model.quotes.clone();
        drop(model);

        let card = GtkBox::new(Orientation::Vertical, 10);
        card.add_css_class("card");
        let title = Label::new(Some("ALERTS"));
        title.add_css_class("title");
        title.set_xalign(0.0);
        card.append(&title);

        if alerts.is_empty() {
            let empty = Label::new(Some(
                "No alert. Watch mode (`bar --watch`) notifies as soon as a threshold is crossed.",
            ));
            empty.add_css_class("muted");
            empty.set_xalign(0.0);
            card.append(&empty);
        }

        for (index, alert) in alerts.iter().enumerate() {
            let row = GtkBox::new(Orientation::Horizontal, 10);
            row.add_css_class("wrow");
            let symbol = Label::new(Some(
                &quotes
                    .iter()
                    .find(|quote| quote.id == alert.coin)
                    .map_or_else(
                        || alert.coin.as_str().to_owned(),
                        |quote| quote.symbol.to_uppercase(),
                    ),
            ));
            symbol.add_css_class("rowsym");
            symbol.set_xalign(0.0);
            symbol.set_size_request(56, -1);
            row.append(&symbol);

            let caption = Label::new(Some(&alert_caption(alert, currency, decimals)));
            caption.add_css_class("muted");
            caption.set_hexpand(true);
            caption.set_xalign(0.0);
            row.append(&caption);

            let remove = Button::with_label("✕");
            remove.add_css_class("ghost");
            remove.set_valign(Align::Center);
            remove.set_tooltip_text(Some("Delete the alert"));
            let ctx = Rc::clone(self);
            remove.connect_clicked(move |_| {
                let written = ctx.app().editor().remove_alert(index);
                ctx.adopt(written, "alert deleted");
            });
            row.append(&remove);
            card.append(&row);
        }

        if !ids.is_empty() {
            card.append(&self.alert_form(&ids, &quotes));
        }

        self.widgets.alerts.append(&card);
    }

    fn alert_form(self: &Rc<Self>, ids: &[CoinId], quotes: &[Quote]) -> GtkBox {
        let ids = ids.to_vec();
        let form = GtkBox::new(Orientation::Horizontal, 8);
        let labels: Vec<String> = ids
            .iter()
            .map(|id| {
                quotes.iter().find(|quote| quote.id == *id).map_or_else(
                    || id.as_str().to_owned(),
                    |quote| quote.symbol.to_uppercase(),
                )
            })
            .collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let picker = gtk4::DropDown::from_strings(&refs);
        picker.set_valign(Align::Center);
        form.append(&picker);

        let above = gtk4::Entry::new();
        above.set_placeholder_text(Some("above"));
        above.set_width_chars(10);
        form.append(&above);
        let below = gtk4::Entry::new();
        below.set_placeholder_text(Some("below"));
        below.set_width_chars(10);
        form.append(&below);
        let change = gtk4::Entry::new();
        change.set_placeholder_text(Some("± %"));
        change.set_width_chars(6);
        form.append(&change);

        let add = Button::with_label("Ajouter");
        add.add_css_class("add");
        add.set_valign(Align::Center);
        let ctx = Rc::clone(self);
        let fields = (above.clone(), below.clone(), change.clone());
        add.connect_clicked(move |_| {
            let index = usize::try_from(picker.selected()).unwrap_or(0);
            let Some(coin) = ids.get(index) else {
                return;
            };
            let (above, below, change) = &fields;
            let written =
                ctx.app()
                    .editor()
                    .add_alert(coin, field(above), field(below), field(change));
            if written.is_ok() {
                above.set_text("");
                below.set_text("");
                change.set_text("");
            }
            ctx.adopt(written, "alert added");
        });
        form.append(&add);
        form
    }
}

fn alert_caption(
    alert: &crate::config::Alert,
    currency: &Currency,
    decimals: format::Decimals,
) -> String {
    let mut parts = Vec::new();
    if let Some(above) = alert.above {
        parts.push(format!(
            "above {}",
            format::price(above, currency, decimals)
        ));
    }
    if let Some(below) = alert.below {
        parts.push(format!(
            "below {}",
            format::price(below, currency, decimals)
        ));
    }
    if let Some(change) = alert.change_pct {
        parts.push(format!("24 h change ≥ {change} %"));
    }
    let mut caption = parts.join(" · ");
    if !alert.enabled {
        caption.push_str("  (disabled)");
    }
    caption
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_alert_caption_describes_every_condition() {
        let config = crate::config::Config::parse(
            "coins = [\"bitcoin\"]\n[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\nchange_pct = 5\n",
        )
        .expect("valid configuration");
        let alert = config.alerts.first().expect("one alert");
        let caption = alert_caption(alert, &Currency::default(), format::Decimals::Auto);
        assert!(caption.contains("above"), "{caption}");
        assert!(caption.contains('%'), "{caption}");
    }
}
