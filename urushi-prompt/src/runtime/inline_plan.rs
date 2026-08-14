//! Terminal commands planned as data, before anything is written.
//!
//! The prompt owns a region of rows anchored at the position it saved on its
//! first draw. Planning that region's updates as a value — rather than emitting
//! them straight into a writer — lets the command order and the owned-region
//! state transitions be tested without a terminal, and keeps the crossterm
//! executor free of layout and diffing.
//!
//! # Recovery contract
//!
//! A plan claims pessimistically and commits optimistically:
//!
//! - [`InlineRenderPlan::claimed_rows`] is applied *before* the first command
//!   is written. It over-states the rows the plan may touch so that, if any
//!   write fails midway, error cleanup erases the whole partially drawn view.
//! - [`CommandStep::committed`] is applied *after* its command is written. It
//!   records what has actually become true of the terminal, so a failure leaves
//!   behind state that describes what was really emitted.
//! - [`InlineRenderPlan::next`] is applied only once every command succeeded.

use super::{
    RenderFinish,
    layout::{LaidOutView, RenderedLine},
    presentation::InlinePresentation,
};

/// A terminal operation in the prompt's own vocabulary.
///
/// Deliberately not a crossterm re-export: reserving rows is a bare line feed,
/// which crossterm has no command for, and this vocabulary is the thing kept
/// aligned with the sibling MoonBit implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InlineCommand {
    HideCursor,
    ShowCursor,
    /// Anchor the owned region at the current cursor position.
    SaveOrigin,
    /// Return to the anchored origin.
    RestoreOrigin,
    MoveUp(u16),
    MoveDown(u16),
    MoveRight(u16),
    MoveToColumn(u16),
    /// Erase from the cursor to the end of the row.
    ClearToEndOfLine,
    /// Erase the whole row.
    ClearLine,
    /// A bare line feed, which scrolls a new row into existence at the bottom.
    Newline,
    /// A real carriage return plus line feed. Unlike relative cursor movement,
    /// this scrolls at the terminal boundary instead of stopping there.
    CarriageReturnNewline,
    /// Write one laid-out row, styled by the executor.
    WriteLine(RenderedLine),
}

/// State that becomes true once its command has been written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Checkpoint {
    /// The origin is anchored, and the row it sits on is reserved.
    OriginAnchored,
    /// Rows scrolled into existence below the origin.
    ReservedRows(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandStep {
    pub command: InlineCommand,
    pub committed: Option<Checkpoint>,
}

impl CommandStep {
    fn plain(command: InlineCommand) -> Self {
        Self {
            command,
            committed: None,
        }
    }

    fn committing(command: InlineCommand, checkpoint: Checkpoint) -> Self {
        Self {
            command,
            committed: Some(checkpoint),
        }
    }
}

pub(crate) struct InlineRenderPlan {
    /// Rows to claim before the first write. See the recovery contract above.
    pub claimed_rows: Option<u16>,
    pub steps: Vec<CommandStep>,
    /// The presentation once every command has been written.
    pub next: InlinePresentation,
}

impl InlineRenderPlan {
    /// The planned commands, for tests that assert on ordering.
    #[cfg(test)]
    pub(crate) fn commands(&self) -> Vec<InlineCommand> {
        self.steps.iter().map(|step| step.command.clone()).collect()
    }

    /// The planned checkpoints in order, for tests that assert on recovery.
    #[cfg(test)]
    pub(crate) fn checkpoints(&self) -> Vec<Checkpoint> {
        self.steps
            .iter()
            .filter_map(|step| step.committed)
            .collect()
    }
}

/// Plan the commands that bring the owned region from `previous` to `view`.
///
/// `max_rows` is the terminal height; the plan never claims more than that.
pub(crate) fn plan_draw(
    view: LaidOutView,
    previous: &InlinePresentation,
    max_rows: u16,
) -> InlineRenderPlan {
    let LaidOutView { lines, cursor } = view;
    let rows_to_touch = previous
        .previous_lines
        .len()
        .max(lines.len())
        .min(usize::from(max_rows)) as u16;

    let mut steps = vec![CommandStep::plain(InlineCommand::HideCursor)];

    let mut reserved_rows = previous.reserved_rows;
    if !previous.origin_saved {
        steps.push(CommandStep::committing(
            InlineCommand::SaveOrigin,
            Checkpoint::OriginAnchored,
        ));
        reserved_rows = 1;
    }

    let wanted_rows = (lines.len() as u16).max(1);
    if wanted_rows > reserved_rows {
        steps.push(CommandStep::plain(InlineCommand::RestoreOrigin));
        if reserved_rows > 1 {
            steps.push(CommandStep::plain(InlineCommand::MoveDown(
                reserved_rows - 1,
            )));
        }
        for _ in reserved_rows..wanted_rows {
            steps.push(CommandStep::plain(InlineCommand::Newline));
        }
        if wanted_rows > 1 {
            steps.push(CommandStep::plain(InlineCommand::MoveUp(wanted_rows - 1)));
        }
        steps.push(CommandStep::committing(
            InlineCommand::SaveOrigin,
            Checkpoint::ReservedRows(wanted_rows),
        ));
        reserved_rows = wanted_rows;
    }

    for row in 0..usize::from(rows_to_touch) {
        let current = lines.get(row);
        if previous.previous_lines.get(row) == current {
            continue;
        }

        steps.push(CommandStep::plain(InlineCommand::RestoreOrigin));
        if row > 0 {
            steps.push(CommandStep::plain(InlineCommand::MoveDown(
                row.min(usize::from(u16::MAX)) as u16,
            )));
            steps.push(CommandStep::plain(InlineCommand::MoveToColumn(0)));
        }
        steps.push(CommandStep::plain(if row == 0 {
            InlineCommand::ClearToEndOfLine
        } else {
            InlineCommand::ClearLine
        }));
        if let Some(line) = current {
            steps.push(CommandStep::plain(InlineCommand::WriteLine(line.clone())));
        }
    }

    steps.push(CommandStep::plain(InlineCommand::RestoreOrigin));
    if let Some(cursor) = cursor {
        if cursor.row == 0 {
            steps.push(CommandStep::plain(InlineCommand::MoveRight(cursor.column)));
        } else {
            steps.push(CommandStep::plain(InlineCommand::MoveDown(cursor.row)));
            steps.push(CommandStep::plain(InlineCommand::MoveToColumn(
                cursor.column,
            )));
        }
        steps.push(CommandStep::plain(InlineCommand::ShowCursor));
    }

    InlineRenderPlan {
        claimed_rows: Some(rows_to_touch),
        steps,
        next: InlinePresentation {
            origin_saved: true,
            reserved_rows,
            previous_rows: lines.len() as u16,
            previous_lines: lines,
        },
    }
}

/// Plan the commands that release the owned region.
///
/// A submitted prompt keeps its final rows and moves the terminal below them;
/// every other outcome erases the rows it claimed and returns to the origin.
pub(crate) fn plan_finish(
    outcome: RenderFinish,
    previous: &InlinePresentation,
) -> InlineRenderPlan {
    let mut steps = Vec::new();
    let mut next = InlinePresentation {
        origin_saved: previous.origin_saved,
        reserved_rows: previous.reserved_rows,
        previous_rows: previous.previous_rows,
        // Cloned rather than borrowed: finish runs once per prompt session, so
        // the copy is not on any redraw path.
        previous_lines: previous.previous_lines.clone(),
    };

    match outcome {
        RenderFinish::Submitted => {
            if previous.origin_saved {
                steps.push(CommandStep::plain(InlineCommand::RestoreOrigin));
                if previous.previous_rows > 1 {
                    steps.push(CommandStep::plain(InlineCommand::MoveDown(
                        previous.previous_rows - 1,
                    )));
                }
                steps.push(CommandStep::plain(InlineCommand::CarriageReturnNewline));
                steps.push(CommandStep::plain(InlineCommand::ShowCursor));
            }
        }
        RenderFinish::Cancelled | RenderFinish::Error | RenderFinish::Panicking => {
            steps.extend(plan_clear_owned_rows(previous));
            next.previous_lines.clear();
            if previous.origin_saved {
                steps.push(CommandStep::plain(InlineCommand::RestoreOrigin));
            }
        }
    }

    InlineRenderPlan {
        claimed_rows: None,
        steps,
        next,
    }
}

fn plan_clear_owned_rows(previous: &InlinePresentation) -> Vec<CommandStep> {
    let rows = previous.previous_rows;
    if !previous.origin_saved || rows == 0 {
        return Vec::new();
    }

    let mut steps = vec![CommandStep::plain(InlineCommand::RestoreOrigin)];
    for row in 0..rows {
        if row == 0 {
            steps.push(CommandStep::plain(InlineCommand::ClearToEndOfLine));
        } else {
            steps.push(CommandStep::plain(InlineCommand::MoveToColumn(0)));
            steps.push(CommandStep::plain(InlineCommand::ClearLine));
        }
        if row + 1 < rows {
            steps.push(CommandStep::plain(InlineCommand::MoveDown(1)));
        }
    }
    steps.push(CommandStep::plain(InlineCommand::RestoreOrigin));
    steps
}
