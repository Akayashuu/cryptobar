use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::channel;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, Orientation};

use crate::app::App;
use crate::model::{Quote, Range};

use super::model::{Ctx, Model, Msg, Widgets};
use super::search::wire_search;
use super::settings::{settings_panel, wire_settings};
use super::status::root_cause;
use super::style::CSS;
use super::worker::drain;

/// Application id, required by GTK.
const APP_ID: &str = "dev.akayashuu.cryptobar";

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
        if let Some((app, quotes, stale)) = seed.borrow_mut().take() {
            build_window(gtk_app, app, quotes, stale);
        } else if let Some(window) = gtk_app.windows().first() {
            toggle(window);
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

fn toggle(window: &gtk4::Window) {
    if window.is_active() {
        window.close();
    } else {
        window.present();
    }
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
            answered: false,
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
