//! Curve plotting with braille characters (2 columns × 4 rows per glyph).
//!
//! The Unicode braille block starts at U+2800; every lit dot maps to one bit of
//! the pattern. As a cell offers eight dots, the vertical resolution is four
//! times that of block characters.

/// Mapping (column, row) → bit of the braille pattern.
/// Column 0 = dots 1,2,3,7; column 1 = dots 4,5,6,8.
const DOTS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// Braille dot canvas, addressable in pixels.
pub struct Canvas {
    /// Width and height in cells.
    cols: usize,
    rows: usize,
    /// One pattern per cell, indexed `row * cols + col`.
    cells: Vec<u8>,
}

impl Canvas {
    /// Creates a canvas of `cols` × `rows` cells, i.e. `2*cols` × `4*rows` dots.
    #[must_use]
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![0; cols.saturating_mul(rows)],
        }
    }

    /// Width in dots.
    #[must_use]
    pub fn width(&self) -> usize {
        self.cols.saturating_mul(2)
    }

    /// Height in dots.
    #[must_use]
    pub fn height(&self) -> usize {
        self.rows.saturating_mul(4)
    }

    /// Lights the dot `(x, y)`, origin at the top left. Off-canvas: ignored.
    pub fn set(&mut self, x: usize, y: usize) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let (col, sub_x) = (x.checked_div(2).unwrap_or(0), x.checked_rem(2).unwrap_or(0));
        let (row, sub_y) = (y.checked_div(4).unwrap_or(0), y.checked_rem(4).unwrap_or(0));
        let Some(bit) = DOTS.get(sub_x).and_then(|c| c.get(sub_y)) else {
            return;
        };
        let Some(index) = row.checked_mul(self.cols).and_then(|i| i.checked_add(col)) else {
            return;
        };
        if let Some(cell) = self.cells.get_mut(index) {
            *cell |= *bit;
        }
    }

    /// Renders the canvas as lines of text.
    #[must_use]
    pub fn to_lines(&self) -> Vec<String> {
        (0..self.rows)
            .map(|row| {
                (0..self.cols)
                    .map(|col| {
                        let index = row.saturating_mul(self.cols).saturating_add(col);
                        let pattern = self.cells.get(index).copied().unwrap_or(0);
                        // U+2800 + pattern: always a valid conversion over 8 bits.
                        char::from_u32(0x2800_u32.saturating_add(u32::from(pattern))).unwrap_or(' ')
                    })
                    .collect()
            })
            .collect()
    }
}

/// `usize` → `f64` conversion for position maths. Terminal dimensions fit very
/// comfortably in an `f64` mantissa; the degenerate case is clamped rather than
/// left imprecise.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn as_f64(value: usize) -> f64 {
    value.min(1 << 52) as f64
}

/// `f64` → `usize` conversion of a value already clamped to `0..=max`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_usize(value: f64, max: usize) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    (value as usize).min(max)
}

/// Vertical bounds of a series, widened when the series is flat.
#[must_use]
pub fn bounds(points: &[f64]) -> (f64, f64) {
    let finite = points.iter().copied().filter(|v| v.is_finite());
    let min = finite.clone().fold(f64::INFINITY, f64::min);
    let max = finite.fold(f64::NEG_INFINITY, f64::max);
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if (max - min).abs() < f64::EPSILON {
        // Flat series: open an arbitrary window to avoid a division by zero
        // and centre the line.
        return (min - 0.5, max + 0.5);
    }
    (min, max)
}

/// Plots a series on a canvas of `cols` × `rows` cells.
#[must_use]
pub fn plot(points: &[f64], cols: usize, rows: usize) -> Vec<String> {
    let mut canvas = Canvas::new(cols, rows);
    if points.is_empty() || cols == 0 || rows == 0 {
        return canvas.to_lines();
    }
    let (min, max) = bounds(points);
    let span = max - min;
    let width = canvas.width();
    let height = canvas.height();
    let last_x = width.saturating_sub(1);
    let last_y = height.saturating_sub(1);

    let mut previous: Option<(usize, usize)> = None;
    for x in 0..width {
        // Fractional position within the series for this dot column.
        let ratio = if last_x == 0 {
            0.0
        } else {
            as_f64(x) / as_f64(last_x)
        };
        let position = ratio * as_f64(points.len().saturating_sub(1));
        let index = as_usize(position.round(), points.len().saturating_sub(1));
        let Some(value) = points.get(index) else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        let normalized = ((value - min) / span).clamp(0.0, 1.0);
        // The vertical axis is flipped: the high value goes on top.
        let y = as_usize(((1.0 - normalized) * as_f64(last_y)).round(), last_y);
        if let Some((prev_x, prev_y)) = previous {
            connect(&mut canvas, prev_x, prev_y, x, y);
        }
        canvas.set(x, y);
        previous = Some((x, y));
    }
    canvas.to_lines()
}

/// Vertically joins two consecutive dots to avoid a dotted curve.
fn connect(canvas: &mut Canvas, _prev_x: usize, prev_y: usize, x: usize, y: usize) {
    let (low, high) = if prev_y <= y {
        (prev_y, y)
    } else {
        (y, prev_y)
    };
    for step in low..=high {
        canvas.set(x, step);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_canvas_holds_only_blank_braille() {
        let lines = Canvas::new(3, 2).to_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.chars().all(|c| c == '\u{2800}')));
    }

    #[test]
    fn the_first_dot_maps_to_the_expected_bit() {
        let mut canvas = Canvas::new(1, 1);
        canvas.set(0, 0);
        assert_eq!(canvas.to_lines().first().unwrap(), "\u{2801}");
    }

    #[test]
    fn the_eight_dots_of_a_cell_yield_the_full_glyph() {
        let mut canvas = Canvas::new(1, 1);
        for x in 0..2 {
            for y in 0..4 {
                canvas.set(x, y);
            }
        }
        assert_eq!(canvas.to_lines().first().unwrap(), "\u{28ff}");
    }

    #[test]
    fn an_off_canvas_dot_is_ignored_without_panicking() {
        let mut canvas = Canvas::new(1, 1);
        canvas.set(99, 99);
        assert_eq!(canvas.to_lines().first().unwrap(), "\u{2800}");
    }

    #[test]
    fn the_plot_dimensions_match_the_request() {
        let lines = plot(&[1.0, 2.0, 3.0], 10, 4);
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| l.chars().count() == 10));
    }

    #[test]
    fn a_rising_series_climbs_from_left_to_right() {
        let lines = plot(&(0..50).map(f64::from).collect::<Vec<_>>(), 20, 4);
        let first = lines.first().unwrap();
        let last = lines.last().unwrap();
        // The top of the chart is filled on the right, the bottom on the left.
        assert_ne!(last.chars().next(), Some('\u{2800}'));
        assert_ne!(first.chars().last(), Some('\u{2800}'));
    }

    #[test]
    fn a_flat_series_does_not_divide_by_zero() {
        let lines = plot(&[5.0; 20], 10, 3);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn non_finite_values_are_ignored() {
        let lines = plot(&[1.0, f64::NAN, 3.0, f64::INFINITY], 8, 3);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn bounds_widens_a_flat_series() {
        let (min, max) = bounds(&[2.0, 2.0]);
        assert!(max > min);
    }

    #[test]
    fn bounds_falls_back_to_an_interval_on_an_unusable_series() {
        assert_eq!(bounds(&[f64::NAN]), (0.0, 1.0));
    }

    #[test]
    fn an_empty_series_or_a_zero_canvas_do_not_panic() {
        assert_eq!(plot(&[], 10, 3).len(), 3);
        assert_eq!(plot(&[1.0, 2.0], 0, 0).len(), 0);
    }
}
