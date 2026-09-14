//! Private stderr output used by the transitional progress implementation.

use std::io::{self, Write};

use crate::{Available, ComponentTheme, RenderSettings, Size, View, render, resolve};
use urushi_terminal::{ColorLevel, TerminalDetection};

use super::{ProgressBar, Spinner};

const PLAIN_OUTPUT_WIDTH: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputMode {
    Live,
    Plain,
}

#[derive(Debug, Clone)]
pub(super) struct ProgressOutput {
    pub(super) settings: RenderSettings,
    pub(super) mode: OutputMode,
    width: usize,
}

impl ProgressOutput {
    pub(super) fn detect() -> io::Result<Self> {
        let stderr = io::stderr();
        let detection = urushi_terminal::detect(&stderr)?;
        let (mode, width, settings) = match detection {
            TerminalDetection::Terminal(info) => (
                output_mode(true, std::env::var("TERM").ok().as_deref()),
                info.size().columns(),
                RenderSettings::from(info.capabilities()),
            ),
            TerminalDetection::NonTerminal => (
                OutputMode::Plain,
                PLAIN_OUTPUT_WIDTH,
                RenderSettings::default(),
            ),
        };
        let mut settings = settings;
        if no_color() {
            settings = settings.with_colors(ColorLevel::None);
        }
        Ok(Self::new(settings, mode, width))
    }

    pub(super) fn new(settings: RenderSettings, mode: OutputMode, width: usize) -> Self {
        Self {
            settings,
            mode,
            width: width.max(10),
        }
    }

    pub(super) const fn mode(&self) -> OutputMode {
        self.mode
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
    fn rendered(&self, view: &View) -> (String, Size) {
        let resolved = resolve(view, self.available())
            .expect("progress views must have finite intrinsic geometry");
        let size = resolved.size();
        (render(&resolved, &self.settings), size)
    }

    pub(super) fn render_text(&self, view: &View) -> String {
        self.rendered(view).0
    }

    pub(super) fn write(&self, view: &View) -> io::Result<()> {
        let (rendered, size) = self.rendered(view);
        if size.height() == 0 {
            return Ok(());
        }
        let stderr = io::stderr();
        let mut writer = stderr.lock();
        writeln!(writer, "{rendered}")?;
        writer.flush()
    }

    pub(super) fn spinner(
        &self,
        styles: &ComponentTheme,
        message: impl Into<String>,
    ) -> io::Result<Spinner> {
        Spinner::with_output(self.clone(), styles.clone(), message.into())
    }

    pub(super) fn progress(
        &self,
        styles: &ComponentTheme,
        total: u64,
        message: impl Into<String>,
    ) -> io::Result<ProgressBar> {
        ProgressBar::with_output(self.clone(), styles.clone(), total, message.into())
    }
}

fn output_mode(is_terminal: bool, term: Option<&str>) -> OutputMode {
    if is_terminal && !term.is_some_and(|value| value.eq_ignore_ascii_case("dumb")) {
        OutputMode::Live
    } else {
        OutputMode::Plain
    }
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockStyle, Border, RenderSettings, TextStyle, measure};

    const SENTENCE: &str = "the quick brown fox jumps over the lazy dog";

    fn bordered_sentence() -> View {
        View::block(
            BlockStyle::new().border(Border::NORMAL),
            View::text(SENTENCE, TextStyle::new()),
        )
    }

    fn terminal(mode: OutputMode, width: usize) -> ProgressOutput {
        ProgressOutput::new(RenderSettings::default(), mode, width)
    }

    #[test]
    fn live_mode_requires_an_attended_non_dumb_terminal() {
        assert_eq!(output_mode(true, Some("unknown")), OutputMode::Live);
        assert_eq!(output_mode(true, Some("dumb")), OutputMode::Plain);
        assert_eq!(
            output_mode(false, Some("xterm-256color")),
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
        let (rendered, size) = terminal(OutputMode::Live, 20).rendered(&bordered_sentence());

        assert_eq!(size.width(), 20);
        assert!(
            size.height() > 3,
            "the text reflowed to the area, so the box is taller than one content row"
        );
        assert!(rendered.lines().all(|line| line.chars().count() == 20));
    }

    #[test]
    fn plain_output_keeps_the_intrinsic_size() {
        let view = bordered_sentence();
        let (_, size) = terminal(OutputMode::Plain, PLAIN_OUTPUT_WIDTH).rendered(&view);

        assert_eq!(size, measure(&view));
        assert_eq!(size.height(), 3);
    }
}
