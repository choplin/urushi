//! Indeterminate progress state and lifecycle.

use std::io;

use crate::{ComponentRole, ComponentStyles, View};

use super::{
    StderrTerminal,
    indicatif_backend::LiveRegion,
    themed_message,
    view::{self, FinishKind},
};
use crate::OutputMode;

/// Indeterminate work that leaves one stable completion record.
#[derive(Debug)]
pub struct Spinner {
    live: Option<LiveRegion>,
    terminal: StderrTerminal,
    styles: ComponentStyles,
    message: String,
    finished: bool,
}

impl Spinner {
    pub(crate) fn new(
        terminal: StderrTerminal,
        styles: ComponentStyles,
        message: String,
    ) -> io::Result<Self> {
        let live = match terminal.mode() {
            OutputMode::Live => {
                let rail = terminal
                    .renderer
                    .render(&View::text(
                        "│",
                        styles.text_style(ComponentRole::Muted).clone(),
                    ))
                    .into_string();
                let ticks = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"].map(|tick| {
                    terminal
                        .renderer
                        .render(&View::text(
                            tick,
                            styles.text_style(ComponentRole::Accent).clone(),
                        ))
                        .into_string()
                });
                Some(LiveRegion::spinner(
                    &rail,
                    &ticks,
                    themed_message(&terminal, &styles, &message),
                ))
            }
            OutputMode::Plain => {
                terminal.write(&view::work(&styles, "…", &message))?;
                None
            }
        };
        Ok(Self {
            live,
            terminal,
            styles,
            message,
            finished: false,
        })
    }

    /// Replaces the description of the active work.
    pub fn set_message(&mut self, message: impl Into<String>) -> io::Result<()> {
        self.message = message.into();
        if let Some(live) = &self.live {
            live.set_message(themed_message(&self.terminal, &self.styles, &self.message));
            Ok(())
        } else {
            self.terminal
                .write(&view::work(&self.styles, "…", &self.message))
        }
    }

    pub fn success(mut self, message: impl AsRef<str>) -> io::Result<()> {
        self.finish(FinishKind::Success, message.as_ref())
    }

    pub fn stop(mut self, message: impl AsRef<str>) -> io::Result<()> {
        self.finish(FinishKind::Neutral, message.as_ref())
    }

    pub fn error(mut self, message: impl AsRef<str>) -> io::Result<()> {
        self.finish(FinishKind::Error, message.as_ref())
    }

    fn finish(&mut self, kind: FinishKind, message: &str) -> io::Result<()> {
        if let Some(live) = self.live.take() {
            live.clear();
        }
        self.finished = true;
        self.terminal
            .write(&view::finished(&self.styles, kind, message))
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(live) = self.live.take() {
                live.clear();
            }
            drop(self.terminal.write(&view::finished(
                &self.styles,
                FinishKind::Error,
                &format!("{}: interrupted", self.message),
            )));
        }
    }
}
