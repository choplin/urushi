//! The only part of the inline render path that touches a terminal.
//!
//! Everything here is translation and I/O: an [`InlineRenderPlan`] in, bytes
//! out, plus the presentation updates the plan prescribes. Deciding *what* to
//! draw belongs to [`super::frame`] and [`super::inline_plan`].

use std::io::{self, Write};

use crossterm::{
    cursor, queue,
    terminal::{Clear, ClearType},
};
use urushi::{RenderSettings, StyledText, render_text};

use super::{
    inline_plan::{InlineCommand, InlineRenderPlan, RenderState, step},
    presentation::InlinePresentation,
};

/// Write `plan` to `writer`, keeping `presentation` in step with what has
/// actually reached the terminal.
///
/// Every command that succeeds is folded into the presentation, so a failure
/// partway leaves behind exactly what was emitted; `plan.next` is adopted only
/// once every command has been written. See the recovery contract on
/// [`super::inline_plan`].
pub(crate) fn execute<W: Write>(
    writer: &mut W,
    presentation: &mut InlinePresentation,
    plan: InlineRenderPlan,
) -> io::Result<()> {
    let mut state = RenderState::resuming(std::mem::take(presentation));

    for command in &plan.commands {
        if let Err(error) = write_command(writer, command) {
            // The drawn content is indeterminate past a failed write, and
            // cleanup erases the region regardless, so the rows are dropped
            // rather than folded.
            state.presentation.rows.clear();
            if !state.presentation.anchored {
                // The failure landed where no origin is saved: before the first
                // frame anchored one, or inside a growth window that had not
                // re-saved it yet. Either way the region's extent cannot be
                // established, so it is abandoned rather than erased.
                state.presentation.lose_region();
            }
            *presentation = state.presentation;
            return Err(error);
        }
        state = step(state, command);
    }

    *presentation = plan.next;
    writer.flush()
}

/// Erase the visible primary-buffer viewport and place the cursor at its
/// upper-left cell. Scrollback is intentionally left untouched.
pub(crate) fn clear_viewport<W: Write>(writer: &mut W) -> io::Result<()> {
    queue!(
        writer,
        cursor::Hide,
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    writer.flush()
}

fn write_command<W: Write>(writer: &mut W, command: &InlineCommand) -> io::Result<()> {
    match command {
        InlineCommand::HideCursor => queue!(writer, cursor::Hide),
        InlineCommand::ShowCursor => queue!(writer, cursor::Show),
        InlineCommand::SavePosition => queue!(writer, cursor::SavePosition),
        InlineCommand::RestorePosition => queue!(writer, cursor::RestorePosition),
        InlineCommand::MoveUp(rows) => queue!(writer, cursor::MoveUp(*rows)),
        InlineCommand::MoveDown(rows) => queue!(writer, cursor::MoveDown(*rows)),
        InlineCommand::MoveRight(columns) => queue!(writer, cursor::MoveRight(*columns)),
        InlineCommand::MoveToColumn(column) => queue!(writer, cursor::MoveToColumn(*column)),
        InlineCommand::ClearLine => queue!(writer, Clear(ClearType::UntilNewLine)),
        InlineCommand::LineFeed => writer.write_all(b"\n"),
        InlineCommand::CarriageReturnLineFeed => writer.write_all(b"\r\n"),
        InlineCommand::Write(row) => {
            for run in &row.runs {
                let text = StyledText::new(run.text.clone(), run.style.clone());
                writer.write_all(render_text(&text, &resolved_style_settings()).as_bytes())?;
            }
            Ok(())
        }
    }
}

/// Prompt styles have already been narrowed to terminal capabilities before
/// planning. This selection serializes those effective values without a
/// second degradation pass.
fn resolved_style_settings() -> RenderSettings {
    RenderSettings::all()
}
