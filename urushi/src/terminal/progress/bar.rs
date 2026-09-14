//! Determinate progress state and lifecycle.

use std::io;

use crate::{ComponentRole, ComponentTheme, View};

use super::{
    indicatif_backend::LiveRegion,
    themed_message,
    view::{self, FinishKind},
};
use crate::terminal::stderr::{OutputMode, ProgressOutput};

/// Determinate work measured in a caller-defined unit.
#[derive(Debug)]
pub struct ProgressBar {
    live: Option<LiveRegion>,
    terminal: ProgressOutput,
    styles: ComponentTheme,
    total: u64,
    position: u64,
    message: String,
    finished: bool,
}

impl ProgressBar {
    /// Starts a determinate progress display on stderr.
    pub fn new(
        styles: &ComponentTheme,
        total: u64,
        message: impl Into<String>,
    ) -> io::Result<Self> {
        ProgressOutput::detect()?.progress(styles, total, message)
    }

    pub(in crate::terminal) fn with_output(
        terminal: ProgressOutput,
        styles: ComponentTheme,
        total: u64,
        message: String,
    ) -> io::Result<Self> {
        let live = match terminal.mode() {
            OutputMode::Live => {
                let rail = terminal.render_text(&View::text(
                    "│",
                    styles.text_style(ComponentRole::Muted).clone(),
                ));
                let progress = terminal.render_text(&View::text(
                    "{wide_bar}",
                    styles.text_style(ComponentRole::Accent).clone(),
                ));
                Some(LiveRegion::progress(
                    total,
                    &rail,
                    &progress,
                    themed_message(&terminal, &styles, &message),
                ))
            }
            OutputMode::Plain => {
                terminal.write(&view::progress(&styles, 0, total, &message))?;
                None
            }
        };
        Ok(Self {
            live,
            terminal,
            styles,
            total,
            position: 0,
            message,
            finished: false,
        })
    }

    pub const fn total(&self) -> u64 {
        self.total
    }

    pub const fn position(&self) -> u64 {
        self.position
    }

    pub fn set_position(&mut self, position: u64) -> io::Result<()> {
        self.position = position.min(self.total);
        if let Some(live) = &self.live {
            live.set_position(self.position);
            Ok(())
        } else {
            self.terminal.write(&view::progress(
                &self.styles,
                self.position,
                self.total,
                &self.message,
            ))
        }
    }

    /// `message` is plain text: escape sequences and cursor movement in it break
    /// that contract, and debug builds panic on them.
    pub fn set_message(&mut self, message: impl Into<String>) -> io::Result<()> {
        self.message = message.into();
        if let Some(live) = &self.live {
            live.set_message(themed_message(&self.terminal, &self.styles, &self.message));
            Ok(())
        } else {
            self.terminal.write(&view::progress(
                &self.styles,
                self.position,
                self.total,
                &self.message,
            ))
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

impl Drop for ProgressBar {
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
