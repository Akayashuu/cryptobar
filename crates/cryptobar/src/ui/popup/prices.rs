use std::cell::Cell;
use std::rc::Rc;

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, ListBox, Orientation};

use super::chart::{CHART_HEIGHT, CHART_WIDTH, Hover, chart};
use super::hover::attach_hover;
use super::identity::identity_widget;
use super::model::{Ctx, Model};
use super::status::root_cause;
use super::widgets::{clear, trend_class};
use crate::commands::nav;
use crate::model::{Currency, Quote, Range};
use crate::render::braille;
use crate::render::format;

const SPARK_WIDTH: i32 = 72;
const SPARK_HEIGHT: i32 = 26;

pub(super) fn series_key(id: &str, range: Range) -> String {
    format!("{id}-{}", range.slug())
}

pub(super) fn series_of(model: &Model, quote: &Quote, range: Range) -> Vec<f64> {
    if range == Range::D7 {
        return quote.sparkline.clone();
    }
    model
        .series
        .get(&series_key(quote.id.as_str(), range))
        .cloned()
        .unwrap_or_default()
}

fn range_caption(range: Range) -> String {
    match range {
        Range::H1 => "Last hour".to_owned(),
        Range::H24 => "Last 24 hours".to_owned(),
        Range::D7 => "Last 7 days".to_owned(),
        Range::D30 => "Last 30 days".to_owned(),
        Range::Y1 => "Last 12 months".to_owned(),
    }
}

impl Ctx {
    pub(super) fn chart_card(
        self: &Rc<Self>,
        quote: &Quote,
        series: &[f64],
        range: Range,
        currency: &Currency,
        decimals: format::Decimals,
    ) -> GtkBox {
        let card = GtkBox::new(Orientation::Vertical, 10);
        card.add_css_class("card");

        let ranges = GtkBox::new(Orientation::Horizontal, 4);
        ranges.set_halign(Align::Start);
        for candidate in [Range::H1, Range::H24, Range::D7, Range::D30, Range::Y1] {
            let pill = Button::with_label(candidate.label());
            pill.add_css_class("range");
            if candidate == range {
                pill.add_css_class("on");
            }
            let ctx = Rc::clone(self);
            pill.connect_clicked(move |_| ctx.set_range(candidate));
            ranges.append(&pill);
        }
        card.append(&ranges);

        let positive = match (series.first(), series.last()) {
            (Some(first), Some(last)) => last >= first,
            _ => quote.change_7d.unwrap_or(0.0) >= 0.0,
        };
        let finite: Vec<f64> = series
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .collect();
        let hover: Hover = Rc::new(Cell::new(None));
        let area = chart(
            series,
            positive,
            CHART_WIDTH,
            CHART_HEIGHT,
            true,
            Some(Rc::clone(&hover)),
        );

        let readout = Label::new(Some(" "));
        readout.add_css_class("muted");
        readout.set_xalign(0.0);

        attach_hover(&area, &readout, &hover, &finite, range, currency, decimals);
        card.append(&area);
        card.append(&readout);

        let footer = GtkBox::new(Orientation::Horizontal, 8);
        let label = Label::new(Some(&range_caption(range)));
        label.add_css_class("muted");
        label.set_hexpand(true);
        label.set_xalign(0.0);
        footer.append(&label);

        if finite.len() >= 2 {
            let (low, high) = braille::bounds(&finite);
            let span = Label::new(Some(&format!(
                "{} to {}",
                format::price(low, currency, decimals),
                format::price(high, currency, decimals)
            )));
            span.add_css_class("muted");
            footer.append(&span);
        } else {
            let pending = Label::new(Some("loading…"));
            pending.add_css_class("muted");
            footer.append(&pending);
        }

        self.export_buttons(&footer, &finite, positive, quote, range);

        card.append(&footer);
        card
    }

    pub(super) fn draw_list(
        self: &Rc<Self>,
        active: usize,
        currency: &Currency,
        decimals: format::Decimals,
    ) {
        clear(&self.widgets.list);
        let model = self.model.borrow();
        let ids = model.app.config.coins.clone();
        let quotes = model.quotes.clone();
        drop(model);

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("card");

        for (index, id) in ids.iter().enumerate() {
            let quote = quotes
                .iter()
                .find(|candidate| candidate.id.as_str() == id.as_str());
            let row = GtkBox::new(Orientation::Horizontal, 10);

            if let Some(quote) = quote {
                row.append(&identity_widget(quote, 32));

                let names = GtkBox::new(Orientation::Vertical, 0);
                let symbol = Label::new(Some(&quote.symbol.to_uppercase()));
                symbol.add_css_class("rowsym");
                symbol.set_xalign(0.0);
                names.append(&symbol);
                let name = Label::new(Some(&quote.name));
                name.add_css_class("muted");
                name.set_xalign(0.0);
                name.set_ellipsize(EllipsizeMode::End);
                names.append(&name);
                names.set_valign(Align::Center);
                names.set_size_request(96, -1);
                row.append(&names);

                let spark = chart(
                    &quote.sparkline,
                    quote.change_7d.unwrap_or(0.0) >= 0.0,
                    SPARK_WIDTH,
                    SPARK_HEIGHT,
                    false,
                    None,
                );
                spark.set_valign(Align::Center);
                row.append(&spark);

                let price = Label::new(Some(&format::price(quote.price, currency, decimals)));
                price.set_hexpand(true);
                price.set_xalign(1.0);
                row.append(&price);

                let change = Label::new(Some(&format::percent(quote.change_24h)));
                change.add_css_class("chip");
                change.add_css_class(trend_class(quote.change_24h));
                change.set_width_chars(8);
                row.append(&change);
            } else {
                let pending = Label::new(Some(id.as_str()));
                pending.add_css_class("muted");
                pending.set_hexpand(true);
                pending.set_xalign(0.0);
                row.append(&pending);
                let spinner = Label::new(Some("loading…"));
                spinner.add_css_class("muted");
                row.append(&spinner);
            }

            let remove = Button::with_label("✕");
            remove.add_css_class("ghost");
            remove.set_valign(Align::Center);
            remove.set_tooltip_text(Some("Stop watching"));
            let ctx = Rc::clone(self);
            let target = id.as_str().to_owned();
            remove.connect_clicked(move |_| {
                let written = ctx.app().editor().remove_coin(&target);
                ctx.adopt(written, &format!("{target} removed"));
            });
            row.append(&remove);

            let wrapper = gtk4::ListBoxRow::new();
            wrapper.set_child(Some(&row));
            wrapper.set_activatable(true);
            if index == active {
                wrapper.add_css_class("active");
            }
            list.append(&wrapper);
        }

        let ctx = Rc::clone(self);
        list.connect_row_activated(move |_, row| {
            let index = usize::try_from(row.index()).unwrap_or(0);
            let app = ctx.app();
            if let Err(error) = nav::select(&app, index) {
                ctx.note(&root_cause(&error), true);
                return;
            }
            ctx.note("", false);
            ctx.redraw();
        });

        self.widgets.list.append(&list);
    }
}

pub(super) fn header(quote: &Quote, currency: &Currency, decimals: format::Decimals) -> GtkBox {
    let container = GtkBox::new(Orientation::Horizontal, 12);
    container.append(&identity_widget(quote, 56));

    let texts = GtkBox::new(Orientation::Vertical, 2);

    let title = GtkBox::new(Orientation::Horizontal, 8);
    let symbol = Label::new(Some(&quote.symbol.to_uppercase()));
    symbol.add_css_class("symbol");
    title.append(&symbol);
    let name = Label::new(Some(&quote.name));
    name.add_css_class("name");
    name.set_valign(Align::Baseline);
    title.append(&name);
    title.set_halign(Align::Start);
    texts.append(&title);

    let price = Label::new(Some(&format::price(quote.price, currency, decimals)));
    price.add_css_class("price");
    price.set_halign(Align::Start);
    texts.append(&price);

    let changes = GtkBox::new(Orientation::Horizontal, 6);
    changes.set_margin_top(4);
    for (key, value) in [
        ("1h", quote.change_1h),
        ("24h", quote.change_24h),
        ("7d", quote.change_7d),
    ] {
        let cell = GtkBox::new(Orientation::Horizontal, 4);
        let key_label = Label::new(Some(key));
        key_label.add_css_class("chip-key");
        cell.append(&key_label);
        let chip = Label::new(Some(&format::percent(value)));
        chip.add_css_class("chip");
        chip.add_css_class(trend_class(value));
        cell.append(&chip);
        changes.append(&cell);
    }
    changes.set_halign(Align::Start);
    texts.append(&changes);

    container.append(&texts);
    container
}

pub(super) fn stats_card(
    quote: &Quote,
    currency: &Currency,
    decimals: format::Decimals,
) -> Option<GtkBox> {
    let mut rows: Vec<(String, String, &'static str)> = Vec::new();
    if let Some(rank) = quote.rank {
        rows.push(("Rank".to_owned(), format!("#{rank}"), "flat"));
    }
    if let Some(cap) = quote.market_cap {
        rows.push((
            "Market cap".to_owned(),
            format::compact(cap, currency),
            "flat",
        ));
    }
    if let Some(volume) = quote.volume_24h {
        rows.push((
            "24 h volume".to_owned(),
            format::compact(volume, currency),
            "flat",
        ));
    }
    if let Some(ath) = quote.ath.filter(|ath| *ath > 0.0) {
        let gap = (quote.price - ath) / ath * 100.0;
        let date = quote
            .ath_date
            .as_deref()
            .and_then(|date| date.get(..10))
            .map(|date| format!(" · {date}"))
            .unwrap_or_default();
        rows.push((
            "All-time high".to_owned(),
            format!(
                "{}  ({}){date}",
                format::price(ath, currency, decimals),
                format::percent(Some(gap))
            ),
            trend_class(Some(gap)),
        ));
    }
    if rows.is_empty() {
        return None;
    }

    let card = GtkBox::new(Orientation::Vertical, 8);
    card.add_css_class("card");
    let title = Label::new(Some("MARKET"));
    title.add_css_class("title");
    title.set_xalign(0.0);
    card.append(&title);
    for (key, value, class) in rows {
        let row = GtkBox::new(Orientation::Horizontal, 8);
        let name = Label::new(Some(&key));
        name.add_css_class("muted");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        row.append(&name);
        let figure = Label::new(Some(&value));
        figure.add_css_class(class);
        figure.set_xalign(1.0);
        row.append(&figure);
        card.append(&row);
    }
    Some(card)
}
