//! Conversions between integers and floats that never wrap or truncate silently.

use num_traits::ToPrimitive;

pub(crate) fn to_f64<T: ToPrimitive + Copy>(value: T) -> f64 {
    value.to_f64().unwrap_or(f64::NAN)
}

pub(crate) fn saturating_index(value: f64, max: usize) -> usize {
    match value.to_usize() {
        Some(index) => index.min(max),
        None if value > 0.0 => max,
        None => 0,
    }
}
