use std::error::Error;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use gtk4::glib;

use crate::model::{Quote, Range};

use super::model::{Ctx, Msg, send_unless_closed};
use super::prices::series_key;
use super::status::root_cause;
use crate::onchain;
use std::thread;

const DRAIN: Duration = Duration::from_millis(120);

impl Ctx {
    pub(super) fn reload_quotes(&self) {
        let app = self.app();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let message = match app
                .source
                .markets_cached(&app.config.coins, &app.config.currency)
            {
                Ok(cached) => {
                    let mut quotes = cached.value;
                    let icon_failures = app.fetch_icons(&mut quotes);
                    Msg::Quotes(quotes, cached.stale, icon_failures)
                }
                Err(failure) => Msg::QuotesUnavailable(failure),
            };
            send_unless_closed(&tx, message);
        });

        let balance_app = self.app();
        let balance_tx = self.tx.clone();
        thread::spawn(move || {
            let refresh = balance_app.watcher().refresh(&balance_app.config.wallets);
            send_unless_closed(
                &balance_tx,
                Msg::Balances(refresh.balances, refresh.failures),
            );
        });
    }

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
        thread::spawn(move || {
            let outcome = app
                .source
                .series_cached(&id, range, &app.config.currency)
                .map(|cached| cached.value.points);
            send_unless_closed(&tx, Msg::Series(key, outcome));
        });
    }
}

fn report_first<E: Error>(ctx: &Ctx, failures: &[E]) {
    if let Some(failure) = failures.first() {
        ctx.note(&root_cause(failure), true);
    }
}

pub(super) fn drain(ctx: &Rc<Ctx>, rx: Receiver<Msg>) {
    let ctx = Rc::clone(ctx);
    glib::timeout_add_local(DRAIN, move || {
        loop {
            match rx.try_recv() {
                Ok(Msg::Quotes(quotes, stale, icon_failures)) => {
                    {
                        let mut model = ctx.model.borrow_mut();
                        model.quotes = quotes;
                        model.stale = stale;
                        model.answered = true;
                    };
                    report_first(&ctx, &icon_failures);
                    ctx.redraw();
                }
                Ok(Msg::QuotesUnavailable(failure)) => {
                    {
                        let mut model = ctx.model.borrow_mut();
                        model.stale = true;
                        model.answered = true;
                    };
                    ctx.note(&root_cause(&failure), true);
                    ctx.redraw();
                }
                Ok(Msg::Balances(balances, failures)) => {
                    {
                        let mut model = ctx.model.borrow_mut();
                        model.onchain = onchain::totals(&balances);
                        model.balances = balances;
                    };
                    report_first(&ctx, &failures);
                    ctx.redraw();
                }
                Ok(Msg::Series(key, Ok(points))) => {
                    ctx.model.borrow_mut().series.insert(key, points);
                    ctx.redraw();
                }
                Ok(Msg::Series(_, Err(failure))) => {
                    ctx.note(&root_cause(&failure), true);
                    ctx.redraw();
                }
                Ok(Msg::Search(generation, outcome, icon_failures)) => {
                    if ctx.model.borrow().generation != generation {
                        continue;
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
                        Err(failure) => {
                            ctx.note(&root_cause(&failure), true);
                            ctx.model.borrow_mut().hits.clear();
                        }
                    }
                    report_first(&ctx, &icon_failures);
                    ctx.draw_results();
                }
                Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }
    });
}
