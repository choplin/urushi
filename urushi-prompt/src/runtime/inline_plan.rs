//! Terminal commands planned as data, before anything is written.
//!
//! The prompt owns a region of rows anchored at the position it saved on its
//! first draw. Planning that region's updates as a value — rather than emitting
//! them straight into a writer — lets the command order and the owned-region
//! state transitions be tested without a terminal, and keeps the command
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
//! unknown. A terminal resize also invalidates the region, but the event loop
//! either exits or clears the viewport and resets presentation state before
//! planning another frame. Neither path asks this plan to recover an
//! unlocatable origin. See [`InlinePresentation::lose_region`] and
//! `docs/design/prompt-region.md`.
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
/// Deliberately not a backend command re-export: this vocabulary captures the
/// prompt's recovery semantics and is the thing kept
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
/// anchored restores its saved origin. An unlocatable region is never passed
/// back for redraw: resize handling exits or resets it after a viewport clear.
pub(crate) fn plan_draw(
    view: FramedView,
    previous: &InlinePresentation,
    start: PromptStart,
    drawing_columns: u16,
    max_rows: u16,
) -> InlineRenderPlan {
    debug_assert!(
        previous.anchored || !previous.drawn,
        "an unlocatable inline region must not be redrawn"
    );
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

#[cfg(test)]
mod tests {
    use super as inline_plan;
    use super::*;
    use crate::runtime::{
        LineKind, PromptStyles, PromptView, ViewCursor, terminal::Renderer,
        terminal_backend::TerminalRenderer, test_styles, view::tests::*,
    };
    use urushi::RenderSettings;
    /// The plan the renderer would execute for `view`, without writing it.
    fn draw_plan<W>(renderer: &TerminalRenderer<W>, view: &PromptView) -> InlineRenderPlan {
        draw_plan_at(
            renderer,
            PromptStart::CurrentPosition { column: 0 },
            renderer.columns,
            view,
        )
    }

    fn draw_plan_at<W>(
        renderer: &TerminalRenderer<W>,
        start: PromptStart,
        drawing_columns: u16,
        view: &PromptView,
    ) -> InlineRenderPlan {
        let framed = lay_out(drawing_columns, renderer.rows, view);
        inline_plan::plan_draw(
            framed,
            &renderer.presentation,
            start,
            drawing_columns,
            renderer.rows,
        )
    }

    /// The state a partially written plan leaves behind: the fold of the
    /// commands that succeeded. Testing recovery this way needs no failure
    /// injection.
    fn fold(previous: &InlinePresentation, commands: &[InlineCommand]) -> InlinePresentation {
        commands
            .iter()
            .fold(RenderState::resuming(previous.clone()), step)
            .presentation
    }

    /// The rows a `columns` x `rows` terminal box shows of `view`.
    #[test]
    fn every_prompt_start_has_an_explicit_first_frame_plan() {
        let styles = test_styles();
        let renderer = TerminalRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(vec![view_line("question", &styles.question)], None);

        let cases = [
            (
                PromptStart::NewLine,
                Some(InlineCommand::CarriageReturnLineFeed),
            ),
            (
                PromptStart::CurrentLine,
                Some(InlineCommand::MoveToColumn(0)),
            ),
            (PromptStart::CurrentPosition { column: 4 }, None),
        ];
        for (start, initial) in cases {
            let columns = 20_u16.saturating_sub(start.column()).max(1);
            let plan = draw_plan_at(&renderer, start, columns, &view);
            let mut expected = vec![InlineCommand::HideCursor];
            expected.extend(initial);
            expected.extend([
                InlineCommand::SavePosition,
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::Write(plan.next.rows[0].clone()),
                InlineCommand::RestorePosition,
            ]);
            assert_eq!(plan.commands, expected, "{start:?}");
        }
    }

    fn every_plan_shape() -> Vec<(&'static str, Vec<InlineCommand>)> {
        let theme = test_theme();
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        let one_row = |cursor| renderer_view(vec![view_line("only", &styles.question)], cursor);
        let three_rows = |cursor| {
            renderer_view(
                vec![
                    view_line("first", &styles.question),
                    view_line("second", &styles.answer),
                    view_line("third", &styles.help).with_kind(LineKind::Help),
                ],
                cursor,
            )
        };

        let mut plans = Vec::new();

        let mut renderer = TerminalRenderer::new(Vec::new(), (20, 4));
        plans.push((
            "first frame, cursor at the origin",
            draw_plan(&renderer, &one_row(Some(ViewCursor { row: 0, column: 0 }))).commands,
        ));
        plans.push((
            "first frame, three rows",
            draw_plan(
                &renderer,
                &three_rows(Some(ViewCursor { row: 2, column: 4 })),
            )
            .commands,
        ));

        plans.push((
            "first frame, new line",
            draw_plan_at(
                &renderer,
                PromptStart::NewLine,
                renderer.columns,
                &one_row(Some(ViewCursor { row: 0, column: 3 })),
            )
            .commands,
        ));
        plans.push((
            "first frame, current line",
            draw_plan_at(
                &renderer,
                PromptStart::CurrentLine,
                renderer.columns,
                &one_row(Some(ViewCursor { row: 0, column: 3 })),
            )
            .commands,
        ));
        plans.push((
            "first frame, current position",
            draw_plan_at(
                &renderer,
                PromptStart::CurrentPosition { column: 4 },
                renderer.columns - 4,
                &one_row(Some(ViewCursor { row: 0, column: 3 })),
            )
            .commands,
        ));

        renderer
            .draw(&one_row(Some(ViewCursor { row: 0, column: 1 })))
            .expect("first frame renders");
        plans.push((
            "growth from one row to three",
            draw_plan(
                &renderer,
                &three_rows(Some(ViewCursor { row: 1, column: 2 })),
            )
            .commands,
        ));

        renderer
            .draw(&three_rows(Some(ViewCursor { row: 1, column: 2 })))
            .expect("grown frame renders");
        plans.push((
            "redraw of a changed middle row",
            draw_plan(
                &renderer,
                &renderer_view(
                    vec![
                        view_line("first", &styles.question),
                        view_line("changed", &styles.answer),
                        view_line("third", &styles.help).with_kind(LineKind::Help),
                    ],
                    Some(ViewCursor { row: 2, column: 0 }),
                ),
            )
            .commands,
        ));
        for outcome in [
            RenderFinish::Submitted,
            RenderFinish::Cancelled,
            RenderFinish::Error,
        ] {
            plans.push((
                match outcome {
                    RenderFinish::Submitted => "finish: submitted",
                    RenderFinish::Cancelled => "finish: cancelled",
                    _ => "finish: error",
                },
                inline_plan::plan_finish(outcome, &renderer.presentation).commands,
            ));
        }

        renderer.resize(20, 4);
        renderer
            .clear_viewport()
            .expect("viewport reset before replacement frame");
        plans.push((
            "first frame after a viewport clear",
            draw_plan(
                &renderer,
                &three_rows(Some(ViewCursor { row: 0, column: 0 })),
            )
            .commands,
        ));

        plans
    }

    #[test]
    fn no_plan_emits_a_movement_of_zero_distance() {
        for (name, commands) in every_plan_shape() {
            for command in &commands {
                let zero = matches!(
                    command,
                    InlineCommand::MoveUp(0)
                        | InlineCommand::MoveDown(0)
                        | InlineCommand::MoveRight(0)
                );
                assert!(!zero, "{name}: {commands:?}");
            }
        }
    }

    #[test]
    fn every_write_is_positioned_and_cleared_the_same_way() {
        let mut writes = 0;
        for (name, commands) in every_plan_shape() {
            for (index, command) in commands.iter().enumerate() {
                if !matches!(command, InlineCommand::Write(_)) {
                    continue;
                }
                writes += 1;
                // Position, clear, write — the same three steps for every row,
                // with no column command and no first-row special case. The
                // clear immediately before the write is what the recovery
                // contract rests on.
                let positioning = matches!(
                    commands[..index],
                    [.., InlineCommand::RestorePosition, InlineCommand::ClearLine]
                        | [
                            ..,
                            InlineCommand::RestorePosition,
                            InlineCommand::MoveDown(_),
                            InlineCommand::ClearLine,
                        ]
                );
                assert!(positioning, "{name}: row form at {index} in {commands:?}");
            }
        }
        assert!(writes >= 8, "the shapes must actually write rows: {writes}");
    }

    #[test]
    fn a_write_is_always_followed_by_an_absolute_reposition() {
        for (name, commands) in every_plan_shape() {
            let mut written = false;
            for command in &commands {
                match command {
                    InlineCommand::Write(_) => written = true,
                    // The cursor's position after a write that reaches the
                    // right margin is terminal-dependent, so nothing relative
                    // may follow one until an absolute command has removed the
                    // ambiguity.
                    InlineCommand::MoveUp(_)
                    | InlineCommand::MoveDown(_)
                    | InlineCommand::MoveRight(_) => {
                        assert!(
                            !written,
                            "{name}: relative move after a write in {commands:?}"
                        )
                    }
                    InlineCommand::RestorePosition
                    | InlineCommand::MoveToColumn(_)
                    | InlineCommand::CarriageReturnLineFeed => written = false,
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn first_draw_reserves_rows_before_saving_the_render_origin() {
        let theme = test_theme();
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        let mut renderer = TerminalRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.answer),
                view_line("third", &styles.help).with_kind(LineKind::Help),
            ],
            None,
        );

        // Two bare line feeds scroll the extra rows into existence, the cursor
        // returns to the top of them, and only then is the origin anchored. No
        // origin is saved before the line feeds: a first frame has nothing to
        // return to, and a position saved ahead of them would be the very one
        // a line feed at the bottom of the screen invalidates.
        let plan = draw_plan(&renderer, &view);
        assert_eq!(
            plan.commands[..4],
            [
                InlineCommand::HideCursor,
                InlineCommand::LineFeed,
                InlineCommand::LineFeed,
                InlineCommand::MoveUp(2),
            ]
        );
        assert_eq!(plan.commands[4], InlineCommand::SavePosition);
        assert_eq!(
            plan.commands
                .iter()
                .position(|command| *command == InlineCommand::SavePosition),
            Some(4),
            "the frame saves the origin once, after the rows are reserved"
        );
        // Mid-growth the region is unanchored: the line feeds may have
        // scrolled the display, and no saved position survives that.
        let growing = fold(&renderer.presentation, &plan.commands[..2]);
        assert!(!growing.anchored);
        // No command carries the anchor or the row count: folding the prefix
        // that ends at the closing SavePosition derives both. Nothing has been
        // written yet, so the region is reserved but not yet owned.
        let reserved = fold(&renderer.presentation, &plan.commands[..5]);
        assert!(reserved.anchored);
        assert_eq!(reserved.reserved_rows, 3);
        assert_eq!(reserved.owned_rows, 0);
        assert!(!reserved.drawn);

        renderer
            .draw(&view)
            .expect("renderer reserves and draws three rows");
        assert_eq!(renderer.presentation.reserved_rows, 3);
    }

    #[test]
    fn unchanged_frames_only_reposition_the_cursor() {
        let theme = test_theme();
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        let mut renderer = TerminalRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![view_line("stable", &styles.answer)],
            Some(ViewCursor { row: 0, column: 2 }),
        );
        renderer.draw(&view).expect("first frame renders");

        // Nothing is cleared and nothing is rewritten; the frame only puts the
        // cursor back.
        let plan = draw_plan(&renderer, &view);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::HideCursor,
                InlineCommand::RestorePosition,
                InlineCommand::MoveRight(2),
                InlineCommand::ShowCursor,
            ]
        );
        // A frame that only repositions leaves the region exactly as it was,
        // whichever command it failed on.
        for k in 0..=plan.commands.len() {
            assert_eq!(
                fold(&renderer.presentation, &plan.commands[..k]),
                renderer.presentation
            );
        }

        let first_frame_bytes = renderer.writer.writer().len();
        renderer.draw(&view).expect("unchanged frame renders");
        let update = String::from_utf8(renderer.writer.writer()[first_frame_bytes..].to_vec())
            .expect("renderer writes UTF-8 commands");
        assert!(!update.contains("stable"), "{update:?}");
    }

    #[test]
    fn submitted_prompt_finishes_with_a_scrolling_line_feed() {
        let theme = test_theme();
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        let mut renderer = TerminalRenderer::new(Vec::new(), (20, 2));
        renderer
            .draw(&renderer_view(
                vec![
                    view_line("answer", &styles.answer),
                    view_line("help", &styles.help).with_kind(LineKind::Help),
                ],
                None,
            ))
            .expect("prompt renders");

        // Step down to the last owned row, then release the region with a real
        // line feed. Relative movement would stop at the terminal boundary
        // instead of scrolling, so the final row must not be re-entered with a
        // MoveDown-style command.
        let plan = inline_plan::plan_finish(RenderFinish::Submitted, &renderer.presentation);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::MoveDown(1),
                InlineCommand::CarriageReturnLineFeed,
                InlineCommand::ShowCursor,
            ]
        );

        renderer
            .finish(RenderFinish::Submitted)
            .expect("submitted prompt finishes");
    }

    #[test]
    fn recovery_state_is_the_fold_of_the_commands_that_succeeded() {
        let theme = test_theme();
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        let renderer = TerminalRenderer::new(Vec::new(), (20, 4));
        let plan = draw_plan(
            &renderer,
            &renderer_view(
                vec![
                    view_line("first", &styles.question),
                    view_line("second", &styles.option),
                    view_line("third", &styles.help).with_kind(LineKind::Help),
                ],
                None,
            ),
        );

        // The state at any failure point is the fold of the prefix that
        // succeeded, so it can be asserted without injecting a failure. Rows
        // are reserved before anything is owned, and each row becomes owned by
        // the clear that precedes its write.
        let expected = [
            (0, false, 0, 0, false),
            (2, false, 2, 0, false),
            (5, true, 3, 0, false),
            (8, true, 3, 1, true),
            (12, true, 3, 2, true),
            (16, true, 3, 3, true),
            (plan.commands.len(), true, 3, 3, true),
        ];
        for (prefix, anchored, reserved_rows, owned_rows, drawn) in expected {
            let state = fold(&renderer.presentation, &plan.commands[..prefix]);
            assert_eq!(
                (
                    state.anchored,
                    state.reserved_rows,
                    state.owned_rows,
                    state.drawn
                ),
                (anchored, reserved_rows, owned_rows, drawn),
                "fold of the first {prefix} commands"
            );
        }

        // A completed frame collapses the owned extent to its own height, which
        // the commands do not carry; only the planner knows it.
        assert_eq!(plan.next.owned_rows, 3);
        assert!(plan.next.drawn);
    }

    #[test]
    fn every_row_is_cleared_before_it_is_written_in_the_same_frame() {
        let theme = test_theme();
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        let mut renderer = TerminalRenderer::new(Vec::new(), (20, 4));
        let tall = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.help).with_kind(LineKind::Help),
            ],
            Some(ViewCursor { row: 1, column: 3 }),
        );
        let short = renderer_view(vec![view_line("only", &styles.question)], None);

        // A write that fails partway never reaches the fold, so it cannot raise
        // owned_rows itself. It is covered because the clear on its own row
        // already did — the invariant the recovery contract rests on. Dropping
        // a clear for a row believed to be empty or merely appended to would
        // fail here rather than silently leaving residue behind a failed write.
        for view in [&tall, &short, &tall] {
            let plan = draw_plan(&renderer, view);
            let mut state = RenderState::resuming(renderer.presentation.clone());
            for command in &plan.commands {
                if matches!(command, InlineCommand::Write(_)) {
                    assert!(
                        state.presentation.owned_rows > state.cursor_row,
                        "row {} is written while cleanup owns only {} rows",
                        state.cursor_row,
                        state.presentation.owned_rows
                    );
                }
                state = step(state, command);
            }
            renderer.draw(view).expect("renderer writes to a buffer");
        }
    }
}
