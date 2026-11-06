use crate::render::sparkline;

/// Number of points kept in the series handed to rich bars: enough to draw a
/// readable curve, few enough for the JSON line to stay short at every
/// interval.
const SERIES_POINTS: usize = 48;

/// Brings a series into [0, 1]. A flat series sits in the middle rather than
/// dividing by a zero amplitude.
pub(super) fn normalize(points: &[f64]) -> Vec<f64> {
    let finite: Vec<f64> = points.iter().copied().filter(|v| v.is_finite()).collect();
    let reduced = sparkline::downsample(&finite, SERIES_POINTS);
    let Some(min) = reduced.iter().copied().reduce(f64::min) else {
        return Vec::new();
    };
    let max = reduced.iter().copied().fold(min, f64::max);
    let span = max - min;
    if span <= 0.0 {
        return vec![0.5; reduced.len()];
    }
    // Three decimals are enough for a pixel-accurate plot and divide by five
    // the length of the JSON line emitted at every interval.
    reduced
        .iter()
        .map(|value| ((value - min) / span * 1000.0).round() / 1000.0)
        .collect()
}

#[cfg(test)]
// A failing test has to stop dead; normalisation puts exactly 0.0 and 1.0 on
// the extremes, so the strict comparison is exact there.
#[allow(clippy::panic, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::render::waybar::Markup;
    use crate::render::waybar::fixtures::{config, no_onchain, quote};
    use crate::render::waybar::render;

    #[test]
    fn the_published_series_fit_within_zero_and_one() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::None,
            &no_onchain(),
        );
        let Some(row) = module.coins.first() else {
            panic!("expected one quote");
        };
        let series = &row.series;
        assert_ne!(series.len(), 0);
        assert!(series.iter().all(|value| (0.0..=1.0).contains(value)));
        // The extremes touch the bounds: the curve fills the whole height.
        assert!(series.iter().copied().any(|value| value == 0.0));
        assert!(series.iter().copied().any(|value| value == 1.0));
    }

    #[test]
    fn a_flat_series_sits_in_the_middle() {
        assert_eq!(normalize(&[3.0, 3.0, 3.0]), vec![0.5, 0.5, 0.5]);
        assert_eq!(normalize(&[]).len(), 0);
        assert_eq!(normalize(&[f64::NAN]).len(), 0);
    }

    #[test]
    fn a_long_series_is_reduced() {
        let long: Vec<f64> = (0..500).map(f64::from).collect();
        assert_eq!(normalize(&long).len(), SERIES_POINTS);
    }
}
