//! Terminal commands planned as data, before anything is written.
//!
//! The prompt owns a region of rows anchored at the position it saved on its
//! first draw. Planning that region's updates as a value — rather than emitting
//! them straight into a writer — lets the command order and the owned-region
//! state transitions be tested without a terminal, and keeps the crossterm
//! executor free of layout and diffing.
//!
//! # The region's left edge
//!
//! The caller chooses the region's left edge and drawing width. Every row is
//! handled the same way — position, clear, write — with no first-row special
//! case. The prompt owns each row from that left edge to the row's end; see
//! [`PromptStart`] and `docs/design/prompt-region.md`.
//!
//! # Recovery contract
//!
//! The plan carries no per-command state. Recovery is derived instead, by
//! folding [`step`] over the commands:
//!
//! - **On success**, the executor adopts [`InlineRenderPlan::next`]. Only the
//!   planner knows that a completed frame collapses `owned_rows` to the new
//!   height, so the planner declares it rather than deriving it.
//! - **On failure** at command *k*, the state is `fold(previous, &commands[..k])`,
//!   which is exactly what reached the terminal.
//!
//! `step` maintains `anchored`, `reserved_rows`, `owned_rows`, `drawn`, and the
//! cursor's row within the region. It does not maintain the drawn rows: after a
//! failure their content is indeterminate and cleanup erases the region anyway.
//!
//! ## Anchoring, and the window where recovery cannot reach
//!
//! The saved origin is an absolute screen position, so a line feed at the
//! bottom of the screen invalidates it: the region scrolls up while the saved
//! position stays. A frame that grows the region therefore emits every line
//! feed *first*, returns to the region top, and only then saves the origin, so
//! no saved origin is ever invalidated by a later command in the same frame.
//! A first frame saves no origin before reserving its rows; it has nothing to
//! return to.
//!
//! Between the first line feed of such a sequence and the [`InlineCommand::SavePosition`]
//! that ends it the region is unanchored, and a failure there leaves its extent
//! unknown. So does a terminal resize. In both the region is *lost*: it is
//! abandoned rather than erased, because erasing from an origin that no longer
//! locates the region would destroy output the prompt does not own. See
//! [`InlinePresentation::lose_region`] and `docs/design/prompt-region.md`.
//!
//! ## Clear before write
//!
//! A [`InlineCommand::Write`] that fails partway does not reach the fold,
//! so it does not raise `owned_rows` — yet its bytes may already be on screen.
//! That row is covered regardless, because every row is positioned, *cleared*,
//! and only then written, and the clear that succeeded has already raised
//! `owned_rows` past that row.
//!
//! > Every row is cleared before it is written, in the same frame.
//!
//! The accuracy of recovery rests on that invariant, not on `step` alone.
//! Dropping the clear for rows believed to be empty, or for content believed to
//! be appended rather than replaced, is a plausible optimization that would
//! silently leave residue behind a failed write. Any change to the uniform row
//! form must preserve the invariant or replace it with something that covers a
//! partially completed write.

use super::{
    PromptStart, RenderFinish,
    frame::{FramedRow, FramedView},
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
    /// Anchor the owned region at the current cursor position, with DEC Save
    /// Cursor.
    SavePosition,
    /// Return to the anchored origin, with DEC Restore Cursor.
    RestorePosition,
    MoveUp(u16),
    MoveDown(u16),
    MoveRight(u16),
    MoveToColumn(u16),
    /// Erase from the region's left edge to the end of the row. This erases the
    /// whole row when the left edge is column zero. It is the only erase in the
    /// vocabulary, and every row is cleared the same way.
    ClearLine,
    /// A bare line feed, which scrolls a new row into existence at the bottom.
    LineFeed,
    /// A real carriage return plus line feed. Unlike relative cursor movement,
    /// this scrolls at the terminal boundary instead of stopping there.
    CarriageReturnLineFeed,
    /// Write one framed row, styled by the executor.
    Write(FramedRow),
}

/// What a fold over a command list maintains.
///
/// The cursor's row within the region is frame-local rather than presentation
/// state: every plan begins with the region either unestablished — the cursor
/// is at the region top by definition — or repositioned by a `RestorePosition`
/// before any command that depends on position. That precondition is what makes
/// [`step`] total.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct RenderState {
    pub presentation: InlinePresentation,
    /// Rows below the origin, as the commands so far have left the cursor.
    pub cursor_row: u16,
}

impl RenderState {
    pub(crate) fn resuming(presentation: InlinePresentation) -> Self {
        Self {
            presentation,
            cursor_row: 0,
        }
    }
}

/// The state one command leaves behind once it has been written.
///
/// Total: no input state and no command combination panics, and unknown cursor
/// arithmetic saturates rather than wrapping.
pub(crate) fn step(mut state: RenderState, command: &InlineCommand) -> RenderState {
    match command {
        InlineCommand::HideCursor
        | InlineCommand::ShowCursor
        | InlineCommand::MoveRight(_)
        | InlineCommand::MoveToColumn(_) => {}
        InlineCommand::SavePosition => {
            // Every plan saves the origin at the region top, so the row the
            // cursor is on becomes row zero and the region is at least one row
            // tall. The rows counted so far were counted from wherever the
            // fold started, which for a frame that reaches a fresh row is one
            // row above the new origin; only the rows at or below the cursor
            // are reserved by the region being anchored here.
            state.presentation.anchored = true;
            state.presentation.reserved_rows = state
                .presentation
                .reserved_rows
                .saturating_sub(state.cursor_row)
                .max(1);
            state.cursor_row = 0;
        }
        InlineCommand::RestorePosition => state.cursor_row = 0,
        InlineCommand::MoveUp(rows) => state.cursor_row = state.cursor_row.saturating_sub(*rows),
        InlineCommand::MoveDown(rows) => state.cursor_row = state.cursor_row.saturating_add(*rows),
        InlineCommand::ClearLine => {
            state.presentation.owned_rows =
                touched(state.presentation.owned_rows, state.cursor_row);
        }
        InlineCommand::LineFeed | InlineCommand::CarriageReturnLineFeed => {
            // A line feed scrolls a row into existence below the cursor, so the
            // region has grown to hold it.
            state.cursor_row = state.cursor_row.saturating_add(1);
            state.presentation.reserved_rows =
                touched(state.presentation.reserved_rows, state.cursor_row);
            // At the bottom of the screen the same line feed scrolls the
            // display instead, moving the region up while the saved origin
            // stays put. Which of the two happened is not observable from
            // here, so the origin is treated as stale until the frame saves it
            // again. This is what makes `anchored` false for exactly the
            // growth window, and it is the condition region loss keys on.
            state.presentation.anchored = false;
        }
        InlineCommand::Write(_) => {
            state.presentation.owned_rows =
                touched(state.presentation.owned_rows, state.cursor_row);
            state.presentation.drawn = true;
        }
    }
    state
}

/// The high-water mark of a row count that must cover `row`.
fn touched(rows: u16, row: u16) -> u16 {
    rows.max(row.saturating_add(1))
}

pub(crate) struct InlineRenderPlan {
    pub commands: Vec<InlineCommand>,
    /// The presentation once every command has been written.
    pub next: InlinePresentation,
}

/// Plan the commands that bring the owned region from `previous` to `view`.
///
/// `drawing_columns` is the effective drawing width selected above Resolve;
/// `max_rows` is the terminal height. The plan never claims more rows than the
/// latter and never moves the cursor past the drawing width. The horizontal
/// bound belongs here rather than in the frame stage, which chooses rows and
/// has no horizontal concern.
///
/// `start` chooses how the first frame reaches its left edge. A region already
/// anchored restores its saved origin. A region re-established after a loss
/// returns to the selected column without adding another row, bounding residue
/// to the rows above it.
pub(crate) fn plan_draw(
    view: FramedView,
    previous: &InlinePresentation,
    start: PromptStart,
    drawing_columns: u16,
    max_rows: u16,
) -> InlineRenderPlan {
    let FramedView {
        rows: lines,
        cursor,
    } = view;
    let rows_to_touch = previous
        .rows
        .len()
        .max(lines.len())
        .min(usize::from(max_rows)) as u16;

    let mut commands = vec![InlineCommand::HideCursor];

    // An unanchored region has no extent to build on: whatever it once
    // reserved was abandoned with it, and there is no origin to restore to.
    let mut reserved_rows = if previous.anchored {
        previous.reserved_rows
    } else {
        0
    };
    // Rows that already exist from the region top down. The cursor is standing
    // on a row whichever state the region is in, and that row costs no line
    // feed; an unanchored region has nothing beyond it.
    let existing_rows = reserved_rows.max(1);

    let wanted_rows = (lines.len() as u16).max(1);
    if wanted_rows > reserved_rows {
        if previous.anchored {
            commands.push(InlineCommand::RestorePosition);
            if existing_rows > 1 {
                commands.push(InlineCommand::MoveDown(existing_rows - 1));
            }
        } else if previous.drawn {
            // Re-establishing after a loss. The cursor's own row becomes the
            // new region top: starting there overwrites it, so the residue left
            // behind is bounded to the rows above it, and a single-row prompt
            // leaves none. A line feed first would push the region below the
            // residue instead, which is what a finishing prompt does and not
            // what a redrawing one does.
            commands.push(InlineCommand::MoveToColumn(start.column()));
        } else {
            match start {
                PromptStart::NewLine => {
                    commands.push(InlineCommand::CarriageReturnLineFeed);
                }
                PromptStart::CurrentLine => {
                    commands.push(InlineCommand::MoveToColumn(0));
                }
                PromptStart::CurrentPosition { .. } => {}
            }
        }
        // Every line feed is emitted before the origin is saved, so all the
        // scrolling a frame can cause has already happened by the time the
        // anchor is taken. A bare line feed preserves the column, so the
        // MoveUp lands back where the sequence started.
        for _ in existing_rows..wanted_rows {
            commands.push(InlineCommand::LineFeed);
        }
        if wanted_rows > 1 {
            commands.push(InlineCommand::MoveUp(wanted_rows - 1));
        }
        commands.push(InlineCommand::SavePosition);
        reserved_rows = wanted_rows;
    }

    let mut drawn = previous.drawn;
    for row in 0..usize::from(rows_to_touch) {
        let current = lines.get(row);
        if previous.rows.get(row) == current {
            continue;
        }

        // Every row takes the same form: position, clear, write. Restore lands
        // at the selected left edge and a bare MoveDown keeps that column, so
        // no row needs a column command and none is a special case.
        commands.push(InlineCommand::RestorePosition);
        if row > 0 {
            commands.push(InlineCommand::MoveDown(
                row.min(usize::from(u16::MAX)) as u16
            ));
        }
        // Positioned, cleared, then written: the clear-before-write invariant
        // this module's recovery contract rests on.
        commands.push(InlineCommand::ClearLine);
        if let Some(line) = current {
            commands.push(InlineCommand::Write(line.clone()));
            drawn = true;
        }
    }

    commands.push(InlineCommand::RestorePosition);
    if let Some(mut cursor) = cursor {
        cursor.column = cursor.column.min(drawing_columns.max(1) - 1);
        if cursor.row == 0 {
            // A movement of zero distance is never emitted: the restore has
            // already put the cursor on the origin's row at its left edge.
            if cursor.column > 0 {
                commands.push(InlineCommand::MoveRight(cursor.column));
            }
        } else {
            commands.push(InlineCommand::MoveDown(cursor.row));
            if cursor.column > 0 {
                commands.push(InlineCommand::MoveRight(cursor.column));
            }
        }
        commands.push(InlineCommand::ShowCursor);
    }

    InlineRenderPlan {
        commands,
        next: InlinePresentation {
            anchored: true,
            reserved_rows,
            owned_rows: lines.len() as u16,
            rows: lines,
            drawn,
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
    let mut commands = Vec::new();
    // Cloned rather than borrowed: finish runs once per prompt session, so the
    // copy is not on any redraw path.
    let mut next = previous.clone();

    if !previous.anchored {
        // The region was lost, or was never established. Its extent cannot be
        // located, so nothing is erased and there is nothing to return to. All
        // that is left is to push below the residue, and the gate for that is
        // `drawn`, not `owned_rows`: a loss resets the row count while the
        // residue stays on screen, and gating on the count would let later
        // output land on top of it. A prompt that never drew anything must not
        // leave a blank row behind either, which is the other half of the same
        // gate.
        if previous.drawn {
            commands.push(InlineCommand::ShowCursor);
            commands.push(InlineCommand::CarriageReturnLineFeed);
        }
        next.rows.clear();
        return InlineRenderPlan { commands, next };
    }

    // Past here the region is anchored, so its extent is known and the origin
    // can be returned to.
    match outcome {
        RenderFinish::Submitted => {
            commands.push(InlineCommand::RestorePosition);
            if previous.owned_rows > 1 {
                commands.push(InlineCommand::MoveDown(previous.owned_rows - 1));
            }
            commands.push(InlineCommand::CarriageReturnLineFeed);
            commands.push(InlineCommand::ShowCursor);
        }
        RenderFinish::Cancelled | RenderFinish::Error | RenderFinish::Panicking => {
            commands.extend(plan_clear_owned_rows(previous));
            next.rows.clear();
            commands.push(InlineCommand::RestorePosition);
        }
    }

    InlineRenderPlan { commands, next }
}

/// Erase the rows the region is known to own, from an anchored origin.
///
/// The caller has already established that the region is anchored: an
/// unanchored one is abandoned rather than erased, because erasing from an
/// origin that no longer locates the region would destroy output the prompt
/// does not own.
fn plan_clear_owned_rows(previous: &InlinePresentation) -> Vec<InlineCommand> {
    let rows = previous.owned_rows;
    if rows == 0 {
        return Vec::new();
    }

    // A clear leaves the cursor where it is and a bare MoveDown keeps the
    // column, so the walk stays at the selected left edge and every row is
    // erased the same way.
    let mut commands = vec![InlineCommand::RestorePosition];
    for row in 0..rows {
        commands.push(InlineCommand::ClearLine);
        if row + 1 < rows {
            commands.push(InlineCommand::MoveDown(1));
        }
    }
    commands.push(InlineCommand::RestorePosition);
    commands
}
