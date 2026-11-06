use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use gtk4::glib;

use crate::model::{Quote, Range};

use super::model::{Ctx, Msg};
use super::prices::series_key;

/// Drain rate of the network channel. Fine enough to be imperceptible, loose
/// enough to cost nothing.
const DRAIN: Duration = Duration::from_millis(120);

impl Ctx {
    /// Reloads the quotes without blocking the interface.
    pub(super) fn reload_quotes(&self) {
        let app = self.app();
        let tx = self.tx.clone();
        // `markets_cached` serves the cache as long as it is fresh: this
        // thread touches the network only when it is truly needed.
        std::thread::spawn(move || {
            let message = match app
                .source
                .markets_cached(&app.config.coins, &app.config.currency)
            {
                Ok(cached) => {
                    let mut quotes = cached.value;
                    app.fetch_icons(&mut quotes);
                    Msg::Quotes(quotes, cached.stale)
                }
                Err(_) => Msg::QuotesUnavailable,
            };
            let _ = tx.send(message);
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

    /// Loads in the background the series of a still unknown range. The 7 day
    /// range is already in the quotes: it never costs a request.
    pub(super) fn request_series(self: &Rc<Self>, quote: &Quote, range: Range) {
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
}

/// Drains the worker channel at a fixed rate.
pub(super) fn drain(ctx: &Rc<Ctx>, rx: Receiver<Msg>) {
    let ctx = Rc::clone(ctx);
    glib::timeout_add_local(DRAIN, move || {
        loop {
            match rx.try_recv() {
                Ok(Msg::Quotes(quotes, stale)) => {
                    {
                        let mut model = ctx.model.borrow_mut();
                        model.quotes = quotes;
                        model.stale = stale;
                        model.answered = true;
                    }
                    ctx.redraw();
                }
                Ok(Msg::QuotesUnavailable) => {
                    {
                        let mut model = ctx.model.borrow_mut();
                        model.stale = true;
                        model.answered = true;
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
