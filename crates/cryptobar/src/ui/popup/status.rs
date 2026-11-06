use gtk4::prelude::*;

use super::model::Ctx;

impl Ctx {
    /// Shows a transient message under the settings.
    pub(super) fn note(&self, message: &str, error: bool) {
        self.widgets.status.set_text(message);
        self.widgets.status.remove_css_class("error");
        self.widgets.status.remove_css_class("muted");
        self.widgets
            .status
            .add_css_class(if error { "error" } else { "muted" });
    }
}

/// Most concrete message of the error chain: that is the helpful one.
pub(super) fn root_cause(error: &anyhow::Error) -> String {
    error
        .chain()
        .last()
        .map_or_else(|| error.to_string(), ToString::to_string)
}
