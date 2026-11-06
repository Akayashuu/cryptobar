use gtk4::Box as GtkBox;
use gtk4::prelude::*;

pub(super) fn clear(container: &GtkBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

pub(super) fn trend_class(value: Option<f64>) -> &'static str {
    match value {
        Some(change) if change > 0.0 => "up",
        Some(change) if change < 0.0 => "down",
        _ => "flat",
    }
}
