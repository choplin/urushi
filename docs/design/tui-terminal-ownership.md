# TUI Rendering and Terminal Ownership

What each name in the runtime's rendering vocabulary — `Renderer`, `Frame`,
`Terminal`, `TerminalSession`, `CellWriter`, `Clock` — owns and does not own, the
Rust shape each has, the commit guarantee the terminal gives, and what a
session can and cannot promise to restore.
[`tui-architecture.md`](../tui-architecture.md) summarizes the ownership under
"Rendering and runtime ownership"; this file holds the exact boundaries.

## The rule

The terminal primitive layer is Urushi's. The workspace-independent
`urushi-terminal` crate owns shared style and geometry primitives, commands,
events, queries, raw-mode control, the `TerminalSession` restoration guard,
and terminal geometry used by those operations. Core `urushi` re-exports its
color, attribute, and underline primitives and uses them inside `TextStyle`;
surfaces do not maintain equivalent style types. The terminal crate owns no
frame, cell buffer, diff, or presentation transaction.
`urushi-tui::terminal` owns `Terminal`, `Frame`, `CellWriter`, and `Rect`, and
binds a frame's associated cell to `StyledGrapheme`. Its `RatatuiTerminal` uses
Ratatui for the working buffer and cell diff, then sends changed cells through
a `CellWriter` that lowers them to `urushi_terminal::Command` values. Replacing
the command backend changes no runtime type and no application.

### Terminal

```rust
pub trait Terminal {
    type Cell: ?Sized;
    type Frame<'a>: Frame<Cell = Self::Cell>;

    fn size(&self) -> TerminalSize;
    fn resize(&mut self, size: TerminalSize) -> io::Result<()>;
    fn draw(&mut self, draw: impl FnOnce(&mut Self::Frame<'_>)) -> io::Result<()>;
}
```

`Terminal` owns working and committed presentation state, cell diffing,
output, and flushing. `draw` lends a frame over the working state, diffs it
against the committed state, writes the difference and the cursor request, and
flushes; only when every step has succeeded does the working state become the
committed state. A failed output leaves the committed state as it was but marks
the physical surface unknown, because an arbitrary prefix may have arrived.
The next `draw` clears that surface and redraws its complete working state from
a blank baseline before it may commit. What the runtime does with the failure itself
— deliver it to an application that subscribed to terminal errors, or end the
run — is defined in [`tui-runtime-entry.md`](tui-runtime-entry.md).

`Terminal` does not resize itself at draw time. Its size changes only through
`resize`, which the runtime calls when a surface observation has been applied
through the `Sync` barrier of
[`tui-delivery-ordering.md`](tui-delivery-ordering.md), so the frame the
renderer draws into and the size the model reflects are the same size.

Cursor position and visibility requested for one frame belong to the frame and
terminal path. They are not application effects. Where drawing a frame changes
terminal state — hiding the cursor for a frame that requests none — restoring
it is `Terminal`'s or `TerminalSession`'s obligation, never the application's.

### Frame

```rust
pub trait Frame {
    type Cell: ?Sized;

    fn area(&self) -> Rect;
    fn put(&mut self, column: usize, row: usize, cell: &Self::Cell);
    fn set_cursor(&mut self, at: Option<Position>);
}
```

A `Frame` is a borrowed, draw-scoped handle to the terminal's working
presentation state: its area, the cells, and the cursor request for that draw.
Its associated `Cell` keeps the terminal contract independent of the
presentation crate. The TUI runtime requires `Cell = StyledGrapheme`; `put`
therefore places one styled grapheme at a cell and claims the cells its width
covers. The frame does not own the previous buffer, the diff, the output
stream, or the flush; it cannot commit. A backend's frame may expose its own
cell buffer beside this trait for a caller that holds the backend's types — the
Ratatui-backed frame exposes its `Buffer` — and nothing in the runtime reaches
for it.

### Renderer

The renderer is a runtime-internal function, not a public type. Given the
application's `View` and a `Frame`, it resolves the view once against the
frame's area, writes every grapheme of the `ResolvedView` through `put`, and
turns the view's cursor anchor into the frame's cursor request: the anchor's
origin when the resolved view contains it, and no cursor when it does not.
That is the whole of it; it does not own the model, scheduling, a terminal, or
session restoration, and it does not fill any other anchored rectangle.

A sized anchor — a region layout places for something this crate does not
draw, a chart say — is served by the view model and the adapter, not by the
runtime: a caller that holds the buffer resolves the view, writes it, reads
the anchored rectangle, and draws there itself, as
[`tui-view.md`](tui-view.md) describes. The runtime's application holds no
buffer and has no such hook; a backend implementation may add one as an
extension of its own, and the runtime contract does not change when it does.

### TerminalSession

`TerminalSession` owns the restoration obligations that entering an
interactive terminal session creates. Its generic default acquires nothing;
the TUI entry point selects raw mode, the alternate screen, bracketed paste,
focus-change reporting, keyboard enhancement where supported, and a hidden
cursor, while leaving mouse capture off. Each is an option on the entry point's
builder, so an application opts out of one it does not want and into mouse
capture when it needs it. Input that these modes produce — a paste as one `Input::Paste`, a
focus change as `Input::Focus`, a key release when an application asks for
releases, and mouse input when capture is enabled — reaches `update` through
`Subscription::input`. `Input` classifies the terminal event stream for the
TEA runtime but re-exports and carries the same key, focus, modifier, and mouse
values; it does not copy that vocabulary.

The session restores the state it changed, in reverse order, on every exit
path the runtime controls: a shutdown the application requested, an `Err` the
entry point returns, and a panic unwinding through it. Restoration is written
twice: an explicit `restore` on the normal paths, and `Drop` as the guard that
runs during an unwind. When entering fails part-way — raw mode taken, the
alternate screen refused — the session restores what it had entered and
reports the failure; it never leaves a half-entered terminal behind.

A signal is a message source the application declares, not a path the session
handles: `Subscription::signal(Signal::Term, f)` installs a handler for that
signal and delivers it through `f`, and the application shuts down on it. An
undeclared signal installs no handler. `Ctrl-C` under raw mode is a key and
arrives through `Subscription::input`; the size change is the `surface`
source.

The session does not promise what the terminal cannot report or the process
cannot run: the cursor's position before the session, scrollback the
alternate screen did not protect, restoration after `panic = "abort"`, `kill
-9`, or a signal the application did not declare. That limitation is part of
the public contract.

### CellWriter

`CellWriter` is the low-level full-screen drawing SPI in
`urushi-tui::terminal`. `RatatuiTerminal<W: CellWriter>` owns the working and
committed Ratatui `Buffer` values and calls `Buffer::diff`, then passes changed
positions, graphemes, resolved terminal styles, cursor state, clear, and flush
to `W`. The blanket implementation for `CommandWriter` coalesces adjacent
positions and repeated styles before emitting backend-independent commands.
The optional Crossterm adapter only serializes those primitive commands; it
does not know about cells, Ratatui buffers, frame history, or diffing.

### Backend capability model

`urushi-terminal` is a terminal abstraction, not a list of operations extracted
from its current callers and not a renamed copy of Crossterm. Its contracts
cover four semantic capability groups:

- observation: cell dimensions, optional pixel geometry, cursor position,
  rendering capabilities, raw-mode state, and enhanced-keyboard support;
- output: validated printable text, complete physical text style and hyperlink
  state, cursor movement and appearance, clear and scroll regions, screen and
  wrapping modes, synchronized updates, title, resize, and flush;
- input: keys, mouse, focus, paste, and resize, including timed polling and the
  enhanced key kind, state, modifier, media-key, and modifier-key information a
  backend observes;
- session ownership: explicit acquisition options and reverse-order restoration
  for terminal modes selected by a caller.

These are Urushi values and invariants. Cursor displacement, for example, is
one signed semantic operation even if an adapter lowers it to separate up/down
and left/right commands. Underline shape and color form one value instead of
mirroring SGR enable/disable commands. Input adapters do not discard an event
because no current Urushi control uses it. Printable text excludes control
characters, so ANSI and other backend instructions cannot enter through a text
payload; only an adapter serializes commands and styles.

Backend implementations live below `urushi_terminal::backend`. One complete
interactive backend owns both directions of a physical connection, its parser,
process-side modes, and queries. `TerminalQuery` therefore takes mutable access:
a query may write a protocol request and consume a response without racing the
ordinary event reader. `backend::ansi::AnsiWriter` implements the output
protocol without a terminal framework. On Unix,
`backend::native::NativeTerminal` owns `/dev/tty`, raw-mode restoration, input
decoding, resize observation, and terminal protocol replies around that writer;
it is the production backend used by prompts. The optional
`backend::crossterm` module remains the cross-platform adapter. No Crossterm
type appears in an Urushi contract or a surface API.

The boundary excludes whole capability groups rather than individual values:
asynchronous scheduling and stream ownership belong to the runtime source
layer, clipboard transfer is a separate service, and graphics presentation is
the extension described below. Serialization helper features and arbitrary
backend-library commands are not terminal capabilities. These exclusions do
not justify dropping information inside the synchronous text-terminal groups
that the boundary does own.

### Clock

`Clock` is the runtime's one source of time — what `Subscription::interval`
and `Effect::after` read, and what they wait on. It belongs to `urushi-tui`, not
the terminal foundation: time is an execution boundary rather than a terminal
contract. It is backed by Tokio's time by default and by a clock the test
advances by hand in the harness. An application never reads it directly.

### Replaceable in tests

The test harness of [`tui-delivery-ordering.md`](tui-delivery-ordering.md)
replaces the terminal layer at two levels. The runtime core — delivery,
scheduling, barriers — runs against an in-memory implementation of the
`urushi_tui::terminal::Terminal` contract, which records every committed frame
and cursor request and depends on no backend. The Ratatui-backed terminal is
tested on its own over a recording `CellWriter`. A harness that needed a physical
adapter to test the runtime would bind the runtime's tests to an implementation
detail the runtime is built to outgrow.

### Cell output and terminal graphics

Cell output plus terminal graphics remains an extension boundary. The first
implementation should prove the cell-only runtime before promoting a shared
graphics contract. When that work begins, it must design asset lifetime,
cell-and-graphics commit, partial-output recovery, and text-only fallback
together.

## Verification

| Guarantee | Required evidence |
| --- | --- |
| Terminal failures | Tests at draw, cursor, clear, and flush proving committed state advances only after successful output and failed frames are retried |
| Session restoration | Tests for normal exit, each partial setup failure, multiple cleanup failures, and panic unwinding |

## Why the terminal is Urushi's trait and not an adapted one

Two rules above are rules about what the terminal layer does: that a frame is
committed only after output succeeds, and that the frame's size changes only
through the `Sync` barrier. An adapted terminal — the one Ratatui ships — does
neither: it autoresizes at the start of every draw, and it swaps its buffers
before it flushes, so a failed flush leaves it believing the frame was shown.
Wrapping it would mean pinning its viewport to stop the first and keeping a
"redraw everything next time" flag to paper over the second, and both would be
workarounds for a contract the wrapper does not hold. A terminal of Urushi's
own over a backend's draw-and-flush holds the contract directly: it diffs, it
writes, it flushes, and it commits last, in about the code the workarounds
would have cost.

That the trait is in `urushi-tui::terminal` rather than a backend module follows
from [`architecture.md`](../architecture.md): the runtime core should see no
backend type, but a presentation transaction is still a TUI concern rather
than a generic terminal primitive. The associated cell type expresses "place
this cell value here" while the TUI binds it to `StyledGrapheme`, the value a
`ResolvedView` holds. The renderer is written once against that binding, and
the cell writer converts changed graphemes into commands.

Rejected: wrapping `ratatui::Terminal` behind the trait anyway. Possible, but
the contract violations above remain under the wrapper, and the wrapper is
larger than the terminal it would hide.

## Why the runtime does not embed foreign widgets

An application that owns its loop holds a buffer, resolves an Urushi view
into it, reads an anchored rectangle, and draws a backend widget there — the
view model's sized anchor exists for exactly that caller. An application on
the runtime holds no buffer; a hook that handed it one would hand it the
backend's types, which is the one dependency the runtime exists to keep out of
applications, and would bind the runtime's contract to a backend the runtime
is built to replace. So the runtime serves the cursor anchor, which needs no
backend type, and leaves sized anchors to the caller that has a buffer. A
backend implementation may offer an embedding hook as its own extension; the
runtime contract is the same with or without it.

## Why session profiles belong to their callers

`SessionOptions::default()` acquires no mode. Raw mode and the alternate screen
are what a full-screen application normally selects.
Bracketed paste turns a paste into one input instead of a stream of keys whose
newlines would act as Enter; focus reporting costs nothing an application did
not ask to see; keyboard enhancement makes Escape and modifier combinations
unambiguous where the terminal supports it and is probed first so an
unsupporting terminal is unchanged. Mouse capture is the one default that
takes something from the user — the terminal's own text selection — so it is
off until an application asks. Those choices form the `urushi-tui` full-screen
profile rather than a generic-library default. `urushi-prompt` independently
selects raw mode and bracketed paste while retaining the primary screen.

Signals are subscriptions because that is how every other source is declared
and because a handler the runtime installed unasked would collide with one the
application installed itself. `Ctrl-C` needs no signal under raw mode, and the
size change already has its source.

## Why `Renderer` is not a public type

Its work is fixed by the view model and the frame trait — resolve once, put
every grapheme, place the cursor — and nothing varies by backend, since the
trait absorbs the variation. A public type would invite a replacement for
which there is no reason; the name stays in the vocabulary as the owner of that
work.

## Open representation choices

- the graphics presentation and recovery model.
