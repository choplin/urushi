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
            *presentation = state.presentation;
            return Err(error);
        }
        state = step(state, command);
    }

    *presentation = plan.next;
    writer.flush()
}

fn write_command<W: Write>(writer: &mut W, command: &InlineCommand) -> io::Result<()> {
    match command {
        InlineCommand::HideCursor => queue!(writer, cursor::Hide),
        InlineCommand::ShowCursor => queue!(writer, cursor::Show),
        InlineCommand::SaveOrigin => queue!(writer, cursor::SavePosition),
        InlineCommand::RestoreOrigin => queue!(writer, cursor::RestorePosition),
        InlineCommand::MoveUp(rows) => queue!(writer, cursor::MoveUp(*rows)),
        InlineCommand::MoveDown(rows) => queue!(writer, cursor::MoveDown(*rows)),
        InlineCommand::MoveRight(columns) => queue!(writer, cursor::MoveRight(*columns)),
        InlineCommand::MoveToColumn(column) => queue!(writer, cursor::MoveToColumn(*column)),
        InlineCommand::ClearToEndOfLine => queue!(writer, Clear(ClearType::UntilNewLine)),
        InlineCommand::ClearLine => queue!(writer, Clear(ClearType::CurrentLine)),
        InlineCommand::Newline => writer.write_all(b"\n"),
        InlineCommand::CarriageReturnNewline => writer.write_all(b"\r\n"),
        InlineCommand::WriteLine(row) => {
            for run in &row.runs {
                writer.write_all(run.style.paint(&run.text).as_bytes())?;
            }
            Ok(())
        }
    }
}
