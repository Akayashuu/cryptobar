use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation};

use crate::config::{Config, MIN_REFRESH_INTERVAL};
use crate::model::Currency;
use crate::numeric::to_f64;
use crate::render::format;

use super::model::Ctx;

/// Settings controls, built once then never rebuilt: their values are those of
/// the file at opening time.
pub(super) struct Settings {
    pub(super) panel: GtkBox,
    pub(super) currency: gtk4::DropDown,
    pub(super) decimals: gtk4::DropDown,
    pub(super) refresh: gtk4::SpinButton,
    pub(super) cycle: gtk4::SpinButton,
    pub(super) sparkline: gtk4::Switch,
    pub(super) sparkline_width: gtk4::SpinButton,
    pub(super) signal: gtk4::SpinButton,
    pub(super) terminal: gtk4::Entry,
}

pub(super) fn settings_panel(config: &Config, config_path: &str) -> Settings {
    let grid = GtkBox::new(Orientation::Vertical, 10);
    grid.add_css_class("card");

    let currencies = Currency::all();
    let labels: Vec<String> = currencies
        .iter()
        .map(|c| format!("{} {}", c.as_str().to_uppercase(), c.symbol()))
        .collect();
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let currency = gtk4::DropDown::from_strings(&refs);
    let selected = currencies
        .iter()
        .position(|c| c.as_str() == config.currency.as_str())
        .unwrap_or(0);
    currency.set_selected(u32::try_from(selected).unwrap_or(0));

    let decimal_labels: Vec<String> = std::iter::once("auto".to_owned())
        .chain((0..=8).map(|n: u8| n.to_string()))
        .collect();
    let decimal_refs: Vec<&str> = decimal_labels.iter().map(String::as_str).collect();
    let decimals = gtk4::DropDown::from_strings(&decimal_refs);
    decimals.set_selected(match config.decimals() {
        format::Decimals::Fixed(n) => u32::from(n).saturating_add(1),
        format::Decimals::Auto => 0,
    });

    let refresh = spin(
        to_f64(MIN_REFRESH_INTERVAL),
        3600.0,
        5.0,
        to_f64(config.refresh_interval),
    );
    let cycle = spin(0.0, 3600.0, 5.0, to_f64(config.cycle_interval));
    let sparkline_width = spin(1.0, 64.0, 1.0, to_f64(config.sparkline_width));
    let signal = spin(1.0, 30.0, 1.0, f64::from(config.waybar_signal));

    let sparkline = gtk4::Switch::new();
    sparkline.set_active(config.sparkline);
    sparkline.set_valign(Align::Center);
    sparkline.set_halign(Align::End);

    let terminal = gtk4::Entry::new();
    terminal.set_text(&config.terminal);
    terminal.set_width_chars(14);

    let settings = Settings {
        panel: grid,
        currency,
        decimals,
        refresh,
        cycle,
        sparkline,
        sparkline_width,
        signal,
        terminal,
    };
    settings_groups(&settings);

    let path = Label::new(Some(config_path));
    path.add_css_class("muted");
    path.set_halign(Align::Start);
    path.set_selectable(true);
    path.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    settings.panel.append(&path);

    settings
}

fn settings_groups(settings: &Settings) {
    let Settings {
        panel: grid,
        currency,
        refresh,
        cycle,
        decimals,
        sparkline,
        sparkline_width,
        signal,
        terminal,
    } = settings;
    for (heading, rows) in [
        (
            "QUOTES",
            vec![
                (
                    "Currency",
                    "Currency the prices are displayed in",
                    currency.clone().upcast::<gtk4::Widget>(),
                ),
                (
                    "Refresh",
                    "Interval between two readings, in seconds",
                    refresh.clone().upcast(),
                ),
                (
                    "Automatic rotation",
                    "Seconds between two coins; 0 for the scroll wheel alone",
                    cycle.clone().upcast(),
                ),
            ],
        ),
        (
            "DISPLAY",
            vec![
                (
                    "Decimals",
                    "\"auto\" adapts to the order of magnitude of the price",
                    decimals.clone().upcast(),
                ),
                (
                    "Curve in the bar",
                    "Sparkline next to the price",
                    sparkline.clone().upcast(),
                ),
                (
                    "Curve width",
                    "Number of characters of the sparkline",
                    sparkline_width.clone().upcast(),
                ),
            ],
        ),
        (
            "SYSTEM",
            vec![
                (
                    "Waybar signal",
                    "SIGRTMIN+n used to wake the bar up",
                    signal.clone().upcast(),
                ),
                (
                    "Terminal",
                    "Opened by the right click; confirm with Enter",
                    terminal.clone().upcast(),
                ),
            ],
        ),
    ] {
        let heading_label = Label::new(Some(heading));
        heading_label.add_css_class("title");
        heading_label.set_xalign(0.0);
        heading_label.set_margin_top(4);
        grid.append(&heading_label);

        let group = GtkBox::new(Orientation::Vertical, 2);
        group.add_css_class("inset");
        for (label, hint, widget) in rows {
            group.append(&setting_row(label, hint, &widget));
        }
        grid.append(&group);
    }
}

fn setting_row(label: &str, hint: &str, control: &gtk4::Widget) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.set_margin_top(4);
    row.set_margin_bottom(4);

    let text = GtkBox::new(Orientation::Vertical, 0);
    text.set_hexpand(true);
    text.set_valign(Align::Center);
    let name = Label::new(Some(label));
    name.set_xalign(0.0);
    text.append(&name);
    let hint_label = Label::new(Some(hint));
    hint_label.add_css_class("muted");
    hint_label.set_xalign(0.0);
    hint_label.set_wrap(true);
    text.append(&hint_label);
    row.append(&text);

    control.set_halign(Align::End);
    control.set_valign(Align::Center);
    row.append(control);
    row
}

fn spin(min: f64, max: f64, step: f64, value: f64) -> gtk4::SpinButton {
    let spin = gtk4::SpinButton::with_range(min, max, step);
    spin.set_value(value);
    spin.set_numeric(true);
    spin.set_width_chars(5);
    spin
}

pub(super) fn wire_settings(ctx: &Rc<Ctx>, settings: &Settings) {
    let owner = Rc::clone(ctx);
    let currencies = Currency::all();
    settings.currency.connect_selected_notify(move |dropdown| {
        let index = usize::try_from(dropdown.selected()).unwrap_or(0);
        let Some(currency) = currencies.get(index) else {
            return;
        };
        if owner.app().config.currency.as_str() == currency.as_str() {
            return;
        }
        let written = owner.app().editor().set_currency(currency);
        owner.adopt(
            written,
            &format!("Currency: {}", currency.as_str().to_uppercase()),
        );
    });

    let decimals_owner = Rc::clone(ctx);
    settings.decimals.connect_selected_notify(move |dropdown| {
        let selected = dropdown.selected();
        let written = decimals_owner
            .app()
            .editor()
            .set_decimals(decimals_at(selected));
        decimals_owner.adopt(written, "Decimals saved");
    });

    wire_spin(ctx, &settings.refresh, "refresh_interval", "Interval saved");
    wire_spin(ctx, &settings.cycle, "cycle_interval", "Rotation saved");
    wire_spin(
        ctx,
        &settings.sparkline_width,
        "sparkline_width",
        "Width saved",
    );
    wire_spin(ctx, &settings.signal, "waybar_signal", "Signal saved");

    let sparkline_owner = Rc::clone(ctx);
    settings.sparkline.connect_state_set(move |_, active| {
        let written = sparkline_owner.app().editor().set_bool("sparkline", active);
        sparkline_owner.adopt(written, "Sparkline saved");
        glib::Propagation::Proceed
    });

    let terminal_owner = Rc::clone(ctx);
    settings.terminal.connect_activate(move |entry| {
        let value = entry.text().trim().to_owned();
        if value.is_empty() {
            terminal_owner.note("The terminal cannot be empty", true);
            return;
        }
        let written = terminal_owner.app().editor().set_string("terminal", &value);
        terminal_owner.adopt(written, "Terminal saved");
    });
}

fn decimals_at(selected: u32) -> Option<u8> {
    selected
        .checked_sub(1)
        .and_then(|places| u8::try_from(places).ok())
}

fn wire_spin(ctx: &Rc<Ctx>, spin: &gtk4::SpinButton, key: &'static str, message: &'static str) {
    let owner = Rc::clone(ctx);
    spin.connect_value_changed(move |spin| {
        let value = i64::from(spin.value_as_int());
        let written = owner.app().editor().set_integer(key, value);
        owner.adopt(written, message);
    });
}
