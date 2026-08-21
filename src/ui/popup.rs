//! GTK4 window: detailed prices, vector curve, search and settings.
//!
//! This is the configuration surface of the project. Everything `config.toml`
//! accepts is set here, and every change is written back to that same file —
//! comments preserved — by [`crate::config_edit`].
//!
//! The network never runs on the graphical thread: requests leave on a
//! dedicated thread and come back through a channel the GTK loop drains. The
//! window therefore stays smooth even when the API takes a second to answer.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox, Orientation,
};

use crate::app::App;
use crate::config::{Config, MIN_REFRESH_INTERVAL};
use crate::model::{CoinId, Currency, Quote, Range};
use crate::portfolio::{self, Portfolio};
use crate::render::braille::as_f64;
use crate::render::{badge, format};
use crate::source::SearchHit;

/// Application id, required by GTK.
const APP_ID: &str = "dev.akayashuu.cryptobar";

/// Delay left to typing before querying the API: below it, every letter would
/// fire a request.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(350);

/// Minimum length of a usable search term.
const SEARCH_MIN_LEN: usize = 2;

/// Number of results displayed under the search bar.
const SEARCH_LIMIT: usize = 8;

/// Drain rate of the network channel. Fine enough to be imperceptible, loose
/// enough to cost nothing.
const DRAIN: Duration = Duration::from_millis(120);

/// Base stylesheet.
///
/// Every shade is derived from `@theme_bg_color` and `@theme_fg_color` through
/// the GTK colour functions: the window matches the desktop theme, light or
/// dark, without freezing or reading it.
const CSS: &str = "
window { background-color: @theme_bg_color; }

.card {
  background-color: mix(@theme_bg_color, @theme_fg_color, 0.05);
  border-radius: 20px;
  padding: 18px;
}
.hero { padding: 20px 22px; }

/* Tabs: flat pills, in the same family as the chart range buttons, rather
   than the default tab bar. GTK paints its buttons with a `background-image`:
   neutralising it is necessary, `background-color` alone would not do. */
.tabs { background: none; padding: 0; }
.tabs button {
  background-image: none;
  background-color: transparent;
  border: none;
  box-shadow: none;
  border-radius: 999px;
  padding: 6px 14px;
  min-height: 0;
  font-weight: 600;
  opacity: 0.5;
}
.tabs button:hover {
  background-color: alpha(@theme_fg_color, 0.06);
  opacity: 0.8;
}
.tabs button:checked {
  background-color: alpha(@theme_fg_color, 0.12);
  opacity: 1;
}

.badge {
  border-radius: 999px;
  font-weight: 800;
  font-size: 26px;
  min-width: 56px;
  min-height: 56px;
  /* Neutral fallback: a coin with no known brand colour — a search result,
     say — stays readable instead of vanishing. */
  background-color: alpha(@theme_fg_color, 0.10);
  color: @theme_fg_color;
}
.badge.small { font-size: 15px; min-width: 32px; min-height: 32px; }

.title  { font-size: 13px; font-weight: 700; letter-spacing: 2px; opacity: 0.35; }
.symbol { font-size: 24px; font-weight: 800; }
.name   { font-size: 15px; opacity: 0.55; }
.price  { font-size: 46px; font-weight: 800; }
.rowsym { font-size: 16px; font-weight: 700; }

.chip {
  border-radius: 999px;
  padding: 3px 11px;
  font-size: 13px;
  font-weight: 600;
  background-color: alpha(@theme_fg_color, 0.08);
}
.chip.up   { color: #2fbf71; background-color: alpha(#2fbf71, 0.16); }
.chip.down { color: #e5484d; background-color: alpha(#e5484d, 0.16); }
.chip.flat { opacity: 0.55; }
.chip-key  { font-size: 12px; opacity: 0.45; font-weight: 600; }

.subtitle { font-size: 11px; font-weight: 700; letter-spacing: 1.5px; opacity: 0.3; }
.total  { font-size: 30px; font-weight: 800; }
.ring-count { font-size: 18px; font-weight: 800; opacity: 0.5; }
.dot    { font-size: 11px; }
.mono   { font-family: monospace; font-size: 12px; opacity: 0.45; }

/* Recessed block: separates the lists from the card background. */
.inset {
  background-color: alpha(@theme_fg_color, 0.04);
  border-radius: 14px;
  padding: 4px 10px;
}
.wrow { padding: 6px 2px; }

expander title { font-size: 13px; opacity: 0.6; }

.up   { color: #2fbf71; }
.down { color: #e5484d; }
.flat { opacity: 0.6; }
.muted { opacity: 0.5; font-size: 13px; }
.error { color: #e5484d; font-size: 13px; }

/* Time range pills. */
.range {
  background: none;
  border: none;
  box-shadow: none;
  border-radius: 999px;
  padding: 3px 13px;
  min-height: 26px;
  font-size: 13px;
  font-weight: 600;
  opacity: 0.5;
}
.range:hover { opacity: 0.85; background-color: alpha(@theme_fg_color, 0.06); }
.range.on {
  opacity: 1;
  background-color: alpha(@theme_fg_color, 0.12);
}

.ghost {
  background: none;
  border: none;
  box-shadow: none;
  padding: 0 8px;
  min-height: 26px;
  min-width: 26px;
  opacity: 0.3;
}
.ghost:hover { opacity: 1; color: #e5484d; }

.add { padding: 2px 12px; min-height: 28px; border-radius: 999px; }

entry { border-radius: 14px; min-height: 38px; }

list { background-color: transparent; }
row { padding: 9px 12px; border-radius: 14px; }
row.active { background-color: alpha(@theme_fg_color, 0.08); }
row:hover { background-color: alpha(@theme_fg_color, 0.05); }
";

/// Geometry of the plot inside the drawing area.
const CHART_WIDTH: i32 = 540;
const CHART_HEIGHT: i32 = 230;

/// Thumbnail drawn on every row of the list.
const SPARK_WIDTH: i32 = 72;
const SPARK_HEIGHT: i32 = 26;

/// Number of horizontal guide lines under the curve.
const GRID_LINES: i32 = 4;

/// Message sent from a worker thread up to the graphical loop.
enum Msg {
    /// Results of a search, tagged by the generation that asked for them.
    Search(u64, Result<Vec<SearchHit>, String>),
    /// Freshly loaded quotes, with the "served from cache" flag.
    Quotes(Vec<Quote>, bool),
    /// Series of a range other than 7 days, keyed by its cache key.
    Series(String, Vec<f64>),
    /// Freshly read balances from the watched chains.
    Balances(Vec<crate::onchain::Balance>),
}

/// Opens the window and returns when it closes.
///
/// # Errors
/// Fails when GTK cannot initialise — typically outside a graphical session.
pub fn show(app: App, quotes: Vec<Quote>, stale: bool) -> anyhow::Result<()> {
    let gtk_app = Application::builder().application_id(APP_ID).build();
    let seed = Rc::new(RefCell::new(Some((Arc::new(app), quotes, stale))));

    gtk_app.connect_startup(|_| {
        let provider = gtk4::CssProvider::new();
        // `load_from_data` works on every GTK 4 version, unlike
        // `load_from_string` which would require GTK 4.12.
        provider.load_from_data(CSS);
        if let Some(display) = gtk4::gdk::Display::default() {
            // Above `~/.config/gtk-4.0/gtk.css`, loaded by GTK at the "user"
            // priority: a desktop theme repainting every button must not decide
            // how this window looks.
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    });

    gtk_app.connect_activate(move |gtk_app| {
        // GTK activates only once per process; a second activation (single
        // instance) simply presents the existing window.
        if let Some((app, quotes, stale)) = seed.borrow_mut().take() {
            build_window(gtk_app, app, quotes, stale);
        } else if let Some(window) = gtk_app.windows().first() {
            window.present();
        }
    });

    // GTK would otherwise parse our own command line arguments.
    let empty: [&str; 0] = [];
    let status = gtk_app.run_with_args(&empty);
    if status == glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        anyhow::bail!("the window exited with an error")
    }
}

/// Live data of the window.
struct Model {
    app: Arc<App>,
    quotes: Vec<Quote>,
    stale: bool,
    hits: Vec<SearchHit>,
    /// Range displayed under the curve.
    range: Range,
    /// Series already loaded, per (coin, range) pair. The 7 day range comes
    /// from the quotes and never lands here.
    series: HashMap<String, Vec<f64>>,
    /// Balances read from the watched addresses, summed per cryptocurrency.
    onchain: std::collections::BTreeMap<CoinId, f64>,
    /// Per-address detail, for the list of watched wallets.
    balances: Vec<crate::onchain::Balance>,
    /// Number of the last search fired: late answers, whose term the user has
    /// already changed, are dropped.
    generation: u64,
}

/// Containers rebuilt on every render.
#[derive(Clone)]
struct Widgets {
    search: gtk4::SearchEntry,
    results: GtkBox,
    detail: GtkBox,
    list: GtkBox,
    wallet: GtkBox,
    alerts: GtkBox,
    status: Label,
}

struct Ctx {
    model: RefCell<Model>,
    /// Provider reloaded on every render: it carries one class per coin, with
    /// its brand colour. Generating the sheet avoids the deprecated
    /// `StyleContext::add_provider` API.
    palette: gtk4::CssProvider,
    widgets: Widgets,
    tx: Sender<Msg>,
    /// Debounce timer of the search, cancelled on every keystroke.
    debounce: RefCell<Option<glib::SourceId>>,
}

impl Ctx {
    fn app(&self) -> Arc<App> {
        Arc::clone(&self.model.borrow().app)
    }

    /// Shows a transient message under the settings.
    fn note(&self, message: &str, error: bool) {
        self.widgets.status.set_text(message);
        self.widgets.status.remove_css_class("error");
        self.widgets.status.remove_css_class("muted");
        self.widgets
            .status
            .add_css_class(if error { "error" } else { "muted" });
    }

    /// Reloads the quotes without blocking the interface.
    fn reload_quotes(&self) {
        let app = self.app();
        let tx = self.tx.clone();
        // `markets_cached` serves the cache as long as it is fresh: this
        // thread touches the network only when it is truly needed.
        std::thread::spawn(move || {
            let (mut quotes, stale) = match app
                .source
                .markets_cached(&app.config.coins, &app.config.currency)
            {
                Ok(cached) => (cached.value, cached.stale),
                Err(_) => (Vec::new(), true),
            };
            app.fetch_icons(&mut quotes);
            let _ = tx.send(Msg::Quotes(quotes, stale));
        });

        // The chains are queried separately: a slow address must not delay the
        // display of the prices.
        let app = self.app();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let balances = app.watcher().refresh(&app.config.wallets);
            let _ = tx.send(Msg::Balances(balances));
        });
    }

    /// Takes a rewritten configuration into account: new context, reloaded
    /// data, bar woken up.
    fn adopt(self: &Rc<Self>, written: anyhow::Result<Config>, message: &str) {
        match written {
            Ok(_) => {
                let reloaded = self.model.borrow().app.reload();
                match reloaded {
                    Ok(app) => {
                        let app = Arc::new(app);
                        app.notify();
                        self.model.borrow_mut().app = app;
                        self.note(message, false);
                        self.reload_quotes();
                        self.redraw();
                    }
                    Err(e) => self.note(&format!("{e}"), true),
                }
            }
            Err(e) => self.note(&root_cause(&e), true),
        }
    }

    fn redraw(self: &Rc<Self>) {
        let model = self.model.borrow();
        self.palette.load_from_data(&badge_css(&model.quotes));
        let active = model.app.active_index();
        let currency = model.app.config.currency.clone();
        let decimals = model.app.config.decimals();
        let range = model.range;
        let selected = model
            .quotes
            .get(active)
            .or_else(|| model.quotes.first())
            .cloned();
        let stale = model.stale && !model.quotes.is_empty();
        let series = selected
            .as_ref()
            .map(|quote| series_of(&model, quote, range))
            .unwrap_or_default();
        drop(model);

        clear(&self.widgets.detail);
        if let Some(quote) = selected {
            self.widgets
                .detail
                .append(&header(&quote, &currency, decimals));
            self.widgets
                .detail
                .append(&self.chart_card(&quote, &series, range, &currency, decimals));
            if let Some(stats) = stats_card(&quote, &currency, decimals) {
                self.widgets.detail.append(&stats);
            }
            self.request_series(&quote, range);
        } else {
            let empty = Label::new(Some("No data — check the network connection"));
            empty.add_css_class("muted");
            empty.set_halign(Align::Start);
            self.widgets.detail.append(&empty);
        }
        if stale {
            let warning = Label::new(Some("Cached data — network unavailable"));
            warning.add_css_class("muted");
            warning.set_halign(Align::Start);
            self.widgets.detail.append(&warning);
        }

        self.draw_list(active, &currency, decimals);
        self.draw_wallet(&currency, decimals);
        self.draw_alerts(&currency, decimals);
        self.draw_results();
    }

    /// Changes the range displayed under the curve.
    fn set_range(self: &Rc<Self>, range: Range) {
        if self.model.borrow().range == range {
            return;
        }
        self.model.borrow_mut().range = range;
        self.redraw();
    }

    /// Loads in the background the series of a still unknown range. The 7 day
    /// range is already in the quotes: it never costs a request.
    fn request_series(self: &Rc<Self>, quote: &Quote, range: Range) {
        if range == Range::D7 {
            return;
        }
        let key = series_key(quote.id.as_str(), range);
        if self.model.borrow().series.contains_key(&key) {
            return;
        }
        let app = self.app();
        let tx = self.tx.clone();
        let id = quote.id.clone();
        std::thread::spawn(move || {
            if let Ok(cached) = app.source.series_cached(&id, range, &app.config.currency) {
                let _ = tx.send(Msg::Series(key, cached.value.points));
            }
        });
    }

    /// Chart card: the curve, its range selector and its bounds.
    fn chart_card(
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

        // The colour follows the change observed over the displayed range, not
        // the 24 h change alone: otherwise a falling curve could be green.
        let positive = match (series.first(), series.last()) {
            (Some(first), Some(last)) => last >= first,
            _ => quote.change_7d.unwrap_or(0.0) >= 0.0,
        };
        let finite: Vec<f64> = series.iter().copied().filter(|v| v.is_finite()).collect();
        let hover: Hover = Rc::new(Cell::new(None));
        let area = chart(
            series,
            positive,
            CHART_WIDTH,
            CHART_HEIGHT,
            true,
            Some(Rc::clone(&hover)),
        );

        // Hover readout: the price of the pointed sample and its age. The line
        // keeps its place even when empty, otherwise the card would jump.
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
            let (low, high) = crate::render::braille::bounds(&finite);
            let span = Label::new(Some(&format!(
                "{} — {}",
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

    /// Export buttons of the displayed curve.
    fn export_buttons(
        self: &Rc<Self>,
        footer: &GtkBox,
        points: &[f64],
        positive: bool,
        quote: &Quote,
        range: Range,
    ) {
        for (label, tooltip, png) in [
            ("PNG", "Export the curve as an image", true),
            ("CSV", "Export the points as a spreadsheet", false),
        ] {
            let button = Button::with_label(label);
            button.add_css_class("range");
            button.set_tooltip_text(Some(tooltip));
            let ctx = Rc::clone(self);
            let points = points.to_vec();
            let symbol = quote.symbol.to_uppercase();
            button.connect_clicked(move |_| {
                let outcome = if png {
                    export_png(&points, positive, &symbol, range)
                } else {
                    export_csv(&points, &symbol, range)
                };
                match outcome {
                    Ok(path) => ctx.note(&format!("exported to {}", path.display()), false),
                    Err(e) => ctx.note(&root_cause(&e), true),
                }
            });
            footer.append(&button);
        }
    }

    fn draw_list(self: &Rc<Self>, active: usize, currency: &Currency, decimals: format::Decimals) {
        clear(&self.widgets.list);
        let model = self.model.borrow();
        let ids = model.app.config.coins.clone();
        let quotes = model.quotes.clone();
        drop(model);

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("card");

        for (index, id) in ids.iter().enumerate() {
            let quote = quotes.iter().find(|q| q.id.as_str() == id.as_str());
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
                name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                names.append(&name);
                names.set_valign(Align::Center);
                names.set_size_request(96, -1);
                row.append(&names);

                // Thumbnail: the shape of the last seven days reads at a
                // glance, without opening the detail of every coin.
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
                // Coin just added: the row exists before its quote.
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
            if let Err(e) = crate::commands::nav::select(&app, index) {
                ctx.note(&root_cause(&e), true);
                return;
            }
            ctx.note("", false);
            ctx.redraw();
        });

        self.widgets.list.append(&list);
    }

    /// Portfolio card: total, split, positions and watched addresses.
    fn draw_wallet(self: &Rc<Self>, currency: &Currency, decimals: format::Decimals) {
        clear(&self.widgets.wallet);
        let model = self.model.borrow();
        let wallets = model.app.config.wallets.clone();
        let balances = model.balances.clone();
        let wallet = portfolio::compute(&model.quotes, &model.onchain);
        let quotes = model.quotes.clone();
        drop(model);

        let card = GtkBox::new(Orientation::Vertical, 14);
        card.add_css_class("card");

        let head = GtkBox::new(Orientation::Horizontal, 8);
        let title = Label::new(Some("PORTFOLIO"));
        title.add_css_class("title");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        head.append(&title);
        if let Some(pct) = wallet.as_ref().and_then(|wallet| wallet.change_24h_pct) {
            let chip = Label::new(Some(&format::percent(Some(pct))));
            chip.add_css_class("chip");
            chip.add_css_class(trend_class(Some(pct)));
            head.append(&chip);
        }
        card.append(&head);

        card.append(&wallet_hero(
            wallet.as_ref(),
            wallets.len(),
            currency,
            decimals,
        ));

        if let Some(wallet) = wallet.as_ref() {
            let positions = GtkBox::new(Orientation::Vertical, 0);
            positions.add_css_class("inset");
            for position in &wallet.positions {
                positions.append(&position_row(position, currency, decimals));
            }
            card.append(&positions);
        }

        card.append(&self.wallets_section(&wallets, &balances, &quotes, currency, decimals));
        self.widgets.wallet.append(&card);
    }

    /// Watched addresses section: one row per address, and the form to add
    /// another one.
    fn wallets_section(
        self: &Rc<Self>,
        wallets: &[crate::onchain::Wallet],
        balances: &[crate::onchain::Balance],
        quotes: &[Quote],
        currency: &Currency,
        decimals: format::Decimals,
    ) -> GtkBox {
        let section = GtkBox::new(Orientation::Vertical, 8);

        let title = Label::new(Some("WATCHED ADDRESSES"));
        title.add_css_class("subtitle");
        title.set_xalign(0.0);
        section.append(&title);

        // Without this sentence, "public address" gets confused with "key". So
        // we say where to find it, and what cryptobar refuses to accept.
        let help = Label::new(Some(
            "Paste the public address of your wallet — the one you hand out to receive \
             funds (Ledger Live: \"Receive\"). cryptobar reads the balance from the chain, \
             and can never spend. A private key or a recovery phrase is rejected and never \
             written to disk.",
        ));
        help.add_css_class("muted");
        help.set_wrap(true);
        help.set_xalign(0.0);
        section.append(&help);

        for (index, wallet) in wallets.iter().enumerate() {
            let row = GtkBox::new(Orientation::Horizontal, 10);
            row.add_css_class("wrow");

            let dot = Label::new(Some("●"));
            dot.add_css_class("dot");
            dot.add_css_class(&format!("dot-{}", css_key(wallet.chain.symbol())));
            row.append(&dot);

            let names = GtkBox::new(Orientation::Vertical, 0);
            let label = Label::new(Some(&wallet.title()));
            label.set_xalign(0.0);
            names.append(&label);
            let address = Label::new(Some(&crate::onchain::shorten(&wallet.address)));
            address.add_css_class("mono");
            address.set_xalign(0.0);
            names.append(&address);
            names.set_hexpand(true);
            row.append(&names);

            let balance = balances
                .iter()
                .find(|balance| balance.address == wallet.address);
            let amounts = GtkBox::new(Orientation::Vertical, 0);
            let quantity = Label::new(Some(&match balance {
                Some(balance) => format!(
                    "{} {}",
                    format::quantity(balance.amount),
                    wallet.chain.symbol()
                ),
                None => "reading…".to_owned(),
            }));
            quantity.set_xalign(1.0);
            amounts.append(&quantity);
            let value = quotes
                .iter()
                .find(|quote| quote.id == wallet.chain.coin())
                .zip(balance)
                .map(|(quote, balance)| quote.price * balance.amount);
            let money = Label::new(Some(&match value {
                Some(value) => format::price(value, currency, decimals),
                None => "—".to_owned(),
            }));
            money.add_css_class("muted");
            money.set_xalign(1.0);
            amounts.append(&money);
            row.append(&amounts);

            let remove = Button::with_label("✕");
            remove.add_css_class("ghost");
            remove.set_valign(Align::Center);
            remove.set_tooltip_text(Some("Stop watching this address"));
            let ctx = Rc::clone(self);
            remove.connect_clicked(move |_| {
                let written = ctx.app().editor().remove_wallet(index);
                ctx.adopt(written, "address removed");
            });
            row.append(&remove);
            section.append(&row);
        }

        section.append(&self.wallet_form());
        section
    }

    /// Sample address for a chain, shown as a placeholder in the field.
    fn address_placeholder(chain: crate::onchain::Chain) -> &'static str {
        match chain {
            crate::onchain::Chain::Bitcoin => "bc1… or 1… or 3…",
            crate::onchain::Chain::Ethereum => "0x…",
            crate::onchain::Chain::Solana => "Solana address, e.g. 9xQe…",
        }
    }

    /// Form to add a watched address.
    fn wallet_form(self: &Rc<Self>) -> GtkBox {
        let form = GtkBox::new(Orientation::Horizontal, 8);

        let labels: Vec<&str> = crate::onchain::Chain::ALL
            .iter()
            .map(|chain| chain.symbol())
            .collect();
        let picker = gtk4::DropDown::from_strings(&labels);
        picker.set_valign(Align::Center);
        form.append(&picker);

        let address = gtk4::Entry::new();
        address.set_placeholder_text(Some(Self::address_placeholder(
            crate::onchain::Chain::Bitcoin,
        )));
        address.set_hexpand(true);
        form.append(&address);

        // The placeholder follows the chosen chain: an Ethereum address looks
        // nothing like a Bitcoin one.
        let hinted = address.clone();
        picker.connect_selected_notify(move |picker| {
            let index = usize::try_from(picker.selected()).unwrap_or(0);
            if let Some(chain) = crate::onchain::Chain::ALL.get(index).copied() {
                hinted.set_placeholder_text(Some(Self::address_placeholder(chain)));
            }
        });

        let label = gtk4::Entry::new();
        label.set_placeholder_text(Some("name, e.g. Ledger"));
        label.set_width_chars(10);
        form.append(&label);

        let add = Button::with_label("Watch");
        add.add_css_class("add");
        add.set_valign(Align::Center);
        let ctx = Rc::clone(self);
        let fields = (address.clone(), label.clone());
        add.connect_clicked(move |_| {
            let index = usize::try_from(picker.selected()).unwrap_or(0);
            let Some(chain) = crate::onchain::Chain::ALL.get(index).copied() else {
                return;
            };
            let (address, label) = &fields;
            let text = address.text().to_string();
            let name = label.text().to_string();
            let written = ctx.app().editor().add_wallet(
                chain,
                &text,
                Some(name.as_str()).filter(|name| !name.trim().is_empty()),
            );
            if written.is_ok() {
                address.set_text("");
                label.set_text("");
            }
            ctx.adopt(written, "address watched — reading the balance…");
        });
        form.append(&add);
        form
    }

    /// Alerts card: those already set, and what it takes to add one.
    fn draw_alerts(self: &Rc<Self>, currency: &Currency, decimals: format::Decimals) {
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

    /// Form to add an alert.
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

    fn draw_results(self: &Rc<Self>) {
        clear(&self.widgets.results);
        let model = self.model.borrow();
        let hits: Vec<SearchHit> = model.hits.iter().take(SEARCH_LIMIT).cloned().collect();
        let known: Vec<String> = model
            .app
            .config
            .coins
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        drop(model);

        if hits.is_empty() {
            self.widgets.results.set_visible(false);
            return;
        }
        self.widgets.results.set_visible(true);

        let list = ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.add_css_class("card");
        for hit in hits {
            let row = GtkBox::new(Orientation::Horizontal, 10);
            row.append(&identity_of(hit.icon.as_deref(), &hit.symbol, 22));
            let symbol = Label::new(Some(&hit.symbol.to_uppercase()));
            symbol.add_css_class("rowsym");
            symbol.set_width_chars(8);
            symbol.set_xalign(0.0);
            row.append(&symbol);

            let name = Label::new(Some(&hit.name));
            name.set_hexpand(true);
            name.set_xalign(0.0);
            name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            row.append(&name);

            if known.iter().any(|id| id == hit.id.as_str()) {
                let already = Label::new(Some("watched"));
                already.add_css_class("muted");
                row.append(&already);
            } else {
                let add = Button::with_label("Ajouter");
                add.add_css_class("add");
                add.set_valign(Align::Center);
                let ctx = Rc::clone(self);
                let id = hit.id.clone();
                add.connect_clicked(move |_| ctx.add_coin(&id));
                row.append(&add);
            }

            let wrapper = gtk4::ListBoxRow::new();
            wrapper.set_child(Some(&row));
            wrapper.set_activatable(false);
            list.append(&wrapper);
        }
        self.widgets.results.append(&list);
    }

    fn add_coin(self: &Rc<Self>, id: &CoinId) {
        let written = self.app().editor().add_coin(id);
        let ok = written.is_ok();
        self.adopt(written, &format!("{id} added"));
        if ok {
            self.widgets.search.set_text("");
            self.model.borrow_mut().hits.clear();
            self.draw_results();
        }
    }

    /// Fires a network search on a dedicated thread.
    fn search(&self, term: String) {
        let generation = {
            let mut model = self.model.borrow_mut();
            model.generation = model.generation.saturating_add(1);
            model.generation
        };
        let app = self.app();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let outcome = app.source.search(&term).map_err(|e| e.to_string()).map(
                |mut hits: Vec<SearchHit>| {
                    // The logos are fetched here, on the search thread: only
                    // those actually displayed are downloaded, and the
                    // interface never blocks on them.
                    let mut agent = None;
                    for hit in hits.iter_mut().take(SEARCH_LIMIT) {
                        hit.icon = app.icon_for(hit.id.as_str(), hit.image.as_deref(), &mut agent);
                    }
                    hits
                },
            );
            let _ = tx.send(Msg::Search(generation, outcome));
        });
    }
}

/// Cache key of a series inside the model.
fn series_key(id: &str, range: Range) -> String {
    format!("{id}-{}", range.slug())
}

/// Points to plot for a range: the quotes already cover 7 days, the other
/// ranges come from the series cache.
fn series_of(model: &Model, quote: &Quote, range: Range) -> Vec<f64> {
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

fn clear(container: &GtkBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Most concrete message of the error chain: that is the helpful one.
fn root_cause(error: &anyhow::Error) -> String {
    error
        .chain()
        .last()
        .map_or_else(|| error.to_string(), ToString::to_string)
}

fn build_window(gtk_app: &Application, app: Arc<App>, quotes: Vec<Quote>, stale: bool) {
    let (tx, rx) = channel::<Msg>();

    let widgets = Widgets {
        search: gtk4::SearchEntry::new(),
        results: GtkBox::new(Orientation::Vertical, 0),
        detail: GtkBox::new(Orientation::Vertical, 14),
        list: GtkBox::new(Orientation::Vertical, 0),
        wallet: GtkBox::new(Orientation::Vertical, 0),
        alerts: GtkBox::new(Orientation::Vertical, 0),
        status: Label::new(None),
    };
    widgets
        .search
        .set_placeholder_text(Some("Search a cryptocurrency to watch…"));
    widgets.results.set_visible(false);
    widgets.status.set_halign(Align::Start);
    widgets.status.add_css_class("muted");

    let config_path = app.config_path().display().to_string();
    let settings = settings_panel(&app.config, &config_path);

    let palette = gtk4::CssProvider::new();
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &palette,
            gtk4::STYLE_PROVIDER_PRIORITY_USER.saturating_add(1),
        );
    }

    let ctx = Rc::new(Ctx {
        palette,
        model: RefCell::new(Model {
            app,
            quotes,
            stale,
            hits: Vec::new(),
            range: Range::D7,
            series: HashMap::new(),
            onchain: std::collections::BTreeMap::new(),
            balances: Vec::new(),
            generation: 0,
        }),
        widgets: widgets.clone(),
        tx,
        debounce: RefCell::new(None),
    });
    wire_search(&ctx);
    wire_settings(&ctx, &settings);
    drain(&ctx, rx);

    // Four pages rather than one endless column: the portfolio and the
    // settings used to sit below the fold, and nothing hinted that one had to
    // scroll to reach them.
    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.set_vexpand(true);
    stack.add_titled(
        &page(&[&widgets.detail, &widgets.list]),
        Some("prices"),
        "Prices",
    );
    stack.add_titled(&page(&[&widgets.wallet]), Some("portfolio"), "Portfolio");
    stack.add_titled(&page(&[&widgets.alerts]), Some("alerts"), "Alerts");
    stack.add_titled(&page(&[&settings.panel]), Some("settings"), "Settings");

    let tabs = gtk4::StackSwitcher::new();
    tabs.set_stack(Some(&stack));
    tabs.set_halign(Align::Fill);
    tabs.add_css_class("tabs");

    let content = GtkBox::new(Orientation::Vertical, 14);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.append(&title_bar(&ctx));
    content.append(&widgets.search);
    content.append(&widgets.results);
    content.append(&tabs);
    content.append(&stack);
    content.append(&widgets.status);

    let window = ApplicationWindow::builder()
        .application(gtk_app)
        .title("cryptobar")
        .default_width(620)
        .default_height(920)
        .resizable(true)
        .child(&content)
        .build();

    // Escape closes the window: that is the expected reflex for a popup. The
    // search bar consumes the key first to clear its field.
    let controller = gtk4::EventControllerKey::new();
    let to_close = window.clone();
    controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            to_close.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(controller);

    ctx.redraw();
    ctx.reload_quotes();
    window.present();
    // The window opens ready to take a term: that is the most frequent gesture
    // after "I want to watch something else".
    widgets.search.grab_focus();
}

/// Drains the worker channel at a fixed rate.
fn drain(ctx: &Rc<Ctx>, rx: Receiver<Msg>) {
    let ctx = Rc::clone(ctx);
    glib::timeout_add_local(DRAIN, move || {
        loop {
            match rx.try_recv() {
                Ok(Msg::Quotes(quotes, stale)) => {
                    {
                        let mut model = ctx.model.borrow_mut();
                        model.quotes = quotes;
                        model.stale = stale;
                    }
                    ctx.redraw();
                }
                Ok(Msg::Balances(balances)) => {
                    {
                        let mut model = ctx.model.borrow_mut();
                        model.onchain = crate::onchain::totals(&balances);
                        model.balances = balances;
                    }
                    ctx.redraw();
                }
                Ok(Msg::Series(key, points)) => {
                    ctx.model.borrow_mut().series.insert(key, points);
                    ctx.redraw();
                }
                Ok(Msg::Search(generation, outcome)) => {
                    if ctx.model.borrow().generation != generation {
                        continue; // Stale answer: the user has already retyped.
                    }
                    match outcome {
                        Ok(hits) => {
                            if hits.is_empty() {
                                ctx.note("No result", false);
                            } else {
                                ctx.note("", false);
                            }
                            ctx.model.borrow_mut().hits = hits;
                        }
                        Err(message) => {
                            ctx.note(&message, true);
                            ctx.model.borrow_mut().hits.clear();
                        }
                    }
                    ctx.draw_results();
                }
                Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }
    });
}

/// One tab page: its sections stacked inside a scrollable area, so that long
/// content never pushes the tabs out of the window.
fn page(sections: &[&GtkBox]) -> gtk4::ScrolledWindow {
    let column = GtkBox::new(Orientation::Vertical, 14);
    for section in sections {
        column.append(*section);
    }
    let scroller = gtk4::ScrolledWindow::new();
    scroller.set_hscrollbar_policy(gtk4::PolicyType::Never);
    scroller.set_child(Some(&column));
    scroller.set_vexpand(true);
    scroller
}

fn title_bar(ctx: &Rc<Ctx>) -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 8);
    let title = Label::new(Some("CRYPTOBAR"));
    title.add_css_class("title");
    title.set_hexpand(true);
    title.set_xalign(0.0);
    bar.append(&title);

    let refresh = Button::with_label("Refresh");
    refresh.add_css_class("range");
    let owner = Rc::clone(ctx);
    refresh.connect_clicked(move |_| {
        let app = owner.app();
        if let Err(e) = crate::commands::nav::refresh(&app) {
            owner.note(&root_cause(&e), true);
            return;
        }
        owner.model.borrow_mut().series.clear();
        owner.note("Refreshing…", false);
        owner.reload_quotes();
    });
    bar.append(&refresh);
    bar
}

fn wire_search(ctx: &Rc<Ctx>) {
    let owner = Rc::clone(ctx);
    ctx.widgets.search.connect_search_changed(move |entry| {
        if let Some(source) = owner.debounce.borrow_mut().take() {
            source.remove();
        }
        let term = entry.text().trim().to_owned();
        if term.chars().count() < SEARCH_MIN_LEN {
            owner.model.borrow_mut().hits.clear();
            owner.note("", false);
            owner.draw_results();
            return;
        }
        let inner = Rc::clone(&owner);
        let source = glib::timeout_add_local_once(SEARCH_DEBOUNCE, move || {
            inner.debounce.borrow_mut().take();
            inner.search(term);
        });
        *owner.debounce.borrow_mut() = Some(source);
    });
}

/// Settings controls, built once then never rebuilt: their values are those of
/// the file at opening time.
struct Settings {
    panel: GtkBox,
    currency: gtk4::DropDown,
    decimals: gtk4::DropDown,
    refresh: gtk4::SpinButton,
    cycle: gtk4::SpinButton,
    sparkline: gtk4::Switch,
    sparkline_width: gtk4::SpinButton,
    signal: gtk4::SpinButton,
    terminal: gtk4::Entry,
}

fn settings_panel(config: &Config, config_path: &str) -> Settings {
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

    // Index 0 = "auto", then 0 to 8 decimals.
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
        as_f64_u64(MIN_REFRESH_INTERVAL),
        3600.0,
        5.0,
        as_f64_u64(config.refresh_interval),
    );
    let cycle = spin(0.0, 3600.0, 5.0, as_f64_u64(config.cycle_interval));
    let sparkline_width = spin(1.0, 64.0, 1.0, as_f64(config.sparkline_width));
    let signal = spin(1.0, 30.0, 1.0, f64::from(config.waybar_signal));

    let sparkline = gtk4::Switch::new();
    sparkline.set_active(config.sparkline);
    sparkline.set_valign(Align::Center);
    sparkline.set_halign(Align::End);

    let terminal = gtk4::Entry::new();
    terminal.set_text(&config.terminal);
    terminal.set_width_chars(14);

    settings_groups(
        &grid,
        &currency,
        &refresh,
        &cycle,
        &decimals,
        &sparkline,
        &sparkline_width,
        &signal,
        &terminal,
    );

    let path = Label::new(Some(config_path));
    path.add_css_class("muted");
    path.set_halign(Align::Start);
    path.set_selectable(true);
    path.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    grid.append(&path);

    Settings {
        panel: grid,
        currency,
        decimals,
        refresh,
        cycle,
        sparkline,
        sparkline_width,
        signal,
        terminal,
    }
}

/// Fills the settings panel in, group by group.
#[allow(clippy::too_many_arguments)]
fn settings_groups(
    grid: &GtkBox,
    currency: &gtk4::DropDown,
    refresh: &gtk4::SpinButton,
    cycle: &gtk4::SpinButton,
    decimals: &gtk4::DropDown,
    sparkline: &gtk4::Switch,
    sparkline_width: &gtk4::SpinButton,
    signal: &gtk4::SpinButton,
    terminal: &gtk4::Entry,
) {
    // Three groups rather than a list of eight rows: a setting is looked up by
    // what it affects, not by its rank in the configuration.
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

/// One settings row: label, one-line explanation, and the control aligned to
/// the right. The explanation saves guessing what a bare number like "8" does.
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

/// Lossless conversion for the ranges handled here (seconds, widths).
fn as_f64_u64(value: u64) -> f64 {
    as_f64(usize::try_from(value).unwrap_or(usize::MAX))
}

fn wire_settings(ctx: &Rc<Ctx>, settings: &Settings) {
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

    let owner = Rc::clone(ctx);
    settings.decimals.connect_selected_notify(move |dropdown| {
        let selected = dropdown.selected();
        let decimals = if selected == 0 {
            None
        } else {
            u8::try_from(selected.saturating_sub(1)).ok()
        };
        let written = owner.app().editor().set_decimals(decimals);
        owner.adopt(written, "Decimals saved");
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

    let owner = Rc::clone(ctx);
    settings.sparkline.connect_state_set(move |_, active| {
        let written = owner.app().editor().set_bool("sparkline", active);
        owner.adopt(written, "Sparkline saved");
        glib::Propagation::Proceed
    });

    let owner = Rc::clone(ctx);
    settings.terminal.connect_activate(move |entry| {
        let value = entry.text().trim().to_owned();
        if value.is_empty() {
            owner.note("The terminal cannot be empty", true);
            return;
        }
        let written = owner.app().editor().set_string("terminal", &value);
        owner.adopt(written, "Terminal saved");
    });
}

fn wire_spin(ctx: &Rc<Ctx>, spin: &gtk4::SpinButton, key: &'static str, message: &'static str) {
    let owner = Rc::clone(ctx);
    spin.connect_value_changed(move |spin| {
        let value = i64::from(spin.value_as_int());
        let written = owner.app().editor().set_integer(key, value);
        owner.adopt(written, message);
    });
}

/// Visual identity of a cryptocurrency: its official logo when the cache holds
/// it, otherwise a coloured badge carrying its glyph.
fn identity_widget(quote: &Quote, size: i32) -> gtk4::Widget {
    identity_of(quote.icon.as_deref(), &quote.symbol, size)
}

/// Same thing from the raw fields, for what is not a quote — a search result,
/// for instance.
fn identity_of(icon: Option<&str>, symbol: &str, size: i32) -> gtk4::Widget {
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
fn badge_css(quotes: &[Quote]) -> String {
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
fn css_key(symbol: &str) -> String {
    symbol
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn header(quote: &Quote, currency: &Currency, decimals: format::Decimals) -> GtkBox {
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
        let name = Label::new(Some(key));
        name.add_css_class("chip-key");
        cell.append(&name);
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

fn trend_class(value: Option<f64>) -> &'static str {
    match value {
        Some(v) if v > 0.0 => "up",
        Some(v) if v < 0.0 => "down",
        _ => "flat",
    }
}

/// Hovered abscissa, within `0.0..=1.0`, shared between the mouse controller
/// and the drawing function. `None` when the pointer is elsewhere.
type Hover = Rc<Cell<Option<f64>>>;

fn chart(
    points: &[f64],
    positive: bool,
    width: i32,
    height: i32,
    detailed: bool,
    hover: Option<Hover>,
) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::new();
    area.set_content_width(width);
    area.set_content_height(height);
    let points = points.to_vec();

    area.set_draw_func(move |_, ctx, width, height| {
        draw_series(
            ctx,
            &points,
            positive,
            f64::from(width),
            f64::from(height),
            detailed,
            hover.as_ref().and_then(|hover| hover.get()),
        );
    });
    area
}

/// Draws the curve into any Cairo context.
///
/// The function does not depend on the widget: it is the one drawing on screen
/// and filling the image surface of the PNG export, which guarantees the
/// exported file is exactly what one sees.
#[allow(clippy::too_many_arguments)]
fn draw_series(
    ctx: &gtk4::cairo::Context,
    points: &[f64],
    positive: bool,
    area_width: f64,
    area_height: f64,
    detailed: bool,
    hover: Option<f64>,
) {
    let usable: Vec<f64> = points.iter().copied().filter(|v| v.is_finite()).collect();
    if usable.len() < 2 {
        return;
    }
    let (min, max) = crate::render::braille::bounds(&usable);
    let span = max - min;
    // The stroke must not be clipped by the edges of the area.
    let inset = 3.0;
    let plot_height = (area_height - inset * 2.0).max(1.0);
    let last = usable.len().saturating_sub(1);
    let step = if last == 0 {
        area_width
    } else {
        area_width / as_f64(last)
    };

    // Guide lines: a neutral grey at low opacity shows on a light background
    // as well as on a dark one, without competing with the curve.
    if detailed {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.18);
        ctx.set_line_width(1.0);
        for line in 0..=GRID_LINES {
            let ratio = f64::from(line) / f64::from(GRID_LINES);
            let y = (inset + ratio * plot_height).round() + 0.5;
            ctx.move_to(0.0, y);
            ctx.line_to(area_width, y);
        }
        let _ = ctx.stroke();
    }

    // The polyline is computed once then reused for the area, the stroke and
    // the end point.
    let polyline: Vec<(f64, f64)> = usable
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = as_f64(index) * step;
            let ratio = ((value - min) / span).clamp(0.0, 1.0);
            (x, inset + (1.0 - ratio) * plot_height)
        })
        .collect();

    let (red, green, blue) = if positive {
        (0.18, 0.75, 0.44)
    } else {
        (0.90, 0.28, 0.30)
    };

    // Area under the curve: a gradient fading downwards, to give volume
    // without hiding the guide lines.
    ctx.move_to(0.0, area_height);
    for (x, y) in &polyline {
        ctx.line_to(*x, *y);
    }
    ctx.line_to(area_width, area_height);
    ctx.close_path();
    let gradient = gtk4::cairo::LinearGradient::new(0.0, 0.0, 0.0, area_height);
    gradient.add_color_stop_rgba(0.0, red, green, blue, 0.32);
    gradient.add_color_stop_rgba(1.0, red, green, blue, 0.0);
    if ctx.set_source(&gradient).is_ok() {
        let _ = ctx.fill();
    }

    for (index, (x, y)) in polyline.iter().enumerate() {
        if index == 0 {
            ctx.move_to(*x, *y);
        } else {
            ctx.line_to(*x, *y);
        }
    }
    ctx.set_source_rgb(red, green, blue);
    ctx.set_line_width(if detailed { 2.0 } else { 1.4 });
    ctx.set_line_join(gtk4::cairo::LineJoin::Round);
    ctx.set_line_cap(gtk4::cairo::LineCap::Round);
    let _ = ctx.stroke();

    // Hover marker: a vertical stroke and a dot on the curve, at the sample
    // closest to the pointer.
    if let Some((x, y)) = hover
        .map(|ratio| sample_at(usable.len(), ratio))
        .and_then(|index| polyline.get(index))
        .filter(|_| detailed)
    {
        ctx.set_source_rgba(0.6, 0.6, 0.6, 0.45);
        ctx.set_line_width(1.0);
        ctx.move_to(x.round() + 0.5, 0.0);
        ctx.line_to(x.round() + 0.5, area_height);
        let _ = ctx.stroke();
        ctx.set_source_rgb(red, green, blue);
        ctx.arc(*x, *y, 3.4, 0.0, std::f64::consts::TAU);
        let _ = ctx.fill();
    }

    // End point: the eye finds the current price immediately.
    if let Some((x, y)) = polyline.last().filter(|_| detailed) {
        let x = x.min(area_width - 4.0);
        ctx.set_source_rgba(red, green, blue, 0.25);
        ctx.arc(x, *y, 6.0, 0.0, std::f64::consts::TAU);
        let _ = ctx.fill();
        ctx.set_source_rgb(red, green, blue);
        ctx.arc(x, *y, 2.6, 0.0, std::f64::consts::TAU);
        let _ = ctx.fill();
    }
}

/// Index of the sample sitting at the given relative abscissa.
fn sample_at(len: usize, ratio: f64) -> usize {
    let last = len.saturating_sub(1);
    let index = (ratio.clamp(0.0, 1.0) * as_f64(last)).round();
    // `as` on a bounded, positive float: the conversion is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = index as usize;
    index.min(last)
}

/// Age of the sample at the given index, spelled out.
///
/// The series carry no timestamp: the position in the curve is enough, since
/// the points of a range are evenly spaced.
fn age_at(range: Range, len: usize, index: usize) -> String {
    let last = len.saturating_sub(1);
    if last == 0 {
        return "now".to_owned();
    }
    let behind = as_f64(last.saturating_sub(index)) / as_f64(last);
    let minutes = behind * range.span_minutes();
    if minutes < 1.0 {
        "now".to_owned()
    } else if minutes < 60.0 {
        format!("{} min ago", minutes.round())
    } else if minutes < 1_440.0 {
        format!("{} h ago", (minutes / 60.0).round())
    } else {
        format!("{} d ago", (minutes / 1_440.0).round())
    }
}

/// Path of an export, in the home directory.
fn export_path(symbol: &str, range: Range, extension: &str) -> anyhow::Result<std::path::PathBuf> {
    let name = format!("cryptobar-{}-{}.{extension}", symbol, range.slug());
    Ok(crate::paths::export_dir()?.join(name))
}

/// Writes the curve as displayed into a PNG file.
///
/// # Errors
/// Fails when the Cairo surface cannot be created or when the write fails.
fn export_png(
    points: &[f64],
    positive: bool,
    symbol: &str,
    range: Range,
) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context as _;

    let path = export_path(symbol, range, "png")?;
    // Twice the screen size: the image stays sharp once enlarged.
    let width = CHART_WIDTH.saturating_mul(2);
    let height = CHART_HEIGHT.saturating_mul(2);
    let surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, width, height)
        .context("export surface unavailable")?;
    let ctx = gtk4::cairo::Context::new(&surface).context("export context unavailable")?;
    // Opaque background: a transparent PNG would be unreadable in most
    // viewers, which show it on white.
    ctx.set_source_rgb(0.07, 0.08, 0.10);
    let _ = ctx.paint();
    draw_series(
        &ctx,
        points,
        positive,
        f64::from(width),
        f64::from(height),
        true,
        None,
    );
    drop(ctx);
    let mut file = std::fs::File::create(&path)
        .with_context(|| format!("cannot write to {}", path.display()))?;
    surface
        .write_to_png(&mut file)
        .context("PNG encoding failed")?;
    Ok(path)
}

/// Writes the points of the curve into a CSV file.
///
/// # Errors
/// Fails when the write fails.
fn export_csv(points: &[f64], symbol: &str, range: Range) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context as _;

    let path = export_path(symbol, range, "csv")?;
    let mut out = String::from("index,age_minutes,price\n");
    let last = points.len().saturating_sub(1);
    for (index, value) in points.iter().enumerate() {
        let behind = if last == 0 {
            0.0
        } else {
            as_f64(last.saturating_sub(index)) / as_f64(last) * range.span_minutes()
        };
        let _ = writeln!(out, "{index},{behind:.0},{value}");
    }
    std::fs::write(&path, out).with_context(|| format!("cannot write to {}", path.display()))?;
    Ok(path)
}

/// Market data card, or `None` when the API published none.
fn stats_card(quote: &Quote, currency: &Currency, decimals: format::Decimals) -> Option<GtkBox> {
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
        // The gap to the all-time high reads better than the high alone: it
        // places the current price in the history of the coin.
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

/// Portfolio banner: split ring, total and subtitle.
fn wallet_hero(
    wallet: Option<&Portfolio>,
    watched: usize,
    currency: &Currency,
    decimals: format::Decimals,
) -> GtkBox {
    let hero = GtkBox::new(Orientation::Horizontal, 18);

    let ring = gtk4::Overlay::new();
    ring.set_child(Some(&donut(wallet)));
    let center = Label::new(Some(&match wallet {
        Some(wallet) => format!("{}", wallet.positions.len()),
        None => "0".to_owned(),
    }));
    center.add_css_class("ring-count");
    center.set_halign(Align::Center);
    center.set_valign(Align::Center);
    ring.add_overlay(&center);
    hero.append(&ring);

    let texts = GtkBox::new(Orientation::Vertical, 2);
    texts.set_valign(Align::Center);
    let total = Label::new(Some(&format::price(
        wallet.map_or(0.0, |wallet| wallet.total),
        currency,
        decimals,
    )));
    total.add_css_class("total");
    total.set_xalign(0.0);
    texts.append(&total);

    if let Some(gain) = wallet.and_then(|wallet| wallet.change_24h) {
        let sign = if gain >= 0.0 { "+" } else { "−" };
        let line = Label::new(Some(&format!(
            "{sign}{} over 24 h",
            format::price(gain.abs(), currency, decimals)
        )));
        line.add_css_class(trend_class(wallet.and_then(|wallet| wallet.change_24h_pct)));
        line.set_xalign(0.0);
        texts.append(&line);
    }

    let counts = match (wallet.map_or(0, |wallet| wallet.positions.len()), watched) {
        (0, 0) => "Add a public address to see its balance".to_owned(),
        (0, watched) => format!("{watched} watched address(es), no balance"),
        (positions, 0) => format!("{positions} position(s)"),
        (positions, watched) => format!("{positions} position(s) · {watched} watched address(es)"),
    };
    let subtitle = Label::new(Some(&counts));
    subtitle.add_css_class("muted");
    subtitle.set_xalign(0.0);
    texts.append(&subtitle);

    hero.append(&texts);
    hero
}

/// One position row: brand badge, symbol, quantity, value, share.
fn position_row(
    position: &portfolio::Position,
    currency: &Currency,
    decimals: format::Decimals,
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.add_css_class("wrow");

    let dot = Label::new(Some("●"));
    dot.add_css_class("dot");
    dot.add_css_class(&format!("dot-{}", css_key(&position.symbol)));
    row.append(&dot);

    let symbol = Label::new(Some(&position.symbol.to_uppercase()));
    symbol.add_css_class("rowsym");
    symbol.set_xalign(0.0);
    symbol.set_size_request(56, -1);
    row.append(&symbol);

    let quantity = Label::new(Some(&format::quantity(position.quantity)));
    quantity.add_css_class("muted");
    quantity.set_xalign(0.0);
    quantity.set_hexpand(true);
    row.append(&quantity);

    let value = Label::new(Some(&format::price(position.value, currency, decimals)));
    value.set_xalign(1.0);
    row.append(&value);

    let share = Label::new(Some(&format!("{:.1} %", position.share * 100.0)));
    share.add_css_class("chip");
    share.add_css_class("flat");
    share.set_width_chars(7);
    row.append(&share);
    row
}

/// Split ring of the portfolio, one slice per position.
fn donut(wallet: Option<&Portfolio>) -> gtk4::DrawingArea {
    const SIZE: i32 = 86;
    let slices: Vec<(f64, String)> = wallet
        .map(|wallet| {
            wallet
                .positions
                .iter()
                .map(|position| (position.share, position.color.clone()))
                .collect()
        })
        .unwrap_or_default();

    let area = gtk4::DrawingArea::new();
    area.set_content_width(SIZE);
    area.set_content_height(SIZE);
    area.set_valign(Align::Center);
    area.set_draw_func(move |_, ctx, width, height| {
        let size = f64::from(width.min(height));
        let center = size / 2.0;
        let radius = center - 8.0;
        ctx.set_line_width(11.0);

        // Background track: the ring exists even without a position, which
        // avoids a visual hole in the card when the portfolio is empty.
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.14);
        ctx.arc(center, center, radius, 0.0, std::f64::consts::TAU);
        let _ = ctx.stroke();

        let mut start = -std::f64::consts::FRAC_PI_2;
        for (share, color) in &slices {
            // A thin gap separates the slices: two close colours stay
            // distinguishable.
            let sweep = (share.clamp(0.0, 1.0) * std::f64::consts::TAU - 0.04).max(0.01);
            let (red, green, blue) = rgb(color);
            ctx.set_source_rgb(red, green, blue);
            ctx.set_line_cap(gtk4::cairo::LineCap::Round);
            ctx.arc(center, center, radius, start, start + sweep);
            let _ = ctx.stroke();
            start += share.clamp(0.0, 1.0) * std::f64::consts::TAU;
        }
    });
    area
}

/// Splits a `#rrggbb` colour into `0.0..=1.0` components.
///
/// An unexpected string yields a neutral grey: the ring stays readable.
fn rgb(color: &str) -> (f64, f64, f64) {
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
fn field(entry: &gtk4::Entry) -> Option<f64> {
    let text = entry.text().to_string();
    if text.trim().is_empty() {
        return None;
    }
    parse_number(&text)
}

/// Readable description of an alert.
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

/// Wires the hover readout onto the drawing area.
#[allow(clippy::too_many_arguments)]
fn attach_hover(
    area: &gtk4::DrawingArea,
    readout: &Label,
    hover: &Hover,
    samples: &[f64],
    range: Range,
    currency: &Currency,
    decimals: format::Decimals,
) {
    let motion = gtk4::EventControllerMotion::new();
    {
        let area = area.clone();
        let readout = readout.clone();
        let hover = Rc::clone(hover);
        let samples = samples.to_vec();
        let currency = currency.clone();
        motion.connect_motion(move |_, x, _| {
            let width = f64::from(area.width()).max(1.0);
            let ratio = (x / width).clamp(0.0, 1.0);
            hover.set(Some(ratio));
            let index = sample_at(samples.len(), ratio);
            if let Some(value) = samples.get(index) {
                readout.set_text(&format!(
                    "{} · {}",
                    format::price(*value, &currency, decimals),
                    age_at(range, samples.len(), index)
                ));
            }
            area.queue_draw();
        });
    }
    {
        let area = area.clone();
        let readout = readout.clone();
        let hover = Rc::clone(hover);
        motion.connect_leave(move |_| {
            hover.set(None);
            readout.set_text(" ");
            area.queue_draw();
        });
    }
    area.add_controller(motion);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_hovered_sample_stays_within_the_bounds() {
        assert_eq!(sample_at(10, 0.0), 0);
        assert_eq!(sample_at(10, 1.0), 9);
        assert_eq!(sample_at(10, 2.0), 9);
        assert_eq!(sample_at(0, 0.5), 0);
    }

    #[test]
    fn the_age_reads_in_the_fitting_unit() {
        assert_eq!(age_at(Range::D7, 8, 7), "now");
        assert_eq!(age_at(Range::H1, 61, 0), "1 h ago");
        assert_eq!(age_at(Range::H1, 3, 1), "30 min ago");
        assert_eq!(age_at(Range::H24, 5, 3), "6 h ago");
        assert_eq!(age_at(Range::D7, 8, 0), "7 d ago");
    }

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
