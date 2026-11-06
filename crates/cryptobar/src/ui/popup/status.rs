use std::error::Error;

use gtk4::prelude::*;

use super::model::Ctx;

impl Ctx {
    pub(super) fn note(&self, message: &str, error: bool) {
        self.widgets.status.set_text(message);
        self.widgets.status.remove_css_class("error");
        self.widgets.status.remove_css_class("muted");
        self.widgets
            .status
            .add_css_class(if error { "error" } else { "muted" });
    }
}

pub(super) fn root_cause(error: &dyn Error) -> String {
    let mut root = error;
    while let Some(cause) = root.source() {
        root = cause;
    }
    root.to_string()
}
