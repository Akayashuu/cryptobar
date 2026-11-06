//! One-line rendering of a failure and its causes, `outer: cause: root`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

pub struct Report<'error>(pub &'error dyn Error);

impl Display for Report<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;
        let mut cause = self.0.source();
        while let Some(current) = cause {
            write!(f, ": {current}")?;
            cause = current.source();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("root cause")]
    struct Root;

    #[derive(Debug, thiserror::Error)]
    #[error("middle")]
    struct Middle(#[source] Root);

    #[derive(Debug, thiserror::Error)]
    #[error("outer")]
    struct Outer(#[source] Middle);

    #[test]
    fn the_chain_is_written_on_one_line() {
        let failure = Outer(Middle(Root));
        assert_eq!(Report(&failure).to_string(), "outer: middle: root cause");
    }

    #[test]
    fn an_error_without_cause_is_its_own_message() {
        assert_eq!(Report(&Root).to_string(), "root cause");
    }
}
