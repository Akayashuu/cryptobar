//! Fenêtre GTK4 : cours détaillés et courbe dessinée au vecteur.
//!
//! Les données sont celles déjà en cache — la fenêtre ne déclenche aucune
//! requête supplémentaire, ce qui la rend instantanée et sans effet sur les
//! quotas de l'API.

use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, Box as GtkBox, Label, ListBox, Orientation};

use crate::config::Config;
use crate::model::Quote;
use crate::render::braille::as_f64;
use crate::render::format;

/// Identifiant d'application, requis par GTK.
const APP_ID: &str = "dev.akayashuu.cryptobar";

/// Feuille de style : la fenêtre doit rester lisible sur un thème clair
/// comme sur un thème sombre, sans dépendre du thème du bureau.
const CSS: &str = "
window { background-color: @theme_bg_color; }
.symbol { font-size: 22px; font-weight: bold; }
.price  { font-size: 30px; font-weight: bold; }
.up     { color: #3ba55d; }
.down   { color: #ed4245; }
.flat   { opacity: 0.7; }
.muted  { opacity: 0.6; font-size: 12px; }
row     { padding: 6px 10px; }
";

/// Géométrie du tracé dans la zone de dessin.
const CHART_WIDTH: i32 = 420;
const CHART_HEIGHT: i32 = 150;

/// Ouvre la fenêtre et rend la main à sa fermeture.
///
/// # Errors
/// Échoue si GTK ne peut pas s'initialiser — typiquement hors session
/// graphique.
pub fn show(quotes: Vec<Quote>, active: usize, config: &Config, stale: bool) -> anyhow::Result<()> {
    let app = Application::builder().application_id(APP_ID).build();
    let currency = config.currency.clone();
    let decimals = config.decimals();

    app.connect_startup(|_| {
        let provider = gtk4::CssProvider::new();
        // `load_from_data` fonctionne sur toutes les versions de GTK 4, contrairement
        // à `load_from_string` qui exigerait GTK 4.12.
        provider.load_from_data(CSS);
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    app.connect_activate(move |app| {
        build_window(app, &quotes, active, &currency, decimals, stale);
    });

    // GTK analyserait sinon nos propres arguments de ligne de commande.
    let empty: [&str; 0] = [];
    let status = app.run_with_args(&empty);
    if status == gtk4::glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        anyhow::bail!("la fenêtre s'est terminée en erreur")
    }
}

fn build_window(
    app: &Application,
    quotes: &[Quote],
    active: usize,
    currency: &crate::model::Currency,
    decimals: format::Decimals,
    stale: bool,
) {
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let selected = quotes.get(active).or_else(|| quotes.first());

    if let Some(quote) = selected {
        root.append(&header(quote, currency, decimals));
        root.append(&chart(
            &quote.sparkline,
            quote.change_7d.unwrap_or(0.0) >= 0.0,
        ));
    } else {
        let empty = Label::new(Some("Aucune donnée disponible"));
        empty.add_css_class("muted");
        root.append(&empty);
    }

    if quotes.len() > 1 {
        root.append(&coin_list(quotes, active, currency, decimals));
    }

    if stale {
        let warning = Label::new(Some("données périmées — réseau indisponible"));
        warning.add_css_class("muted");
        warning.set_halign(Align::Start);
        root.append(&warning);
    }

    let window = ApplicationWindow::builder()
        .application(app)
        .title("cryptobar")
        .default_width(480)
        .default_height(420)
        .resizable(true)
        .child(&root)
        .build();

    // Échap ferme la fenêtre : c'est le réflexe attendu d'un popup.
    let controller = gtk4::EventControllerKey::new();
    let to_close = window.clone();
    controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            to_close.close();
            return gtk4::glib::Propagation::Stop;
        }
        gtk4::glib::Propagation::Proceed
    });
    window.add_controller(controller);

    window.present();
}

fn header(quote: &Quote, currency: &crate::model::Currency, decimals: format::Decimals) -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 4);

    let symbol = Label::new(Some(&format!(
        "{}  ·  {}",
        quote.symbol.to_uppercase(),
        quote.name
    )));
    symbol.add_css_class("symbol");
    symbol.set_halign(Align::Start);
    container.append(&symbol);

    let price = Label::new(Some(&format::price(quote.price, currency, decimals)));
    price.add_css_class("price");
    price.set_halign(Align::Start);
    container.append(&price);

    let changes = GtkBox::new(Orientation::Horizontal, 16);
    for (label, value) in [
        ("1 h", quote.change_1h),
        ("24 h", quote.change_24h),
        ("7 j", quote.change_7d),
    ] {
        let text = Label::new(Some(&format!("{label}  {}", format::percent(value))));
        text.add_css_class(trend_class(value));
        changes.append(&text);
    }
    changes.set_halign(Align::Start);
    container.append(&changes);
    container
}

fn trend_class(value: Option<f64>) -> &'static str {
    match value {
        Some(v) if v > 0.0 => "up",
        Some(v) if v < 0.0 => "down",
        _ => "flat",
    }
}

/// Zone de dessin vectorielle : la courbe reste nette à toute résolution.
fn chart(points: &[f64], positive: bool) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::new();
    area.set_content_width(CHART_WIDTH);
    area.set_content_height(CHART_HEIGHT);
    let points = points.to_vec();

    area.set_draw_func(move |_, ctx, width, height| {
        let usable: Vec<f64> = points.iter().copied().filter(|v| v.is_finite()).collect();
        if usable.len() < 2 {
            return;
        }
        let (min, max) = crate::render::braille::bounds(&usable);
        let span = max - min;
        let area_width = f64::from(width);
        let area_height = f64::from(height);
        let last = usable.len().saturating_sub(1);
        let step = if last == 0 {
            area_width
        } else {
            area_width / as_f64(last)
        };

        // La polyligne est calculée une fois puis réutilisée pour l'aire et le trait.
        let polyline: Vec<(f64, f64)> = usable
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let x = as_f64(index) * step;
                let ratio = ((value - min) / span).clamp(0.0, 1.0);
                (x, area_height - ratio * area_height)
            })
            .collect();

        let (red, green, blue) = if positive {
            (0.23, 0.65, 0.36)
        } else {
            (0.93, 0.26, 0.27)
        };

        // Aire sous la courbe, puis la courbe elle-même par-dessus.
        ctx.move_to(0.0, area_height);
        for (x, y) in &polyline {
            ctx.line_to(*x, *y);
        }
        ctx.line_to(area_width, area_height);
        ctx.close_path();
        ctx.set_source_rgba(red, green, blue, 0.15);
        let _ = ctx.fill();

        for (index, (x, y)) in polyline.iter().enumerate() {
            if index == 0 {
                ctx.move_to(*x, *y);
            } else {
                ctx.line_to(*x, *y);
            }
        }
        ctx.set_source_rgb(red, green, blue);
        ctx.set_line_width(2.0);
        let _ = ctx.stroke();
    });
    area
}

fn coin_list(
    quotes: &[Quote],
    active: usize,
    currency: &crate::model::Currency,
    decimals: format::Decimals,
) -> ListBox {
    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    for (index, quote) in quotes.iter().enumerate() {
        let row = GtkBox::new(Orientation::Horizontal, 12);
        let symbol = Label::new(Some(&quote.symbol.to_uppercase()));
        symbol.set_width_chars(6);
        symbol.set_xalign(0.0);
        if index == active {
            symbol.add_css_class("symbol");
        }
        row.append(&symbol);

        let price = Label::new(Some(&format::price(quote.price, currency, decimals)));
        price.set_hexpand(true);
        price.set_xalign(0.0);
        row.append(&price);

        let change = Label::new(Some(&format::percent(quote.change_24h)));
        change.add_css_class(trend_class(quote.change_24h));
        row.append(&change);

        list.append(&row);
    }
    list
}
