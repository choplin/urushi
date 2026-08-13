//! Stable stderr output and live-mode detection.

use std::{
    env,
    io::{self, IsTerminal, Write},
};

use crate::{AnsiRenderer, ComponentStyles, TerminalProfile, View};

use super::{ProgressBar, Spinner};

const PLAIN_OUTPUT_WIDTH: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Live,
    Plain,
}

#[derive(Debug, Clone)]
pub struct StderrTerminal {
    pub(super) renderer: AnsiRenderer,
    pub(super) mode: OutputMode,
    width: usize,
}

impl StderrTerminal {
    pub fn detect() -> Self {
        let stderr = io::stderr();
        let profile = TerminalProfile::detect_for(&stderr);
        let mode = detect_output_mode(stderr.is_terminal(), env::var("TERM").ok().as_deref());
        let width = match mode {
            OutputMode::Live => usize::from(console::Term::stderr().size().1),
            OutputMode::Plain => PLAIN_OUTPUT_WIDTH,
        };
        Self::new(profile, mode, width)
    }

    pub fn new(profile: TerminalProfile, mode: OutputMode, width: usize) -> Self {
        Self {
            renderer: AnsiRenderer::new(profile),
            mode,
            width: width.max(10),
        }
    }

    pub const fn mode(&self) -> OutputMode {
        self.mode
    }
    pub const fn width(&self) -> usize {
        self.width
    }

    pub fn write(&self, view: &View) -> io::Result<()> {
        if view.is_empty() {
            return Ok(());
        }
        let stderr = io::stderr();
        let mut writer = stderr.lock();
        writeln!(writer, "{}", self.renderer.render(view))?;
        writer.flush()
    }

    pub fn spinner(
        &self,
        styles: &ComponentStyles,
        message: impl Into<String>,
    ) -> io::Result<Spinner> {
        Spinner::new(self.clone(), styles.clone(), message.into())
    }

    pub fn progress(
        &self,
        styles: &ComponentStyles,
        total: u64,
        message: impl Into<String>,
    ) -> io::Result<ProgressBar> {
        ProgressBar::new(self.clone(), styles.clone(), total, message.into())
    }
}

fn detect_output_mode(stderr_is_terminal: bool, term: Option<&str>) -> OutputMode {
    if stderr_is_terminal && !term.is_some_and(|value| value.eq_ignore_ascii_case("dumb")) {
        OutputMode::Live
    } else {
        OutputMode::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_mode_requires_an_attended_non_dumb_terminal() {
        assert_eq!(
            detect_output_mode(true, Some("xterm-256color")),
            OutputMode::Live
        );
        assert_eq!(detect_output_mode(true, Some("dumb")), OutputMode::Plain);
        assert_eq!(
            detect_output_mode(false, Some("xterm-256color")),
            OutputMode::Plain
        );
    }
}
