use std::rc::Rc;
use std::sync::Arc;

use gtk4::prelude::*;
use gtk4::{Align, Label};

use crate::config::Config;
use crate::config_edit::EditError;
use crate::model::Range;

use super::identity::badge_css;
use super::model::Ctx;
use super::prices::{header, series_of, stats_card};
use super::status::root_cause;
use super::widgets::clear;

impl Ctx {
    pub(super) fn adopt(self: &Rc<Self>, written: Result<Config, EditError>, message: &str) {
        match written {
            Ok(_) => {
                let reloaded = self.model.borrow().app.reload();
                match reloaded {
                    Ok(app) => {
                        let app = Arc::new(app);
                        if let Err(failure) = app.notify() {
                            self.note(&root_cause(&failure), true);
                        }
                        self.model.borrow_mut().app = app;
                        self.note(message, false);
                        self.reload_quotes();
                        self.redraw();
                    }
                    Err(failure) => self.note(&root_cause(&failure), true),
                }
            }
            Err(failure) => self.note(&root_cause(&failure), true),
        }
    }

    pub(super) fn redraw(self: &Rc<Self>) {
        let model = self.model.borrow();
        self.palette.load_from_data(&badge_css(&model.quotes));
        let active = model.app.active_index().unwrap_or_else(|failure| {
            self.note(&root_cause(&failure), true);
            0
        });
        let currency = model.app.config.currency.clone();
        let decimals = model.app.config.decimals();
        let range = model.range;
        let selected = model
            .quotes
            .get(active)
            .or_else(|| model.quotes.first())
            .cloned();
        let stale = model.stale && !model.quotes.is_empty();
        let empty_caption = if model.answered {
            "No data: check the network connection"
        } else {
            "Loading prices…"
        };
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
            let empty = Label::new(Some(empty_caption));
            empty.add_css_class("muted");
            empty.set_halign(Align::Start);
            self.widgets.detail.append(&empty);
        }
        if stale {
            let warning = Label::new(Some("Cached data, network unavailable"));
            warning.add_css_class("muted");
            warning.set_halign(Align::Start);
            self.widgets.detail.append(&warning);
        }

        self.draw_list(active, &currency, decimals);
        self.draw_wallet(&currency, decimals);
        self.draw_alerts(&currency, decimals);
        self.draw_results();
    }

    pub(super) fn set_range(self: &Rc<Self>, range: Range) {
        if self.model.borrow().range == range {
            return;
        }
        self.model.borrow_mut().range = range;
        self.redraw();
    }
}
