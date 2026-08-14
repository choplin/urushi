//! The only part of the inline render path that touches a terminal.
//!
//! Everything here is translation and I/O: an [`InlineRenderPlan`] in, bytes
//! out, plus the presentation updates the plan prescribes. Deciding *what* to
//! draw belongs to [`super::layout`] and [`super::inline_plan`].

use std::io::{self, Write};

use crossterm::{
    cursor, queue,
    terminal::{Clear, ClearType},
};

use super::{
    PromptStyles,
    inline_plan::{Checkpoint, InlineCommand, InlineRenderPlan},
    presentation::InlinePresentation,
};

/// Write `plan` to `writer`, keeping `presentation` in step with what has
/// actually reached the terminal. See the recovery contract on
/// [`super::inline_plan`].
pub(crate) fn execute<W: Write>(
    writer: &mut W,
    styles: &PromptStyles,
    presentation: &mut InlinePresentation,
    plan: InlineRenderPlan,
) -> io::Result<()> {
    if let Some(rows) = plan.claimed_rows {
        presentation.previous_rows = rows;
    }

    for step in plan.steps {
        write_command(writer, styles, &step.command)?;
        if let Some(checkpoint) = step.committed {
            commit(presentation, checkpoint);
        }
    }

    *presentation = plan.next;
    writer.flush()
}

fn commit(presentation: &mut InlinePresentation, checkpoint: Checkpoint) {
    match checkpoint {
        Checkpoint::OriginAnchored => {
            presentation.origin_saved = true;
            presentation.reserved_rows = 1;
        }
        Checkpoint::ReservedRows(rows) => presentation.reserved_rows = rows,
    }
}

fn write_command<W: Write>(
    writer: &mut W,
    styles: &PromptStyles,
    command: &InlineCommand,
) -> io::Result<()> {
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
        InlineCommand::WriteLine(line) => {
            for span in &line.spans {
                writer.write_all(styles.style(span.role).render(&span.text).as_bytes())?;
            }
            Ok(())
        }
    }
}
