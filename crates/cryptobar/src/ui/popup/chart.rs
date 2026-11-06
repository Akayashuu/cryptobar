use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::numeric::to_f64;

use super::hover::sample_at;
use crate::render::braille;
use crate::report::Report;
use gtk4::glib::WeakRef;
use std::f64::consts::TAU;

/// Geometry of the plot inside the drawing area.
pub(super) const CHART_WIDTH: i32 = 540;
pub(super) const CHART_HEIGHT: i32 = 230;

const GRID_LINES: i32 = 4;

/// Hovered abscissa, within `0.0..=1.0`, shared between the mouse controller
/// and the drawing function. `None` when the pointer is elsewhere.
pub(super) type Hover = Rc<Cell<Option<f64>>>;

pub(super) fn chart(
    points: &[f64],
    positive: bool,
    width: i32,
    height: i32,
    detailed: bool,
    hover: Option<Hover>,
) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::new();
    area.set_content_width(width);
    area.set_content_height(height);
    let points = points.to_vec();

    let widget = area.downgrade();
    area.set_draw_func(move |_, ctx, draw_width, draw_height| {
        let outcome = draw_series(
            ctx,
            &points,
            positive,
            f64::from(draw_width),
            f64::from(draw_height),
            detailed,
            hover.as_ref().and_then(|hover| hover.get()),
        );
        report_frame(&widget, outcome);
    });
    area
}

pub(super) fn report_frame(widget: &WeakRef<gtk4::DrawingArea>, outcome: Result<(), cairo::Error>) {
    if let (Err(failure), Some(area)) = (outcome, widget.upgrade()) {
        area.set_tooltip_text(Some(&Report(&failure).to_string()));
    }
}

/// Draws the curve into any Cairo context.
///
/// The function does not depend on the widget: it is the one drawing on screen
/// and filling the image surface of the PNG export, which guarantees the
/// exported file is exactly what one sees.
pub(super) fn draw_series(
    ctx: &cairo::Context,
    points: &[f64],
    positive: bool,
    area_width: f64,
    area_height: f64,
    detailed: bool,
    hover: Option<f64>,
) -> Result<(), cairo::Error> {
    let usable: Vec<f64> = points
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    if usable.len() < 2 {
        return Ok(());
    }
    let (min, max) = braille::bounds(&usable);
    let span = max - min;
    let inset = 3.0;
    let plot_height = (area_height - inset * 2.0).max(1.0);
    let last = usable.len().saturating_sub(1);
    let step = if last == 0 {
        area_width
    } else {
        area_width / to_f64(last)
    };

    if detailed {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.18);
        ctx.set_line_width(1.0);
        for line in 0..=GRID_LINES {
            let ratio = f64::from(line) / f64::from(GRID_LINES);
            let pos_y = (inset + ratio * plot_height).round() + 0.5;
            ctx.move_to(0.0, pos_y);
            ctx.line_to(area_width, pos_y);
        }
        ctx.stroke()?;
    }

    let polyline: Vec<(f64, f64)> = usable
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let pos_x = to_f64(index) * step;
            let ratio = ((value - min) / span).clamp(0.0, 1.0);
            (pos_x, inset + (1.0 - ratio) * plot_height)
        })
        .collect();

    let (red, green, blue) = if positive {
        (0.18, 0.75, 0.44)
    } else {
        (0.90, 0.28, 0.30)
    };

    ctx.move_to(0.0, area_height);
    for (pos_x, pos_y) in &polyline {
        ctx.line_to(*pos_x, *pos_y);
    }
    ctx.line_to(area_width, area_height);
    ctx.close_path();
    let gradient = cairo::LinearGradient::new(0.0, 0.0, 0.0, area_height);
    gradient.add_color_stop_rgba(0.0, red, green, blue, 0.32);
    gradient.add_color_stop_rgba(1.0, red, green, blue, 0.0);
    ctx.set_source(&gradient)?;
    ctx.fill()?;

    for (index, (pos_x, pos_y)) in polyline.iter().enumerate() {
        if index == 0 {
            ctx.move_to(*pos_x, *pos_y);
        } else {
            ctx.line_to(*pos_x, *pos_y);
        }
    }
    ctx.set_source_rgb(red, green, blue);
    ctx.set_line_width(if detailed { 2.0 } else { 1.4 });
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.stroke()?;

    if let Some((pos_x, pos_y)) = hover
        .map(|ratio| sample_at(usable.len(), ratio))
        .and_then(|index| polyline.get(index))
        .filter(|_| detailed)
    {
        ctx.set_source_rgba(0.6, 0.6, 0.6, 0.45);
        ctx.set_line_width(1.0);
        ctx.move_to(pos_x.round() + 0.5, 0.0);
        ctx.line_to(pos_x.round() + 0.5, area_height);
        ctx.stroke()?;
        ctx.set_source_rgb(red, green, blue);
        ctx.arc(*pos_x, *pos_y, 3.4, 0.0, TAU);
        ctx.fill()?;
    }

    if let Some((pos_x, pos_y)) = polyline.last().filter(|_| detailed) {
        let pos_x = pos_x.min(area_width - 4.0);
        ctx.set_source_rgba(red, green, blue, 0.25);
        ctx.arc(pos_x, *pos_y, 6.0, 0.0, TAU);
        ctx.fill()?;
        ctx.set_source_rgb(red, green, blue);
        ctx.arc(pos_x, *pos_y, 2.6, 0.0, TAU);
        ctx.fill()?;
    }
    Ok(())
}
