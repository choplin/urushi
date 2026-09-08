//! Crossterm implementations of prompt input, rendering, and terminal control.

use std::{
    io::{self, IsTerminal, Write},
    time::Duration,
};

use crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event as CrosstermEvent,
        KeyCode as CrosstermKeyCode, KeyEventKind,
    },
    execute,
    terminal::{self},
};

use super::{
    crossterm_executor,
    form::PromptStart,
    frame,
    inline_plan::{self, InlineRenderPlan},
    presentation::InlinePresentation,
    resolve,
    terminal::{
        Event, EventSource, KeyCode, KeyEvent, KeyModifiers, RenderFinish, Renderer,
        TerminalControl,
    },
    view::PromptView,
};

pub(super) struct CrosstermEventSource;

pub(super) fn terminal_size() -> io::Result<(u16, u16)> {
    terminal::size()
}

impl EventSource for CrosstermEventSource {
    fn read_event(&mut self) -> io::Result<Event> {
        loop {
            if let Some(event) = translate_event(event::read()?) {
                return Ok(event);
            }
        }
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        while event::poll(Duration::ZERO)? {
            if let Some(event) = translate_event(event::read()?) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }
}

/// The prompt's own event, for the crossterm events it has a use for.
fn translate_event(event: CrosstermEvent) -> Option<Event> {
    match event {
        CrosstermEvent::Key(key)
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
        {
            translate_key(key.code, key.modifiers).map(Event::Key)
        }
        CrosstermEvent::Resize(columns, rows) => Some(Event::Resize { columns, rows }),
        CrosstermEvent::Paste(text) => Some(Event::Paste(text)),
        _ => None,
    }
}

fn translate_key(
    code: CrosstermKeyCode,
    modifiers: crossterm::event::KeyModifiers,
) -> Option<KeyEvent> {
    let code = match code {
        CrosstermKeyCode::Char(character) => KeyCode::Char(character),
        CrosstermKeyCode::Enter => KeyCode::Enter,
        CrosstermKeyCode::Esc => KeyCode::Escape,
        CrosstermKeyCode::Tab => KeyCode::Tab,
        CrosstermKeyCode::BackTab => KeyCode::BackTab,
        CrosstermKeyCode::Backspace => KeyCode::Backspace,
        CrosstermKeyCode::Delete => KeyCode::Delete,
        CrosstermKeyCode::Left => KeyCode::Left,
        CrosstermKeyCode::Right => KeyCode::Right,
        CrosstermKeyCode::Up => KeyCode::Up,
        CrosstermKeyCode::Down => KeyCode::Down,
        CrosstermKeyCode::Home => KeyCode::Home,
        CrosstermKeyCode::End => KeyCode::End,
        _ => return None,
    };
    Some(KeyEvent {
        code,
        modifiers: KeyModifiers {
            shift: modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
            control: modifiers.contains(crossterm::event::KeyModifiers::CONTROL),
            alt: modifiers.contains(crossterm::event::KeyModifiers::ALT),
        },
    })
}

pub(super) struct CrosstermRenderer<W> {
    pub(super) writer: W,
    pub(super) presentation: InlinePresentation,
    pub(super) columns: u16,
    pub(super) rows: u16,
}

impl CrosstermRenderer<io::Stderr> {
    pub(super) fn stderr(size: (u16, u16)) -> Self {
        Self::new(io::stderr(), size)
    }
}

impl<W: Write> CrosstermRenderer<W> {
    pub(super) fn new(writer: W, size: (u16, u16)) -> Self {
        Self {
            writer,
            presentation: InlinePresentation::default(),
            columns: size.0.max(1),
            rows: size.1.max(1),
        }
    }

    fn present(&mut self, plan: InlineRenderPlan) -> io::Result<()> {
        crossterm_executor::execute(&mut self.writer, &mut self.presentation, plan)
    }

    #[cfg(test)]
    pub(super) fn draw(&mut self, view: &PromptView) -> io::Result<()> {
        <Self as Renderer>::draw(
            self,
            view,
            PromptStart::CurrentPosition { column: 0 },
            self.columns,
        )
    }
}

impl<W: Write> Renderer for CrosstermRenderer<W> {
    fn draw(
        &mut self,
        view: &PromptView,
        start: PromptStart,
        drawing_columns: u16,
    ) -> io::Result<()> {
        let resolved = resolve::resolve_prompt(drawing_columns, view);
        let framed = frame::frame(&resolved, self.rows);
        let plan = inline_plan::plan_draw(
            framed,
            &self.presentation,
            start,
            drawing_columns,
            self.rows,
        );
        self.present(plan)
    }

    fn columns(&self) -> u16 {
        self.columns
    }

    fn finish(&mut self, outcome: RenderFinish) -> io::Result<()> {
        let plan = inline_plan::plan_finish(outcome, &self.presentation);
        self.present(plan)
    }

    fn resize(&mut self, columns: u16, rows: u16) {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
        // A resize may reflow existing content and push rows past the top of
        // the screen, which invalidates both the saved origin and the row
        // count. Neither can be recovered by inspection, so the region is
        // abandoned; the next frame re-establishes on the cursor's own row.
        self.presentation.lose_region();
    }
}
pub(super) struct CrosstermTerminalControl;

impl TerminalControl for CrosstermTerminalControl {
    fn is_interactive(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()
    }

    fn enable_bracketed_paste(&mut self) -> io::Result<()> {
        execute!(io::stderr(), EnableBracketedPaste)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(io::stderr(), cursor::Show)
    }

    fn disable_bracketed_paste(&mut self) -> io::Result<()> {
        execute!(io::stderr(), DisableBracketedPaste)
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        Form, FormState, Group, LineKind, PromptLine, PromptStyles, PromptView, ReducerResult,
        ViewCursor, ViewSpan,
        inline_plan::{InlineCommand, RenderState, step},
        terminal::tests::*,
        test_styles,
        view::tests::*,
    };
    use crate::{Confirm, FieldKey, Input, Select, SelectOption};
    use urushi::{AnsiPolicy, ColorProfile, TerminalProfile};
    /// The plan the renderer would execute for `view`, without writing it.
    fn draw_plan<W>(renderer: &CrosstermRenderer<W>, view: &PromptView) -> InlineRenderPlan {
        draw_plan_at(
            renderer,
            PromptStart::CurrentPosition { column: 0 },
            renderer.columns,
            view,
        )
    }

    fn draw_plan_at<W>(
        renderer: &CrosstermRenderer<W>,
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
    /// Fails the `nth` bare line feed, which lands the failure inside the
    /// window where a frame is re-anchoring its origin.
    struct FailOnLineFeedWriter {
        bytes: Vec<u8>,
        line_feeds: usize,
        fail_at: usize,
    }

    impl FailOnLineFeedWriter {
        fn new(nth: usize) -> Self {
            Self {
                bytes: Vec::new(),
                line_feeds: 0,
                fail_at: nth,
            }
        }
    }

    impl io::Write for FailOnLineFeedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes == b"\n" {
                self.line_feeds += 1;
                if self.line_feeds == self.fail_at {
                    return Err(io::Error::other("planned line feed failure"));
                }
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct PrefixThenFailWriter {
        bytes: Vec<u8>,
        trigger: Vec<u8>,
        failed: bool,
        fail_next_write: bool,
    }

    impl PrefixThenFailWriter {
        fn new(trigger: &str) -> Self {
            Self {
                bytes: Vec::new(),
                trigger: trigger.as_bytes().to_vec(),
                failed: false,
                fail_next_write: false,
            }
        }
    }

    impl io::Write for PrefixThenFailWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_next_write {
                self.fail_next_write = false;
                return Err(io::Error::other("planned partial draw failure"));
            }
            if !self.failed && bytes == self.trigger {
                self.failed = true;
                self.fail_next_write = true;
                self.bytes.push(bytes[0]);
                return Ok(1);
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn inline_renderer_uses_resolved_theme_styles_and_anchors_on_the_current_row() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![PromptLine::spans(vec![
                ViewSpan::new("質問", &styles.question),
                ViewSpan::new("＊", &styles.cursor),
            ])],
            Some(ViewCursor { row: 0, column: 2 }),
        );

        // A current-position start anchors on the row the cursor already
        // occupies. Clearing begins at that origin and leaves its left side
        // untouched.
        let plan = draw_plan(&renderer, &view);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::HideCursor,
                InlineCommand::SavePosition,
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::Write(plan.next.rows[0].clone()),
                InlineCommand::RestorePosition,
                InlineCommand::MoveRight(2),
                InlineCommand::ShowCursor,
            ]
        );

        renderer.draw(&view).expect("renderer writes to a buffer");

        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(output.contains("\x1b[1m質問\x1b[0m"));
        assert!(output.contains("\x1b[4m＊\x1b[0m"));
        assert!(output.contains("\x1b[K"));
        assert!(!output.contains("\x1b[2K"));
    }

    #[test]
    fn a_new_line_prompt_comes_out_to_a_fresh_row_first() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![view_line("question", &styles.question)],
            Some(ViewCursor { row: 0, column: 2 }),
        );

        // NewLine always reaches a fresh row before anchoring it, without
        // querying where the cursor happened to be.
        let plan = draw_plan_at(&renderer, PromptStart::NewLine, renderer.columns, &view);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::HideCursor,
                InlineCommand::CarriageReturnLineFeed,
                InlineCommand::SavePosition,
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::Write(plan.next.rows[0].clone()),
                InlineCommand::RestorePosition,
                InlineCommand::MoveRight(2),
                InlineCommand::ShowCursor,
            ]
        );

        // The row the carriage return and line feed reached is the region top,
        // so the fold counts one reserved row rather than the two it passed
        // through.
        let anchored = fold(&renderer.presentation, &plan.commands[..3]);
        assert!(anchored.anchored);
        assert_eq!(anchored.reserved_rows, 1);
        assert_eq!(plan.next.reserved_rows, 1);

        <CrosstermRenderer<_> as Renderer>::draw(&mut renderer, &view, PromptStart::NewLine, 20)
            .expect("renderer writes to a buffer");
        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(output.starts_with("\x1b[?25l\r\n"), "{output:?}");
    }

    #[test]
    fn start_column_and_width_change_the_wrap_bound_before_resolve() {
        let styles = test_styles();
        let view = renderer_view(
            vec![view_line(
                "a question whose words wrap at the supplied boundary",
                &styles.question,
            )],
            None,
        );
        let group = || {
            Group::builder()
                .field(TestField::new("field", "value"))
                .build()
                .expect("test group has a field")
        };
        let full = Form::builder()
            .group(group())
            .build()
            .expect("full-width form");
        let positioned = Form::builder()
            .start(PromptStart::CurrentPosition { column: 12 })
            .group(group())
            .build()
            .expect("positioned form");
        let limited = Form::builder()
            .width(8)
            .group(group())
            .build()
            .expect("width-limited form");
        let oversized = Form::builder()
            .width(99)
            .group(group())
            .build()
            .expect("width-capped form");

        let row_count = |form: &Form| {
            let width = form.drawing_width(20);
            resolve::resolve_prompt(width, &view).lines[0]
                .view
                .rows()
                .len()
        };
        assert_eq!(positioned.drawing_width(20), 8);
        assert!(row_count(&positioned) > row_count(&full));
        assert_eq!(limited.drawing_width(20), 8);
        assert_eq!(row_count(&limited), row_count(&positioned));
        assert_eq!(oversized.drawing_width(20), 20);
        assert_eq!(row_count(&oversized), row_count(&full));
    }

    /// Every plan the inline renderer can produce, named for failure messages.
    ///
    /// The canonical-list invariants below hold for the vocabulary as a whole,
    /// not for one frame, so they are asserted over the whole set rather than
    /// re-derived per test.
    #[test]
    fn changing_a_selection_preserves_unrelated_rendered_rows() {
        let mut form = Form::builder()
            .group(
                Group::builder()
                    .title("Settings")
                    .description("Review the values.")
                    .field(Input::new(FieldKey::new("name"), "Name", "value").expect("input"))
                    .field(
                        Select::new(
                            FieldKey::new("choice"),
                            "Choice",
                            vec![SelectOption::new("One", 1), SelectOption::new("Two", 2)],
                        )
                        .expect("select"),
                    )
                    .field(
                        Confirm::new(FieldKey::new("confirm"), "Continue?", Some(true))
                            .expect("confirm"),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let mut state = FormState::Running { group: 0, field: 0 };
        assert_eq!(form.reduce(&mut state, enter()), ReducerResult::Running);
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let renderer = CrosstermRenderer::new(Vec::new(), (80, 10));
        let before = lay_out(
            renderer.columns,
            renderer.rows,
            &form.view(&state, &styles, renderer.columns),
        );
        assert_eq!(
            form.reduce(
                &mut state,
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::default(),
                })
            ),
            ReducerResult::Running
        );
        let after = lay_out(
            renderer.columns,
            renderer.rows,
            &form.view(&state, &styles, renderer.columns),
        );
        let unchanged = before
            .rows
            .iter()
            .zip(&after.rows)
            .filter(|(before, after)| before == after)
            .count();
        assert!(
            unchanged >= 8,
            "only selection rows should change: {unchanged}"
        );
    }

    #[test]
    fn a_failure_while_the_origin_is_being_anchored_abandons_the_region() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(FailOnLineFeedWriter::new(2), (20, 4));

        assert!(
            renderer
                .draw(&renderer_view(
                    vec![
                        view_line("first", &styles.question),
                        view_line("second", &styles.option),
                        view_line("third", &styles.help).with_kind(LineKind::Help),
                    ],
                    None,
                ))
                .is_err()
        );

        // The failure landed between the first line feed and the SaveOrigin
        // that would have ended the window, so no origin is saved and the rows
        // the first line feed scrolled in cannot be located again. The region
        // is abandoned rather than erased at a guessed position.
        assert!(!renderer.presentation.anchored);
        assert_eq!(renderer.presentation.reserved_rows, 0);
        assert_eq!(renderer.presentation.owned_rows, 0);
        assert!(renderer.presentation.rows.is_empty());
        // Nothing was written, so there is no residue either: cleanup emits
        // nothing at all, not even the closing line feed that a lost region
        // with content on screen would need.
        assert!(!renderer.presentation.drawn);
        for outcome in [
            RenderFinish::Submitted,
            RenderFinish::Cancelled,
            RenderFinish::Error,
            RenderFinish::Panicking,
        ] {
            assert_eq!(
                inline_plan::plan_finish(outcome, &renderer.presentation).commands,
                [],
                "{outcome:?} cleanup after a region that was never anchored"
            );
        }

        renderer
            .finish(RenderFinish::Error)
            .expect("cleanup of an abandoned region writes nothing");
    }

    #[test]
    fn a_resize_abandons_the_region_and_cleanup_pushes_below_the_residue() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        renderer
            .draw(&renderer_view(
                vec![
                    view_line("first", &styles.question),
                    view_line("second", &styles.option),
                    view_line("third", &styles.help).with_kind(LineKind::Help),
                ],
                None,
            ))
            .expect("first draw succeeds");
        assert_eq!(renderer.presentation.owned_rows, 3);

        renderer.resize(30, 6);
        // A resize may reflow content and move everything the origin pointed
        // at, so the region goes with it. `drawn` is the one field that
        // survives, because the residue on screen is not undone by a resize.
        assert!(!renderer.presentation.anchored);
        assert_eq!(renderer.presentation.reserved_rows, 0);
        assert_eq!(renderer.presentation.owned_rows, 0);
        assert!(renderer.presentation.drawn);

        // Cancelling immediately afterwards has no rows to erase, yet the
        // residue is still on screen. Gating the closing line feed on
        // owned_rows would skip it and let the next output land on top of that
        // residue; the gate is drawn, which the loss did not reset.
        for outcome in [
            RenderFinish::Submitted,
            RenderFinish::Cancelled,
            RenderFinish::Error,
            RenderFinish::Panicking,
        ] {
            assert_eq!(
                inline_plan::plan_finish(outcome, &renderer.presentation).commands,
                [
                    InlineCommand::ShowCursor,
                    InlineCommand::CarriageReturnLineFeed,
                ],
                "{outcome:?} cleanup after a lost region"
            );
        }

        renderer
            .finish(RenderFinish::Cancelled)
            .expect("cleanup after a resize succeeds");
    }

    #[test]
    fn a_redraw_after_a_resize_re_establishes_on_the_cursor_row() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let tall = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.help).with_kind(LineKind::Help),
            ],
            None,
        );
        renderer.draw(&tall).expect("first draw succeeds");
        renderer.resize(20, 4);

        // A prompt that is still running re-establishes on the cursor's own
        // row, overwriting it. It does not push below the residue first: that
        // is what a finishing prompt does, and doing it here would add a blank
        // row on every resize.
        let plan = draw_plan(&renderer, &tall);
        assert_eq!(
            plan.commands[..5],
            [
                InlineCommand::HideCursor,
                InlineCommand::MoveToColumn(0),
                InlineCommand::LineFeed,
                InlineCommand::LineFeed,
                InlineCommand::MoveUp(2),
            ]
        );
        assert_eq!(plan.commands[5], InlineCommand::SavePosition);
        assert!(
            !plan
                .commands
                .contains(&InlineCommand::CarriageReturnLineFeed),
            "a redraw never releases the terminal below the region"
        );
        // The old origin is gone, so nothing restores to it before the frame
        // has saved a new one.
        assert!(
            !plan.commands[..5].contains(&InlineCommand::RestorePosition),
            "a lost region has no origin to return to"
        );

        // A single-row prompt overwrites the whole of what it had, so it leaves
        // no residue at all: the re-established region is exactly the cursor's
        // row.
        renderer.resize(20, 4);
        let short = draw_plan(
            &renderer,
            &renderer_view(vec![view_line("only", &styles.question)], None),
        );
        assert_eq!(
            short.commands[..3],
            [
                InlineCommand::HideCursor,
                InlineCommand::MoveToColumn(0),
                InlineCommand::SavePosition,
            ]
        );

        renderer
            .draw(&tall)
            .expect("redraw after a resize succeeds");
        assert!(renderer.presentation.anchored);
        assert_eq!(renderer.presentation.owned_rows, 3);
    }

    #[test]
    fn error_cleanup_clears_the_rows_a_partial_first_draw_touched() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let writer = PrefixThenFailWriter::new("first");
        let mut renderer = CrosstermRenderer::new(writer, (20, 4));
        let view = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.error).with_kind(LineKind::Error),
            ],
            None,
        );

        assert!(renderer.draw(&view).is_err());
        // Three rows were scrolled into existence, but the write that failed
        // had reached only the first of them. The fold owns exactly that row:
        // its ClearLine succeeded, so the bytes the partial write left
        // there are covered, while the two rows below are still the blank ones
        // the line feeds scrolled in and there is nothing on them to erase.
        assert_eq!(renderer.presentation.reserved_rows, 3);
        assert_eq!(renderer.presentation.owned_rows, 1);
        assert_eq!(
            inline_plan::plan_finish(RenderFinish::Error, &renderer.presentation).commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::RestorePosition,
                InlineCommand::RestorePosition,
            ]
        );

        renderer
            .finish(RenderFinish::Error)
            .expect("error cleanup succeeds after one draw failure");

        let output =
            String::from_utf8(renderer.writer.bytes).expect("renderer writes UTF-8 commands");
        assert!(output.contains('f'));
    }

    #[test]
    fn error_cleanup_after_partial_growth_owns_only_the_rows_the_frame_reached() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(PrefixThenFailWriter::new("growth"), (20, 4));
        renderer
            .draw(&renderer_view(
                vec![view_line("short", &styles.question)],
                None,
            ))
            .expect("initial draw succeeds");
        assert_eq!(renderer.presentation.owned_rows, 1);

        let growth = renderer_view(
            vec![
                view_line("growth", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.error).with_kind(LineKind::Error),
            ],
            None,
        );
        assert!(renderer.draw(&growth).is_err());
        // The region grew from one row to three before the failing write, and
        // reserved_rows records that. What cleanup must erase is narrower: the
        // rows below the first were scrolled in blank and never written.
        assert_eq!(renderer.presentation.reserved_rows, 3);
        assert_eq!(renderer.presentation.owned_rows, 1);
        assert_eq!(
            inline_plan::plan_finish(RenderFinish::Error, &renderer.presentation).commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::RestorePosition,
                InlineCommand::RestorePosition,
            ]
        );

        renderer
            .finish(RenderFinish::Error)
            .expect("growth cleanup succeeds after one draw failure");
    }

    #[test]
    fn inline_renderer_clears_stale_rows_and_clamps_cjk_cursor_in_narrow_viewports() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (4, 2));
        renderer
            .draw(&renderer_view(
                vec![
                    PromptLine::spans(vec![
                        ViewSpan::new("名前 ", &styles.question),
                        ViewSpan::new("あいうえ", &styles.answer),
                    ]),
                    view_line("validation message", &styles.error).with_kind(LineKind::Error),
                ],
                Some(ViewCursor { row: 0, column: 11 }),
            ))
            .expect("narrow draw succeeds");
        let narrow = renderer_view(
            vec![view_line("名前 あいうえ", &styles.answer)],
            Some(ViewCursor { row: 0, column: 11 }),
        );
        let layout = lay_out(renderer.columns, renderer.rows, &narrow);
        assert!(layout.rows.len() <= 2);
        // Bounding the cursor to the terminal box is the plan stage's job: it
        // is the stage the geometry is an input to, and the frame stage
        // chooses rows and has no horizontal concern at all.
        let plan = draw_plan(&renderer, &narrow);
        assert!(!plan.commands.iter().any(|command| matches!(
            command,
            InlineCommand::MoveRight(column) | InlineCommand::MoveToColumn(column)
                if *column >= renderer.columns
        )));

        renderer.resize(3, 1);
        renderer
            .draw(&renderer_view(
                vec![view_line("短い", &styles.question)],
                None,
            ))
            .expect("redraw succeeds");
        assert_eq!(renderer.presentation.owned_rows, 1);
        // A shrunk viewport leaves the region one row tall, so cancelling
        // erases exactly that row — never the screen.
        assert_eq!(
            inline_plan::plan_finish(RenderFinish::Cancelled, &renderer.presentation).commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::RestorePosition,
                InlineCommand::RestorePosition,
            ]
        );

        renderer
            .finish(RenderFinish::Cancelled)
            .expect("cancel cleanup succeeds");
    }

    #[test]
    fn inline_renderer_applies_terminal_profile_before_writing_styles() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 2));
        renderer
            .draw(&renderer_view(
                vec![view_line("plain", &styles.question)],
                None,
            ))
            .expect("draw succeeds");
        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(!output.contains("\x1b[1m"));
        assert!(output.contains("plain"));
    }
}
