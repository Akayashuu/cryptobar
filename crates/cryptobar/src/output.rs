//! The one channel through which the commands write: stdout for the results,
//! stderr for the warnings. Every line is written and flushed on a locked handle.

use std::fmt::Display;
use std::io::{self, Stderr, StderrLock, Stdout, StdoutLock, Write};

/// Writer pair: stdout for the results, stderr for the warnings.
#[derive(Debug)]
pub struct Output<Out, Err> {
    stdout: Out,
    stderr: Err,
}

/// [`Output`] bound to the standard streams of the process.
pub type ProcessOutput = Output<StdoutLock<'static>, StderrLock<'static>>;

impl ProcessOutput {
    /// Output bound to the locked standard streams of this process.
    #[must_use]
    pub fn process() -> Self {
        Output::new(Stdout::lock(&io::stdout()), Stderr::lock(&io::stderr()))
    }
}

impl<Out: Write, Err: Write> Output<Out, Err> {
    /// Output writing to the given streams.
    #[must_use]
    pub fn new(stdout: Out, stderr: Err) -> Self {
        Self { stdout, stderr }
    }

    /// # Errors
    /// Fails when stdout cannot be written or flushed.
    pub fn line(&mut self, text: impl Display) -> io::Result<()> {
        writeln!(self.stdout, "{text}")?;
        self.stdout.flush()
    }

    /// # Errors
    /// Fails when stdout cannot be written or flushed.
    pub fn text(&mut self, text: impl Display) -> io::Result<()> {
        write!(self.stdout, "{text}")?;
        self.stdout.flush()
    }

    /// # Errors
    /// Fails when stderr cannot be written or flushed.
    pub fn warning(&mut self, text: impl Display) -> io::Result<()> {
        writeln!(self.stderr, "{text}")?;
        self.stderr.flush()
    }
}

/// Appends a line, newline included, to a text built in memory.
pub fn push_line(buffer: &mut String, line: impl Display) {
    buffer.push_str(&line.to_string());
    buffer.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_go_to_their_own_stream() -> io::Result<()> {
        let mut output = Output::new(Vec::new(), Vec::new());
        output.line("price")?;
        output.warning("stale")?;
        assert_eq!(output.stdout, b"price\n");
        assert_eq!(output.stderr, b"stale\n");
        Ok(())
    }
}
