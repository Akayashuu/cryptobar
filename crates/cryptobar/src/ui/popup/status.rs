use std::cmp::Reverse;
use std::error::Error;

use gtk4::prelude::*;

use crate::report::Report;
use crate::state::StateError;

use super::model::Ctx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn class(self) -> &'static str {
        match self {
            Self::Info => "muted",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) severity: Severity,
}

impl Notice {
    pub(super) fn new(text: impl Into<String>, severity: Severity) -> Self {
        Self {
            text: text.into(),
            severity,
        }
    }

    pub(super) fn of_state(notice: &StateError) -> Self {
        let severity = if matches!(notice, StateError::Corrupt { .. }) {
            Severity::Info
        } else {
            Severity::Warning
        };
        Self::new(Report(notice).to_string(), severity)
    }

    pub(super) fn failure(error: &dyn Error) -> Self {
        Self::new(root_cause(error), Severity::Error)
    }
}

pub(super) fn compose(notices: &[Notice]) -> Option<Notice> {
    let mut ordered: Vec<&Notice> = notices.iter().collect();
    ordered.sort_by_key(|notice| Reverse(notice.severity));
    let severity = ordered.first()?.severity;
    let text = ordered
        .iter()
        .map(|notice| notice.text.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    Some(Notice::new(text, severity))
}

impl Ctx {
    pub(super) fn note(&self, message: &str, error: bool) {
        let severity = if error {
            Severity::Error
        } else {
            Severity::Info
        };
        self.show_notice(&Notice::new(message, severity));
    }

    pub(super) fn show_notice(&self, notice: &Notice) {
        self.widgets.status.set_text(&notice.text);
        for severity in [Severity::Info, Severity::Warning, Severity::Error] {
            self.widgets.status.remove_css_class(severity.class());
        }
        self.widgets.status.add_css_class(notice.severity.class());
    }
}

pub(super) fn root_cause(error: &dyn Error) -> String {
    let mut root = error;
    while let Some(cause) = root.source() {
        root = cause;
    }
    root.to_string()
}

#[cfg(test)]
mod tests {
    use std::io::{self, ErrorKind};
    use std::path::PathBuf;

    use super::*;

    fn info(text: &str) -> Notice {
        Notice::new(text, Severity::Info)
    }

    fn warning(text: &str) -> Notice {
        Notice::new(text, Severity::Warning)
    }

    #[test]
    fn no_notice_composes_to_nothing() {
        assert_eq!(compose(&[]), None);
    }

    #[test]
    fn a_single_notice_keeps_its_text_and_severity() {
        assert_eq!(
            compose(&[info("exported to /tmp/chart.png")]),
            Some(info("exported to /tmp/chart.png"))
        );
    }

    #[test]
    fn every_notice_stays_visible_most_severe_first() {
        let composed = compose(&[info("repaired"), warning("unreadable"), info("exported")]);
        assert_eq!(composed, Some(warning("unreadable · repaired · exported")));
    }

    #[test]
    fn the_composition_takes_the_most_severe_severity() {
        let composed = compose(&[info("saved"), Notice::new("failed", Severity::Error)]);
        assert_eq!(
            composed,
            Some(Notice::new("failed · saved", Severity::Error))
        );
    }

    #[test]
    fn a_repaired_corrupt_state_is_informational() {
        let corrupt = StateError::Corrupt {
            path: PathBuf::from("state.json"),
            source: serde_json::from_str::<u8>("garbage").unwrap_err(),
        };
        let notice = Notice::of_state(&corrupt);
        assert_eq!(notice.severity, Severity::Info);
        assert!(notice.text.starts_with("the state file "));
        assert!(
            notice
                .text
                .contains(" is corrupt, starting from the default state")
        );
    }

    #[test]
    fn an_unreadable_state_is_a_warning() {
        let unreadable = StateError::Read {
            path: PathBuf::from("state.json"),
            source: io::Error::from(ErrorKind::PermissionDenied),
        };
        assert_eq!(Notice::of_state(&unreadable).severity, Severity::Warning);
    }
}
