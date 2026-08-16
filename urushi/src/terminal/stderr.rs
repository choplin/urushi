//! Stable stderr output and live-mode detection.

use std::{
    env,
    io::{self, IsTerminal, Write},
};

use crate::{AnsiRenderer, Available, ComponentStyles, RenderedBlock, TerminalProfile, View};

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

    /// The area a written view is resolved under.
    ///
    /// A live terminal has a known width and an unknown height, so it bounds
    /// the width only. Without a terminal there is no area at all: the width
    /// held for `Plain` stands for "unconstrained", and passing it as a bound
    /// would make 4096 a real column count to wrap at.
    const fn available(&self) -> Available {
        match self.mode {
            OutputMode::Live => Available::columns(self.width),
            OutputMode::Plain => Available::NONE,
        }
    }

    /// Resolves a view under the terminal's area and serializes it.
    fn rendered(&self, view: &View) -> RenderedBlock {
        self.renderer.render_within(view, self.available())
    }

    pub fn write(&self, view: &View) -> io::Result<()> {
        let rendered = self.rendered(view);
        if rendered.size().height() == 0 {
            return Ok(());
        }
        let stderr = io::stderr();
        let mut writer = stderr.lock();
        writeln!(writer, "{rendered}")?;
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
    use crate::{AnsiPolicy, BlockStyle, Border, ColorProfile, TextStyle, measure, visible_width};

    const SENTENCE: &str = "the quick brown fox jumps over the lazy dog";

    fn bordered_sentence() -> View {
        View::block(
            BlockStyle::new().border(Border::NORMAL),
            View::text(SENTENCE, TextStyle::new()),
        )
    }

    fn terminal(mode: OutputMode, width: usize) -> StderrTerminal {
        StderrTerminal::new(
            TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled),
            mode,
            width,
        )
    }

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

    #[test]
    fn a_live_terminal_is_an_area_bounded_in_width_only() {
        assert_eq!(
            terminal(OutputMode::Live, 40).available(),
            Available::columns(40)
        );
    }

    #[test]
    fn without_a_terminal_there_is_no_area() {
        assert_eq!(
            terminal(OutputMode::Plain, PLAIN_OUTPUT_WIDTH).available(),
            Available::NONE
        );
    }

    #[test]
    fn a_view_wider_than_the_terminal_is_written_within_it() {
        let rendered = terminal(OutputMode::Live, 20).rendered(&bordered_sentence());

        assert_eq!(rendered.size().width(), 20);
        assert!(
            rendered.size().height() > 3,
            "the text reflowed to the area, so the box is taller than one content row"
        );
        let lines = rendered.into_string();
        for line in lines.lines() {
            assert_eq!(visible_width(line), 20, "every line closes at the area");
        }
    }

    #[test]
    fn plain_output_keeps_the_intrinsic_size() {
        let view = bordered_sentence();
        let rendered = terminal(OutputMode::Plain, PLAIN_OUTPUT_WIDTH).rendered(&view);

        assert_eq!(rendered.size(), measure(&view));
        assert_eq!(rendered.size().height(), 3);
    }
}
