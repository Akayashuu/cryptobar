use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button};

use crate::model::{Quote, Range};
use crate::numeric::to_f64;

use super::chart::{CHART_HEIGHT, CHART_WIDTH, draw_series};
use super::error::PopupError;
use super::model::Ctx;
use super::status::root_cause;

impl Ctx {
    pub(super) fn export_buttons(
        self: &Rc<Self>,
        footer: &GtkBox,
        points: &[f64],
        positive: bool,
        quote: &Quote,
        range: Range,
    ) {
        for (label, tooltip, png) in [
            ("PNG", "Export the curve as an image", true),
            ("CSV", "Export the points as a spreadsheet", false),
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
                    Ok(path) => ctx.note(&format!("exported to {}", path.display()), false),
                    Err(failure) => ctx.note(&root_cause(&failure), true),
                }
            });
            footer.append(&button);
        }
    }
}

fn export_path(symbol: &str, range: Range, extension: &str) -> Result<PathBuf, PopupError> {
    let name = format!("cryptobar-{}-{}.{extension}", symbol, range.slug());
    Ok(crate::paths::export_dir()?.join(name))
}

fn export_png(
    points: &[f64],
    positive: bool,
    symbol: &str,
    range: Range,
) -> Result<PathBuf, PopupError> {
    let path = export_path(symbol, range, "png")?;
    let width = CHART_WIDTH.saturating_mul(2);
    let height = CHART_HEIGHT.saturating_mul(2);
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let ctx = cairo::Context::new(&surface)?;
    ctx.set_source_rgb(0.07, 0.08, 0.10);
    ctx.paint()?;
    draw_series(
        &ctx,
        points,
        positive,
        f64::from(width),
        f64::from(height),
        true,
        None,
    )?;
    drop(ctx);
    let mut file = std::fs::File::create(&path).map_err(|source| PopupError::Export {
        path: path.clone(),
        source,
    })?;
    surface
        .write_to_png(&mut file)
        .map_err(|source| PopupError::Png {
            path: path.clone(),
            source,
        })?;
    Ok(path)
}

fn export_csv(points: &[f64], symbol: &str, range: Range) -> Result<PathBuf, PopupError> {
    let path = export_path(symbol, range, "csv")?;
    let mut out = String::from("index,age_minutes,price\n");
    let last = points.len().saturating_sub(1);
    for (index, value) in points.iter().enumerate() {
        let behind = if last == 0 {
            0.0
        } else {
            to_f64(last.saturating_sub(index)) / to_f64(last) * range.span_minutes()
        };
        let row = format!("{index},{behind:.0},{value}\n");
        out.push_str(&row);
    }
    std::fs::write(&path, out).map_err(|source| write_failed(&path, source))?;
    Ok(path)
}

fn write_failed(path: &Path, source: std::io::Error) -> PopupError {
    PopupError::Export {
        path: path.to_path_buf(),
        source,
    }
}
