use std::rc::Rc;

use gtk4::Label;
use gtk4::prelude::*;

use crate::model::{Currency, Range};
use crate::numeric::{saturating_index, to_f64};
use crate::render::format;

use super::chart::Hover;

pub(super) fn sample_at(len: usize, ratio: f64) -> usize {
    let last = len.saturating_sub(1);
    saturating_index((ratio.clamp(0.0, 1.0) * to_f64(last)).round(), last)
}

fn age_at(range: Range, len: usize, index: usize) -> String {
    let last = len.saturating_sub(1);
    if last == 0 {
        return "now".to_owned();
    }
    let behind = to_f64(last.saturating_sub(index)) / to_f64(last);
    let minutes = behind * range.span_minutes();
    if minutes < 1.0 {
        "now".to_owned()
    } else if minutes < 60.0 {
        format!("{} min ago", minutes.round())
    } else if minutes < 1_440.0 {
        format!("{} h ago", (minutes / 60.0).round())
    } else {
        format!("{} d ago", (minutes / 1_440.0).round())
    }
}

pub(super) fn attach_hover(
    area: &gtk4::DrawingArea,
    readout: &Label,
    hover: &Hover,
    samples: &[f64],
    range: Range,
    currency: &Currency,
    decimals: format::Decimals,
) {
    let motion = gtk4::EventControllerMotion::new();
    {
        let area = area.clone();
        let readout = readout.clone();
        let hover = Rc::clone(hover);
        let samples = samples.to_vec();
        let currency = currency.clone();
        motion.connect_motion(move |_, pos_x, _| {
            let width = f64::from(area.width()).max(1.0);
            let ratio = (pos_x / width).clamp(0.0, 1.0);
            hover.set(Some(ratio));
            let index = sample_at(samples.len(), ratio);
            if let Some(value) = samples.get(index) {
                readout.set_text(&format!(
                    "{} · {}",
                    format::price(*value, &currency, decimals),
                    age_at(range, samples.len(), index)
                ));
            }
            area.queue_draw();
        })
    };
    {
        let area = area.clone();
        let readout = readout.clone();
        let hover = Rc::clone(hover);
        motion.connect_leave(move |_| {
            hover.set(None);
            readout.set_text(" ");
            area.queue_draw();
        })
    };
    area.add_controller(motion);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hovered_sample_stays_within_the_bounds() {
        assert_eq!(sample_at(10, 0.0), 0);
        assert_eq!(sample_at(10, 1.0), 9);
        assert_eq!(sample_at(10, 2.0), 9);
        assert_eq!(sample_at(0, 0.5), 0);
    }

    #[test]
    fn the_age_reads_in_the_fitting_unit() {
        assert_eq!(age_at(Range::D7, 8, 7), "now");
        assert_eq!(age_at(Range::H1, 61, 0), "1 h ago");
        assert_eq!(age_at(Range::H1, 3, 1), "30 min ago");
        assert_eq!(age_at(Range::H24, 5, 3), "6 h ago");
        assert_eq!(age_at(Range::D7, 8, 0), "7 d ago");
    }
}
