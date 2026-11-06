use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use gtk4::glib;
use gtk4::{Box as GtkBox, Label};

use crate::app::App;
use crate::model::{CoinId, Quote, Range};
use crate::source::SearchHit;

/// Message sent from a worker thread up to the graphical loop.
pub(super) enum Msg {
    /// Results of a search, tagged by the generation that asked for them.
    Search(u64, Result<Vec<SearchHit>, String>),
    /// Freshly loaded quotes, with the "served from cache" flag.
    Quotes(Vec<Quote>, bool),
    QuotesUnavailable,
    /// Series of a range other than 7 days, keyed by its cache key.
    Series(String, Vec<f64>),
    /// Freshly read balances from the watched chains.
    Balances(Vec<crate::onchain::Balance>),
}

/// Live data of the window.
pub(super) struct Model {
    pub(super) app: Arc<App>,
    pub(super) quotes: Vec<Quote>,
    pub(super) stale: bool,
    pub(super) answered: bool,
    pub(super) hits: Vec<SearchHit>,
    /// Range displayed under the curve.
    pub(super) range: Range,
    /// Series already loaded, per (coin, range) pair. The 7 day range comes
    /// from the quotes and never lands here.
    pub(super) series: HashMap<String, Vec<f64>>,
    /// Balances read from the watched addresses, summed per cryptocurrency.
    pub(super) onchain: std::collections::BTreeMap<CoinId, f64>,
    /// Per-address detail, for the list of watched wallets.
    pub(super) balances: Vec<crate::onchain::Balance>,
    /// Number of the last search fired: late answers, whose term the user has
    /// already changed, are dropped.
    pub(super) generation: u64,
}

/// Containers rebuilt on every render.
#[derive(Clone)]
pub(super) struct Widgets {
    pub(super) search: gtk4::SearchEntry,
    pub(super) results: GtkBox,
    pub(super) detail: GtkBox,
    pub(super) list: GtkBox,
    pub(super) wallet: GtkBox,
    pub(super) alerts: GtkBox,
    pub(super) status: Label,
}

pub(super) struct Ctx {
    pub(super) model: RefCell<Model>,
    /// Provider reloaded on every render: it carries one class per coin, with
    /// its brand colour. Generating the sheet avoids the deprecated
    /// `StyleContext::add_provider` API.
    pub(super) palette: gtk4::CssProvider,
    pub(super) widgets: Widgets,
    pub(super) tx: Sender<Msg>,
    /// Debounce timer of the search, cancelled on every keystroke.
    pub(super) debounce: RefCell<Option<glib::SourceId>>,
}

impl Ctx {
    pub(super) fn app(&self) -> Arc<App> {
        Arc::clone(&self.model.borrow().app)
    }
}
