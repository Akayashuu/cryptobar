//! Sparkline made of Unicode blocks. Pure function, no I/O.

use crate::numeric::{saturating_index, to_f64};
use std::iter;

const LEVELS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const FLAT_LEVEL: char = '▄';

/// Reduces the series to at most `width` points by averaging contiguous slices.
/// Averaging rather than sampling keeps a lone spike from vanishing or taking over.
pub(crate) fn downsample(points: &[f64], width: usize) -> Vec<f64> {
    if points.len() <= width {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(width);
    out.extend((0..width).filter_map(|bucket| bucket_mean(points, bucket, width)));
    out
}

fn bucket_mean(points: &[f64], bucket: usize, width: usize) -> Option<f64> {
    let len = points.len();
    let start = bucket.saturating_mul(len).checked_div(width)?;
    let end = bucket
        .saturating_add(1)
        .saturating_mul(len)
        .checked_div(width)?
        .max(start.saturating_add(1));
    let slice = points.get(start..end.min(len))?;
    if slice.is_empty() {
        return None;
    }
    let sum: f64 = slice.iter().sum();
    Some(sum / to_f64(slice.len()))
}

/// Renders a numeric series as a sparkline of at most `width` characters.
///
/// Non-finite values are dropped. An empty series, a zero width or an entirely
/// non-finite series yield an empty string. A constant series renders a median
/// level rather than dividing by a zero amplitude.
#[must_use]
pub fn sparkline(points: &[f64], width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let finite: Vec<f64> = points
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    if finite.is_empty() {
        return String::new();
    }

    let sampled = downsample(&finite, width);
    let min = sampled.iter().copied().fold(f64::INFINITY, f64::min);
    let max = sampled.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let amplitude = max - min;

    if !amplitude.is_finite() || amplitude <= f64::EPSILON {
        return iter::repeat_n(FLAT_LEVEL, sampled.len()).collect();
    }

    let last_level = LEVELS.len().saturating_sub(1);
    sampled
        .iter()
        .map(|value| {
            let scaled = ((value - min) / amplitude) * to_f64(last_level);
            let level = saturating_index(scaled.round(), last_level);
            LEVELS.get(level).copied().unwrap_or(FLAT_LEVEL)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection;
    use proptest::num::f64::ANY;

    #[test]
    fn a_rising_series_climbs_in_steps() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0, 4.0], 4), "▁▃▆█");
    }

    #[test]
    fn a_flat_series_yields_a_median_level_without_dividing_by_zero() {
        assert_eq!(sparkline(&[5.0, 5.0, 5.0], 3), "▄▄▄");
    }

    #[test]
    fn an_empty_series_or_zero_width_yields_an_empty_string() {
        assert_eq!(sparkline(&[], 8), "");
        assert_eq!(sparkline(&[1.0, 2.0], 0), "");
    }

    #[test]
    fn a_single_value_yields_a_single_character() {
        assert_eq!(sparkline(&[42.0], 8), "▄");
    }

    #[test]
    fn non_finite_values_are_ignored() {
        let out = sparkline(&[1.0, f64::NAN, 3.0, f64::INFINITY], 3);
        assert_eq!(out.chars().count(), 2);
        assert!(!out.contains('\u{fffd}'));
    }

    #[test]
    fn the_series_is_downsampled_to_the_requested_width() {
        let points: Vec<f64> = (0..1000).map(f64::from).collect();
        assert_eq!(sparkline(&points, 8).chars().count(), 8);
    }

    #[test]
    fn a_series_shorter_than_the_width_is_not_stretched() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0], 8).chars().count(), 3);
    }

    proptest::proptest! {
        #[test]
        fn never_panics_and_respects_the_width(
            points in collection::vec(ANY, 0..200),
            width in 0usize..64,
        ) {
            let out = sparkline(&points, width);
            proptest::prop_assert!(out.chars().count() <= width);
        }
    }
}
