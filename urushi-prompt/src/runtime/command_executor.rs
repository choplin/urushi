//! The only part of the inline render path that emits terminal commands.
//!
//! Everything here is translation and I/O: an [`InlineRenderPlan`] in, bytes
//! out, plus the presentation updates the plan prescribes. Deciding *what* to
//! draw belongs to [`super::frame`] and [`super::inline_plan`].

use std::io;

use urushi::TerminalTextStyle;
use urushi_terminal::{
    Command, CommandWriter, HyperlinkParameter, TerminalHyperlink, TerminalText,
};

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
pub(crate) fn execute(
    writer: &mut (impl CommandWriter + ?Sized),
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
pub(crate) fn clear_viewport(writer: &mut (impl CommandWriter + ?Sized)) -> io::Result<()> {
    writer.write_command(Command::SetCursorVisible(false))?;
    writer.write_command(Command::Clear(urushi_terminal::ClearRegion::Screen))?;
    writer.write_command(Command::MoveCursor(urushi_terminal::CursorMove::To(
        urushi_terminal::Position::new(0, 0),
    )))?;
    writer.flush()
}

fn write_command(
    writer: &mut (impl CommandWriter + ?Sized),
    command: &InlineCommand,
) -> io::Result<()> {
    match command {
        InlineCommand::HideCursor => writer.write_command(Command::SetCursorVisible(false)),
        InlineCommand::ShowCursor => writer.write_command(Command::SetCursorVisible(true)),
        InlineCommand::SavePosition => writer.write_command(Command::SaveCursorPosition),
        InlineCommand::RestorePosition => writer.write_command(Command::RestoreCursorPosition),
        InlineCommand::MoveUp(rows) => {
            writer.write_command(Command::MoveCursor(urushi_terminal::CursorMove::By {
                columns: 0,
                rows: -i32::from(*rows),
            }))
        }
        InlineCommand::MoveDown(rows) => {
            writer.write_command(Command::MoveCursor(urushi_terminal::CursorMove::By {
                columns: 0,
                rows: i32::from(*rows),
            }))
        }
        InlineCommand::MoveRight(columns) => {
            writer.write_command(Command::MoveCursor(urushi_terminal::CursorMove::By {
                columns: i32::from(*columns),
                rows: 0,
            }))
        }
        InlineCommand::MoveToColumn(column) => writer.write_command(Command::MoveCursor(
            urushi_terminal::CursorMove::ToColumn(usize::from(*column)),
        )),
        InlineCommand::ClearLine => writer.write_command(Command::Clear(
            urushi_terminal::ClearRegion::AfterCursorInLine,
        )),
        InlineCommand::LineFeed => writer.write_command(Command::LineFeed),
        InlineCommand::CarriageReturnLineFeed => {
            writer.write_command(Command::CarriageReturnLineFeed)
        }
        InlineCommand::Write(row) => {
            for run in &row.runs {
                let text = TerminalText::try_from(run.text.as_str())
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
                let style = TerminalTextStyle::from(&run.style);
                writer.write_command(Command::SetStyle(style.style()))?;
                if let Some(link) = style.hyperlink() {
                    let parameters = link
                        .parameters()
                        .iter()
                        .map(|(key, value)| HyperlinkParameter { key, value })
                        .collect::<Vec<_>>();
                    writer.write_command(Command::SetHyperlink(Some(TerminalHyperlink {
                        uri: link.uri(),
                        parameters: &parameters,
                    })))?;
                    writer.write_command(Command::Print(text))?;
                    writer.write_command(Command::SetHyperlink(None))?;
                } else {
                    writer.write_command(Command::Print(text))?;
                }
                writer.write_command(Command::ResetStyle)?;
            }
            Ok(())
        }
    }
}
