use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, ListBox, Orientation};

use crate::model::CoinId;
use crate::source::SearchHit;

use super::identity::identity_of;
use super::model::{Ctx, Msg};
use super::widgets::clear;

/// Delay left to typing before querying the API: below it, every letter would
/// fire a request.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(350);

/// Minimum length of a usable search term.
const SEARCH_MIN_LEN: usize = 2;

/// Number of results displayed under the search bar.
const SEARCH_LIMIT: usize = 8;

impl Ctx {
    pub(super) fn draw_results(self: &Rc<Self>) {
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

pub(super) fn wire_search(ctx: &Rc<Ctx>) {
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
