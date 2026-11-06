use std::fmt::Write as _;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button};

use crate::model::{Quote, Range};
use crate::render::braille::as_f64;

use super::chart::{CHART_HEIGHT, CHART_WIDTH, draw_series};
use super::model::Ctx;
use super::status::root_cause;

impl Ctx {
    /// Export buttons of the displayed curve.
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
                    Err(e) => ctx.note(&root_cause(&e), true),
                }
            });
            footer.append(&button);
        }
    }
}

/// Path of an export, in the home directory.
fn export_path(symbol: &str, range: Range, extension: &str) -> anyhow::Result<std::path::PathBuf> {
    let name = format!("cryptobar-{}-{}.{extension}", symbol, range.slug());
    Ok(crate::paths::export_dir()?.join(name))
}

/// Writes the curve as displayed into a PNG file.
///
/// # Errors
/// Fails when the Cairo surface cannot be created or when the write fails.
fn export_png(
    points: &[f64],
    positive: bool,
    symbol: &str,
    range: Range,
) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context as _;

    let path = export_path(symbol, range, "png")?;
    // Twice the screen size: the image stays sharp once enlarged.
    let width = CHART_WIDTH.saturating_mul(2);
    let height = CHART_HEIGHT.saturating_mul(2);
    let surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, width, height)
        .context("export surface unavailable")?;
    let ctx = gtk4::cairo::Context::new(&surface).context("export context unavailable")?;
    // Opaque background: a transparent PNG would be unreadable in most
    // viewers, which show it on white.
    ctx.set_source_rgb(0.07, 0.08, 0.10);
    let _ = ctx.paint();
    draw_series(
        &ctx,
        points,
        positive,
        f64::from(width),
        f64::from(height),
        true,
        None,
    );
    drop(ctx);
    let mut file = std::fs::File::create(&path)
        .with_context(|| format!("cannot write to {}", path.display()))?;
    surface
        .write_to_png(&mut file)
        .context("PNG encoding failed")?;
    Ok(path)
}

/// Writes the points of the curve into a CSV file.
///
/// # Errors
/// Fails when the write fails.
fn export_csv(points: &[f64], symbol: &str, range: Range) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context as _;

    let path = export_path(symbol, range, "csv")?;
    let mut out = String::from("index,age_minutes,price\n");
    let last = points.len().saturating_sub(1);
    for (index, value) in points.iter().enumerate() {
        let behind = if last == 0 {
            0.0
        } else {
            as_f64(last.saturating_sub(index)) / as_f64(last) * range.span_minutes()
        };
        let _ = writeln!(out, "{index},{behind:.0},{value}");
    }
    std::fs::write(&path, out).with_context(|| format!("cannot write to {}", path.display()))?;
    Ok(path)
}
