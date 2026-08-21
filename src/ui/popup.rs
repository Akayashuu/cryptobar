//! Fenêtre GTK4 : cours détaillés, courbe vectorielle, recherche et réglages.
//!
//! C'est la surface de configuration du projet. Tout ce que le fichier
//! `config.toml` accepte se règle ici, et chaque modification est écrite dans
//! ce même fichier — commentaires préservés — par [`crate::config_edit`].
//!
//! Le réseau ne tourne jamais sur le fil graphique : les requêtes partent dans
//! un fil dédié et reviennent par un canal que la boucle GTK draine. La
//! fenêtre reste donc fluide même quand l'API met une seconde à répondre.

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

/// Identifiant d'application, requis par GTK.
const APP_ID: &str = "dev.akayashuu.cryptobar";

/// Délai laissé à la frappe avant d'interroger l'API : au-dessous, chaque
/// lettre déclencherait une requête.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(350);

/// Longueur minimale d'un terme de recherche exploitable.
const SEARCH_MIN_LEN: usize = 2;

/// Nombre de résultats affichés sous la barre de recherche.
const SEARCH_LIMIT: usize = 8;

/// Cadence de drainage du canal réseau. Assez fin pour être imperceptible,
/// assez lâche pour ne rien coûter.
const DRAIN: Duration = Duration::from_millis(120);

/// Feuille de style de base.
///
/// Toutes les teintes sont dérivées de `@theme_bg_color` et `@theme_fg_color`
/// par les fonctions de couleur de GTK : la fenêtre s'accorde au thème du
/// bureau, clair comme sombre, sans le figer ni le lire.
const CSS: &str = "
window { background-color: @theme_bg_color; }

.card {
  background-color: mix(@theme_bg_color, @theme_fg_color, 0.05);
  border-radius: 20px;
  padding: 18px;
}
.hero { padding: 20px 22px; }

/* Onglets : des pastilles à plat, dans la même famille que les boutons de
   plage du graphique, plutôt que la barre d'onglets par défaut. GTK peint ses
   boutons avec un `background-image` : le neutraliser est nécessaire, la seule
   `background-color` ne suffirait pas. */
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
  /* Repli neutre : une pièce sans couleur de marque connue — un résultat de
     recherche, par exemple — reste lisible au lieu de disparaître. */
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

/* Bloc en creux : sépare les listes du fond de la carte. */
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

/* Pilules de plage temporelle. */
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

/// Géométrie du tracé dans la zone de dessin.
const CHART_WIDTH: i32 = 540;
const CHART_HEIGHT: i32 = 230;

/// Miniature tracée sur chaque ligne de la liste.
const SPARK_WIDTH: i32 = 72;
const SPARK_HEIGHT: i32 = 26;

/// Nombre de lignes de repère horizontales sous la courbe.
const GRID_LINES: i32 = 4;

/// Message remonté d'un fil de travail vers la boucle graphique.
enum Msg {
    /// Résultats d'une recherche, étiquetés par la génération qui les a demandés.
    Search(u64, Result<Vec<SearchHit>, String>),
    /// Cotations fraîchement chargées, avec le drapeau « servi depuis le cache ».
    Quotes(Vec<Quote>, bool),
    /// Série d'une plage autre que 7 jours, indexée par sa clé de cache.
    Series(String, Vec<f64>),
    /// Soldes fraîchement lus sur les chaînes suivies.
    Balances(Vec<crate::onchain::Balance>),
}

/// Ouvre la fenêtre et rend la main à sa fermeture.
///
/// # Errors
/// Échoue si GTK ne peut pas s'initialiser — typiquement hors session
/// graphique.
pub fn show(app: App, quotes: Vec<Quote>, stale: bool) -> anyhow::Result<()> {
    let gtk_app = Application::builder().application_id(APP_ID).build();
    let seed = Rc::new(RefCell::new(Some((Arc::new(app), quotes, stale))));

    gtk_app.connect_startup(|_| {
        let provider = gtk4::CssProvider::new();
        // `load_from_data` fonctionne sur toutes les versions de GTK 4, contrairement
        // à `load_from_string` qui exigerait GTK 4.12.
        provider.load_from_data(CSS);
        if let Some(display) = gtk4::gdk::Display::default() {
            // Au-dessus de `~/.config/gtk-4.0/gtk.css`, chargé par GTK en
            // priorité « user » : un thème de bureau qui repeint tous les
            // boutons ne doit pas décider de l'allure de cette fenêtre.
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    });

    gtk_app.connect_activate(move |gtk_app| {
        // GTK n'active qu'une fois par processus ; une seconde activation
        // (instance unique) se contente de présenter la fenêtre existante.
        if let Some((app, quotes, stale)) = seed.borrow_mut().take() {
            build_window(gtk_app, app, quotes, stale);
        } else if let Some(window) = gtk_app.windows().first() {
            window.present();
        }
    });

    // GTK analyserait sinon nos propres arguments de ligne de commande.
    let empty: [&str; 0] = [];
    let status = gtk_app.run_with_args(&empty);
    if status == glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        anyhow::bail!("la fenêtre s'est terminée en erreur")
    }
}

/// Données vivantes de la fenêtre.
struct Model {
    app: Arc<App>,
    quotes: Vec<Quote>,
    stale: bool,
    hits: Vec<SearchHit>,
    /// Plage affichée sous la courbe.
    range: Range,
    /// Séries déjà chargées, par couple (pièce, plage). La plage 7 jours vient
    /// des cotations et n'atterrit jamais ici.
    series: HashMap<String, Vec<f64>>,
    /// Soldes lus sur les adresses suivies, cumulés par cryptomonnaie.
    onchain: std::collections::BTreeMap<CoinId, f64>,
    /// Détail par adresse, pour la liste des portefeuilles suivis.
    balances: Vec<crate::onchain::Balance>,
    /// Numéro de la dernière recherche lancée : les réponses en retard, dont
    /// l'utilisateur a déjà changé le terme, sont ignorées.
    generation: u64,
}

/// Conteneurs reconstruits à chaque rendu.
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
    /// Fournisseur rechargé à chaque rendu : il porte une classe par pièce,
    /// avec sa couleur de marque. Générer la feuille évite l'API dépréciée
    /// `StyleContext::add_provider`.
    palette: gtk4::CssProvider,
    widgets: Widgets,
    tx: Sender<Msg>,
    /// Minuterie anti-rebond de la recherche, annulée à chaque frappe.
    debounce: RefCell<Option<glib::SourceId>>,
}

impl Ctx {
    fn app(&self) -> Arc<App> {
        Arc::clone(&self.model.borrow().app)
    }

    /// Affiche un message transitoire sous les réglages.
    fn note(&self, message: &str, error: bool) {
        self.widgets.status.set_text(message);
        self.widgets.status.remove_css_class("error");
        self.widgets.status.remove_css_class("muted");
        self.widgets
            .status
            .add_css_class(if error { "error" } else { "muted" });
    }

    /// Recharge les cotations sans bloquer l'interface.
    fn reload_quotes(&self) {
        let app = self.app();
        let tx = self.tx.clone();
        // `markets_cached` sert le cache tant qu'il est frais : ce fil ne
        // touche le réseau que lorsque c'est réellement nécessaire.
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

        // Les chaînes sont interrogées à part : une adresse lente ne doit pas
        // retarder l'affichage des cours.
        let app = self.app();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let balances = app.watcher().refresh(&app.config.wallets);
            let _ = tx.send(Msg::Balances(balances));
        });
    }

    /// Prend en compte une configuration réécrite : nouveau contexte, données
    /// rechargées, barre réveillée.
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
            let empty = Label::new(Some("Aucune donnée — vérifiez la connexion réseau"));
            empty.add_css_class("muted");
            empty.set_halign(Align::Start);
            self.widgets.detail.append(&empty);
        }
        if stale {
            let warning = Label::new(Some("Données du cache — réseau indisponible"));
            warning.add_css_class("muted");
            warning.set_halign(Align::Start);
            self.widgets.detail.append(&warning);
        }

        self.draw_list(active, &currency, decimals);
        self.draw_wallet(&currency, decimals);
        self.draw_alerts(&currency, decimals);
        self.draw_results();
    }

    /// Change la plage affichée sous la courbe.
    fn set_range(self: &Rc<Self>, range: Range) {
        if self.model.borrow().range == range {
            return;
        }
        self.model.borrow_mut().range = range;
        self.redraw();
    }

    /// Charge en tâche de fond la série d'une plage encore inconnue. La plage
    /// 7 jours est déjà dans les cotations : elle ne coûte jamais de requête.
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

    /// Carte du graphique : la courbe, son sélecteur de plage et ses bornes.
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

        // La couleur suit la variation observée sur la plage affichée, pas la
        // seule variation 24 h : sinon une courbe en baisse peut être verte.
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

        // Lecture au survol : le prix de l'échantillon pointé et son âge. La
        // ligne garde sa place même vide, sinon la carte sauterait au survol.
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
            let pending = Label::new(Some("chargement…"));
            pending.add_css_class("muted");
            footer.append(&pending);
        }

        self.export_buttons(&footer, &finite, positive, quote, range);

        card.append(&footer);
        card
    }

    /// Boutons d'export de la courbe affichée.
    fn export_buttons(
        self: &Rc<Self>,
        footer: &GtkBox,
        points: &[f64],
        positive: bool,
        quote: &Quote,
        range: Range,
    ) {
        for (label, tooltip, png) in [
            ("PNG", "Exporter la courbe en image", true),
            ("CSV", "Exporter les points en tableur", false),
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
                    Ok(path) => ctx.note(&format!("exporté dans {}", path.display()), false),
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

                // Miniature : la forme des sept derniers jours se lit d'un
                // coup d'œil, sans ouvrir le détail de chaque pièce.
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
                // Pièce tout juste ajoutée : la ligne existe avant sa cotation.
                let pending = Label::new(Some(id.as_str()));
                pending.add_css_class("muted");
                pending.set_hexpand(true);
                pending.set_xalign(0.0);
                row.append(&pending);
                let spinner = Label::new(Some("chargement…"));
                spinner.add_css_class("muted");
                row.append(&spinner);
            }

            let remove = Button::with_label("✕");
            remove.add_css_class("ghost");
            remove.set_valign(Align::Center);
            remove.set_tooltip_text(Some("Ne plus suivre"));
            let ctx = Rc::clone(self);
            let target = id.as_str().to_owned();
            remove.connect_clicked(move |_| {
                let written = ctx.app().editor().remove_coin(&target);
                ctx.adopt(written, &format!("{target} retirée"));
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

    /// Carte du portefeuille : total, répartition, positions et adresses
    /// suivies.
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
        let title = Label::new(Some("PORTEFEUILLE"));
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

    /// Section des adresses suivies : une ligne par adresse, et le formulaire
    /// pour en ajouter une.
    fn wallets_section(
        self: &Rc<Self>,
        wallets: &[crate::onchain::Wallet],
        balances: &[crate::onchain::Balance],
        quotes: &[Quote],
        currency: &Currency,
        decimals: format::Decimals,
    ) -> GtkBox {
        let section = GtkBox::new(Orientation::Vertical, 8);

        let title = Label::new(Some("ADRESSES SUIVIES"));
        title.add_css_class("subtitle");
        title.set_xalign(0.0);
        section.append(&title);

        // Sans cette phrase, « adresse publique » se confond avec « clé ». On
        // dit donc où la trouver, et ce que cryptobar refuse d'accepter.
        let help = Label::new(Some(
            "Collez l'adresse publique de votre portefeuille — celle que vous donnez pour \
             recevoir des fonds (Ledger Live : « Recevoir »). cryptobar lit le solde sur la \
             chaîne, sans jamais pouvoir dépenser. Une clé privée ou une phrase de \
             récupération est refusée et n'est jamais écrite sur le disque.",
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
                None => "lecture…".to_owned(),
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
            remove.set_tooltip_text(Some("Ne plus suivre cette adresse"));
            let ctx = Rc::clone(self);
            remove.connect_clicked(move |_| {
                let written = ctx.app().editor().remove_wallet(index);
                ctx.adopt(written, "adresse retirée");
            });
            row.append(&remove);
            section.append(&row);
        }

        section.append(&self.wallet_form());
        section
    }

    /// Exemple d'adresse pour une chaîne, montré en filigrane dans le champ.
    fn address_placeholder(chain: crate::onchain::Chain) -> &'static str {
        match chain {
            crate::onchain::Chain::Bitcoin => "bc1… ou 1… ou 3…",
            crate::onchain::Chain::Ethereum => "0x…",
            crate::onchain::Chain::Solana => "adresse Solana, ex. 9xQe…",
        }
    }

    /// Formulaire d'ajout d'une adresse suivie.
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

        // Le filigrane suit la chaîne choisie : une adresse Ethereum ne
        // ressemble à aucune adresse Bitcoin.
        let hinted = address.clone();
        picker.connect_selected_notify(move |picker| {
            let index = usize::try_from(picker.selected()).unwrap_or(0);
            if let Some(chain) = crate::onchain::Chain::ALL.get(index).copied() {
                hinted.set_placeholder_text(Some(Self::address_placeholder(chain)));
            }
        });

        let label = gtk4::Entry::new();
        label.set_placeholder_text(Some("nom, ex. Ledger"));
        label.set_width_chars(10);
        form.append(&label);

        let add = Button::with_label("Suivre");
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
            ctx.adopt(written, "adresse suivie — lecture du solde…");
        });
        form.append(&add);
        form
    }

    /// Carte des alertes : celles déjà posées, et de quoi en ajouter une.
    fn draw_alerts(self: &Rc<Self>, currency: &Currency, decimals: format::Decimals) {
        clear(&self.widgets.alerts);
        let model = self.model.borrow();
        let ids = model.app.config.coins.clone();
        let alerts = model.app.config.alerts.clone();
        let quotes = model.quotes.clone();
        drop(model);

        let card = GtkBox::new(Orientation::Vertical, 10);
        card.add_css_class("card");
        let title = Label::new(Some("ALERTES"));
        title.add_css_class("title");
        title.set_xalign(0.0);
        card.append(&title);

        if alerts.is_empty() {
            let empty = Label::new(Some(
                "Aucune alerte. Le suivi (`bar --watch`) notifie dès qu'un seuil est franchi.",
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
            remove.set_tooltip_text(Some("Supprimer l'alerte"));
            let ctx = Rc::clone(self);
            remove.connect_clicked(move |_| {
                let written = ctx.app().editor().remove_alert(index);
                ctx.adopt(written, "alerte supprimée");
            });
            row.append(&remove);
            card.append(&row);
        }

        if !ids.is_empty() {
            card.append(&self.alert_form(&ids, &quotes));
        }

        self.widgets.alerts.append(&card);
    }

    /// Formulaire d'ajout d'une alerte.
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
        above.set_placeholder_text(Some("au-dessus"));
        above.set_width_chars(10);
        form.append(&above);
        let below = gtk4::Entry::new();
        below.set_placeholder_text(Some("en dessous"));
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
            ctx.adopt(written, "alerte ajoutée");
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
                let already = Label::new(Some("suivie"));
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
        self.adopt(written, &format!("{id} ajoutée"));
        if ok {
            self.widgets.search.set_text("");
            self.model.borrow_mut().hits.clear();
            self.draw_results();
        }
    }

    /// Lance une recherche réseau dans un fil dédié.
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
                    // Les logos sont récupérés ici, dans le fil de recherche :
                    // seuls ceux réellement affichés sont téléchargés, et
                    // l'interface ne bloque jamais dessus.
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

/// Clé de cache d'une série dans le modèle.
fn series_key(id: &str, range: Range) -> String {
    format!("{id}-{}", range.slug())
}

/// Points à tracer pour une plage : les cotations couvrent déjà 7 jours, les
/// autres plages viennent du cache de séries.
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
        Range::H1 => "Dernière heure".to_owned(),
        Range::H24 => "Dernières 24 heures".to_owned(),
        Range::D7 => "7 derniers jours".to_owned(),
        Range::D30 => "30 derniers jours".to_owned(),
        Range::Y1 => "12 derniers mois".to_owned(),
    }
}

fn clear(container: &GtkBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Message le plus concret de la chaîne d'erreurs : c'est celui qui aide.
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
        .set_placeholder_text(Some("Chercher une cryptomonnaie à suivre…"));
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

    // Quatre pages plutôt qu'une colonne interminable : le portefeuille et
    // les réglages se trouvaient auparavant sous la ligne de flottaison, et
    // rien n'indiquait qu'il fallait faire défiler pour les atteindre.
    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.set_vexpand(true);
    stack.add_titled(
        &page(&[&widgets.detail, &widgets.list]),
        Some("cours"),
        "Cours",
    );
    stack.add_titled(
        &page(&[&widgets.wallet]),
        Some("portefeuille"),
        "Portefeuille",
    );
    stack.add_titled(&page(&[&widgets.alerts]), Some("alertes"), "Alertes");
    stack.add_titled(&page(&[&settings.panel]), Some("reglages"), "Réglages");

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

    // Échap ferme la fenêtre : c'est le réflexe attendu d'un popup. La barre
    // de recherche consomme la touche d'abord pour vider son champ.
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
    // La fenêtre s'ouvre prête à recevoir un terme : c'est le geste le plus
    // fréquent après « je veux suivre autre chose ».
    widgets.search.grab_focus();
}

/// Draine le canal des fils de travail à cadence fixe.
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
                        continue; // Réponse périmée : l'utilisateur a déjà retapé.
                    }
                    match outcome {
                        Ok(hits) => {
                            if hits.is_empty() {
                                ctx.note("Aucun résultat", false);
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

/// Ligne de titre : identité de la fenêtre et rafraîchissement manuel.
/// Une page d'onglet : ses sections empilées dans une zone défilante, pour
/// qu'un contenu long ne pousse jamais les onglets hors de la fenêtre.
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

    let refresh = Button::with_label("Rafraîchir");
    refresh.add_css_class("range");
    let owner = Rc::clone(ctx);
    refresh.connect_clicked(move |_| {
        let app = owner.app();
        if let Err(e) = crate::commands::nav::refresh(&app) {
            owner.note(&root_cause(&e), true);
            return;
        }
        owner.model.borrow_mut().series.clear();
        owner.note("Rafraîchissement en cours…", false);
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

/// Contrôles de réglage, construits une fois puis jamais reconstruits : leurs
/// valeurs sont celles du fichier au moment de l'ouverture.
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

    // Indice 0 = « auto », puis 0 à 8 décimales.
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

/// Remplit le panneau des réglages, groupe par groupe.
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
    // Trois groupes plutôt qu'une liste de huit lignes : on cherche un
    // réglage par ce qu'il touche, pas par son rang dans la configuration.
    for (heading, rows) in [
        (
            "COTATIONS",
            vec![
                (
                    "Devise",
                    "Monnaie dans laquelle les prix sont affichés",
                    currency.clone().upcast::<gtk4::Widget>(),
                ),
                (
                    "Rafraîchissement",
                    "Intervalle entre deux relevés, en secondes",
                    refresh.clone().upcast(),
                ),
                (
                    "Rotation automatique",
                    "Secondes entre deux pièces ; 0 pour la molette seule",
                    cycle.clone().upcast(),
                ),
            ],
        ),
        (
            "AFFICHAGE",
            vec![
                (
                    "Décimales",
                    "« auto » s'adapte à l'ordre de grandeur du prix",
                    decimals.clone().upcast(),
                ),
                (
                    "Courbe dans la barre",
                    "Sparkline à côté du prix",
                    sparkline.clone().upcast(),
                ),
                (
                    "Largeur de la courbe",
                    "Nombre de caractères de la sparkline",
                    sparkline_width.clone().upcast(),
                ),
            ],
        ),
        (
            "SYSTÈME",
            vec![
                (
                    "Signal Waybar",
                    "SIGRTMIN+n utilisé pour réveiller la barre",
                    signal.clone().upcast(),
                ),
                (
                    "Terminal",
                    "Ouvert par le clic droit ; validez par Entrée",
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

/// Une ligne de réglage : intitulé, explication d'une ligne, et le contrôle
/// aligné à droite. L'explication évite d'avoir à deviner ce que fait un
/// nombre nu comme « 8 ».
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

/// Conversion sans perte pour les plages manipulées ici (secondes, largeurs).
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
            &format!("Devise : {}", currency.as_str().to_uppercase()),
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
        owner.adopt(written, "Décimales enregistrées");
    });

    wire_spin(
        ctx,
        &settings.refresh,
        "refresh_interval",
        "Intervalle enregistré",
    );
    wire_spin(
        ctx,
        &settings.cycle,
        "cycle_interval",
        "Rotation enregistrée",
    );
    wire_spin(
        ctx,
        &settings.sparkline_width,
        "sparkline_width",
        "Largeur enregistrée",
    );
    wire_spin(ctx, &settings.signal, "waybar_signal", "Signal enregistré");

    let owner = Rc::clone(ctx);
    settings.sparkline.connect_state_set(move |_, active| {
        let written = owner.app().editor().set_bool("sparkline", active);
        owner.adopt(written, "Sparkline enregistrée");
        glib::Propagation::Proceed
    });

    let owner = Rc::clone(ctx);
    settings.terminal.connect_activate(move |entry| {
        let value = entry.text().trim().to_owned();
        if value.is_empty() {
            owner.note("Le terminal ne peut pas être vide", true);
            return;
        }
        let written = owner.app().editor().set_string("terminal", &value);
        owner.adopt(written, "Terminal enregistré");
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

/// Identité visuelle d'une cryptomonnaie : son logo officiel si le cache le
/// contient, sinon une pastille colorée portant son glyphe.
fn identity_widget(quote: &Quote, size: i32) -> gtk4::Widget {
    identity_of(quote.icon.as_deref(), &quote.symbol, size)
}

/// Même chose à partir des champs bruts, pour ce qui n'est pas une cotation —
/// un résultat de recherche, par exemple.
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

/// Une classe CSS par cryptomonnaie affichée, portant sa couleur de marque.
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

/// Réduit un symbole à un suffixe de classe CSS sûr.
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
        ("1 h", quote.change_1h),
        ("24 h", quote.change_24h),
        ("7 j", quote.change_7d),
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

/// Zone de dessin vectorielle : la courbe reste nette à toute résolution.
/// Abscisse survolée, dans `0.0..=1.0`, partagée entre le contrôleur de souris
/// et la fonction de dessin. `None` quand le pointeur est ailleurs.
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

/// Trace la courbe dans un contexte Cairo quelconque.
///
/// La fonction ne dépend pas du widget : c'est elle qui dessine à l'écran et
/// qui remplit la surface image de l'export PNG, ce qui garantit que le
/// fichier exporté est exactement ce que l'on voit.
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
    // Le trait ne doit pas être rogné par les bords de la zone.
    let inset = 3.0;
    let plot_height = (area_height - inset * 2.0).max(1.0);
    let last = usable.len().saturating_sub(1);
    let step = if last == 0 {
        area_width
    } else {
        area_width / as_f64(last)
    };

    // Lignes de repère : un gris neutre à faible opacité se voit sur un
    // fond clair comme sur un fond sombre, sans concurrencer la courbe.
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

    // La polyligne est calculée une fois puis réutilisée pour l'aire, le
    // trait et le point terminal.
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

    // Aire sous la courbe : un dégradé qui s'éteint vers le bas, pour
    // donner du volume sans masquer les lignes de repère.
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

    // Repère de survol : un trait vertical et un point sur la courbe, à
    // l'échantillon le plus proche du pointeur.
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

    // Point terminal : l'œil trouve immédiatement le cours actuel.
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

/// Indice de l'échantillon situé à l'abscisse relative donnée.
fn sample_at(len: usize, ratio: f64) -> usize {
    let last = len.saturating_sub(1);
    let index = (ratio.clamp(0.0, 1.0) * as_f64(last)).round();
    // `as` sur un flottant borné et positif : la conversion est exacte.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = index as usize;
    index.min(last)
}

/// Âge de l'échantillon d'indice donné, exprimé en clair.
///
/// Les séries ne portent pas d'horodatage : la position dans la courbe suffit,
/// puisque les points d'une plage sont régulièrement espacés.
fn age_at(range: Range, len: usize, index: usize) -> String {
    let last = len.saturating_sub(1);
    if last == 0 {
        return "maintenant".to_owned();
    }
    let behind = as_f64(last.saturating_sub(index)) / as_f64(last);
    let minutes = behind * range.span_minutes();
    if minutes < 1.0 {
        "maintenant".to_owned()
    } else if minutes < 60.0 {
        format!("il y a {} min", minutes.round())
    } else if minutes < 1_440.0 {
        format!("il y a {} h", (minutes / 60.0).round())
    } else {
        format!("il y a {} j", (minutes / 1_440.0).round())
    }
}

/// Chemin d'un export, dans le répertoire personnel.
fn export_path(symbol: &str, range: Range, extension: &str) -> anyhow::Result<std::path::PathBuf> {
    let name = format!("cryptobar-{}-{}.{extension}", symbol, range.slug());
    Ok(crate::paths::export_dir()?.join(name))
}

/// Écrit la courbe telle qu'elle est affichée dans un fichier PNG.
///
/// # Errors
/// Échoue si la surface Cairo ne peut être créée ou si l'écriture échoue.
fn export_png(
    points: &[f64],
    positive: bool,
    symbol: &str,
    range: Range,
) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context as _;

    let path = export_path(symbol, range, "png")?;
    // Deux fois la taille écran : l'image reste nette une fois agrandie.
    let width = CHART_WIDTH.saturating_mul(2);
    let height = CHART_HEIGHT.saturating_mul(2);
    let surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, width, height)
        .context("surface d'export indisponible")?;
    let ctx = gtk4::cairo::Context::new(&surface).context("contexte d'export indisponible")?;
    // Fond opaque : un PNG transparent serait illisible dans la plupart des
    // visionneuses, qui l'affichent sur blanc.
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
        .with_context(|| format!("écriture impossible dans {}", path.display()))?;
    surface
        .write_to_png(&mut file)
        .context("encodage PNG impossible")?;
    Ok(path)
}

/// Écrit les points de la courbe dans un fichier CSV.
///
/// # Errors
/// Échoue si l'écriture échoue.
fn export_csv(points: &[f64], symbol: &str, range: Range) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context as _;

    let path = export_path(symbol, range, "csv")?;
    let mut out = String::from("index,age_minutes,prix\n");
    let last = points.len().saturating_sub(1);
    for (index, value) in points.iter().enumerate() {
        let behind = if last == 0 {
            0.0
        } else {
            as_f64(last.saturating_sub(index)) / as_f64(last) * range.span_minutes()
        };
        // Point décimal et séparateur virgule : c'est ce qu'attendent les
        // tableurs, contrairement à l'affichage francisé de la fenêtre.
        let _ = writeln!(out, "{index},{behind:.0},{value}");
    }
    std::fs::write(&path, out)
        .with_context(|| format!("écriture impossible dans {}", path.display()))?;
    Ok(path)
}

/// Carte des données de marché, ou `None` si l'API n'en a publié aucune.
fn stats_card(quote: &Quote, currency: &Currency, decimals: format::Decimals) -> Option<GtkBox> {
    let mut rows: Vec<(String, String, &'static str)> = Vec::new();
    if let Some(rank) = quote.rank {
        rows.push(("Rang".to_owned(), format!("n° {rank}"), "flat"));
    }
    if let Some(cap) = quote.market_cap {
        rows.push((
            "Capitalisation".to_owned(),
            format::compact(cap, currency),
            "flat",
        ));
    }
    if let Some(volume) = quote.volume_24h {
        rows.push((
            "Volume 24 h".to_owned(),
            format::compact(volume, currency),
            "flat",
        ));
    }
    if let Some(ath) = quote.ath.filter(|ath| *ath > 0.0) {
        // L'écart au sommet historique se lit mieux que le sommet seul : il
        // situe le cours actuel dans l'histoire de la pièce.
        let gap = (quote.price - ath) / ath * 100.0;
        let date = quote
            .ath_date
            .as_deref()
            .and_then(|date| date.get(..10))
            .map(|date| format!(" · {date}"))
            .unwrap_or_default();
        rows.push((
            "Sommet historique".to_owned(),
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
    let title = Label::new(Some("MARCHÉ"));
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

/// Bandeau du portefeuille : anneau de répartition, total et sous-titre.
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
            "{sign}{} sur 24 h",
            format::price(gain.abs(), currency, decimals)
        )));
        line.add_css_class(trend_class(wallet.and_then(|wallet| wallet.change_24h_pct)));
        line.set_xalign(0.0);
        texts.append(&line);
    }

    let counts = match (wallet.map_or(0, |wallet| wallet.positions.len()), watched) {
        (0, 0) => "Ajoutez une adresse publique pour voir son solde".to_owned(),
        (0, watched) => format!("{watched} adresse(s) suivie(s), aucun solde"),
        (positions, 0) => format!("{positions} position(s)"),
        (positions, watched) => format!("{positions} position(s) · {watched} adresse(s) suivie(s)"),
    };
    let subtitle = Label::new(Some(&counts));
    subtitle.add_css_class("muted");
    subtitle.set_xalign(0.0);
    texts.append(&subtitle);

    hero.append(&texts);
    hero
}

/// Ligne d'une position : pastille de marque, symbole, quantité, valeur, part.
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

    let share = Label::new(Some(
        &format!("{:.1} %", position.share * 100.0).replace('.', ","),
    ));
    share.add_css_class("chip");
    share.add_css_class("flat");
    share.set_width_chars(7);
    row.append(&share);
    row
}

/// Anneau de répartition du portefeuille, une part par position.
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

        // Piste de fond : l'anneau existe même sans position, ce qui évite un
        // trou visuel dans la carte quand le portefeuille est vide.
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.14);
        ctx.arc(center, center, radius, 0.0, std::f64::consts::TAU);
        let _ = ctx.stroke();

        let mut start = -std::f64::consts::FRAC_PI_2;
        for (share, color) in &slices {
            // Un fin interstice sépare les parts : deux couleurs proches
            // restent distinguables.
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

/// Décompose une couleur `#rrggbb` en composantes `0.0..=1.0`.
///
/// Une chaîne inattendue donne un gris neutre : l'anneau reste lisible.
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

/// Lit un nombre saisi à la française comme à l'anglaise.
fn parse_number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .replace(['\u{202f}', '\u{a0}', ' '], "")
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Contenu numérique d'un champ, ou `None` s'il est vide ou illisible.
fn field(entry: &gtk4::Entry) -> Option<f64> {
    let text = entry.text().to_string();
    if text.trim().is_empty() {
        return None;
    }
    parse_number(&text)
}

/// Description lisible d'une alerte.
fn alert_caption(
    alert: &crate::config::Alert,
    currency: &Currency,
    decimals: format::Decimals,
) -> String {
    let mut parts = Vec::new();
    if let Some(above) = alert.above {
        parts.push(format!(
            "au-dessus de {}",
            format::price(above, currency, decimals)
        ));
    }
    if let Some(below) = alert.below {
        parts.push(format!("sous {}", format::price(below, currency, decimals)));
    }
    if let Some(change) = alert.change_pct {
        parts.push(format!("variation 24 h ≥ {change} %").replace('.', ","));
    }
    let mut caption = parts.join(" · ");
    if !alert.enabled {
        caption.push_str("  (désactivée)");
    }
    caption
}

/// Branche la lecture au survol sur la zone de dessin.
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
    fn lechantillon_survole_reste_dans_les_bornes() {
        assert_eq!(sample_at(10, 0.0), 0);
        assert_eq!(sample_at(10, 1.0), 9);
        assert_eq!(sample_at(10, 2.0), 9);
        assert_eq!(sample_at(0, 0.5), 0);
    }

    #[test]
    fn lage_se_lit_dans_lunite_qui_convient() {
        assert_eq!(age_at(Range::D7, 8, 7), "maintenant");
        assert_eq!(age_at(Range::H1, 61, 0), "il y a 1 h");
        assert_eq!(age_at(Range::H1, 3, 1), "il y a 30 min");
        assert_eq!(age_at(Range::H24, 5, 3), "il y a 6 h");
        assert_eq!(age_at(Range::D7, 8, 0), "il y a 7 j");
    }

    #[test]
    fn une_quantite_se_saisit_a_la_francaise_comme_a_langlaise() {
        assert_eq!(parse_number("0,25"), Some(0.25));
        assert_eq!(parse_number(" 1 234.5 "), Some(1234.5));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number(""), None);
    }

    #[test]
    fn une_couleur_illisible_donne_un_gris() {
        assert_eq!(rgb("#ffffff"), (1.0, 1.0, 1.0));
        assert_eq!(rgb("#000000"), (0.0, 0.0, 0.0));
        assert_eq!(rgb("bleu"), (0.5, 0.5, 0.5));
    }

    #[test]
    fn le_libelle_dalerte_decrit_toutes_les_conditions() {
        let config = crate::config::Config::parse(
            "coins = [\"bitcoin\"]\n[[alerts]]\ncoin = \"bitcoin\"\nabove = 100000\nchange_pct = 5\n",
        )
        .expect("configuration valide");
        let alert = config.alerts.first().expect("une alerte");
        let caption = alert_caption(alert, &Currency::default(), format::Decimals::Auto);
        assert!(caption.contains("au-dessus"), "{caption}");
        assert!(caption.contains('%'), "{caption}");
    }
}
