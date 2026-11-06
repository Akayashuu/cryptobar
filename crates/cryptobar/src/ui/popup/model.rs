use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{SendError, Sender};

use gtk4::glib;
use gtk4::{Box as GtkBox, Label};

use crate::app::App;
use crate::model::{CoinId, Quote, Range};
use crate::onchain::{Balance, OnchainError};
use crate::source::icons::IconError;
use crate::source::{SearchHit, SourceError};

pub(super) enum Msg {
    Search(u64, Result<Vec<SearchHit>, SourceError>, Vec<IconError>),
    Quotes(Vec<Quote>, bool, Vec<IconError>),
    QuotesUnavailable(SourceError),
    Series(String, Result<Vec<f64>, SourceError>),
    Balances(Vec<Balance>, Vec<OnchainError>),
}

pub(super) fn send_unless_closed(tx: &Sender<Msg>, message: Msg) {
    match tx.send(message) {
        Ok(()) | Err(SendError(_)) => {}
    }
}

pub(super) struct Model {
    pub(super) app: Arc<App>,
    pub(super) quotes: Vec<Quote>,
    pub(super) stale: bool,
    pub(super) answered: bool,
    pub(super) hits: Vec<SearchHit>,
    pub(super) range: Range,
    pub(super) series: HashMap<String, Vec<f64>>,
    pub(super) onchain: BTreeMap<CoinId, f64>,
    pub(super) balances: Vec<Balance>,
    pub(super) generation: u64,
}

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
    pub(super) palette: gtk4::CssProvider,
    pub(super) widgets: Widgets,
    pub(super) tx: Sender<Msg>,
    pub(super) debounce: RefCell<Option<glib::SourceId>>,
}

impl Ctx {
    pub(super) fn app(&self) -> Arc<App> {
        Arc::clone(&self.model.borrow().app)
    }
}
