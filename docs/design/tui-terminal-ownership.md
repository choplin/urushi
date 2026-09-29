# TUI Rendering and Terminal Ownership

What each name in the application framework's rendering vocabulary —
`Renderer`, `Frame`, `Screen`, `TerminalSession`, `CommandWriter`, `Clock` —
owns and does not own, the Rust shape each has, the commit guarantee the screen
gives, and what a session can and cannot promise to restore.
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

`urushi-tui` owns `Screen`, `Frame`, `Rect`, its cell representation, and the
working and committed buffers. It writes changed cells through
`urushi_terminal::CommandWriter`; replacing the command backend changes no
frame type and no application. `urushi-tui-app` owns the application runtime,
renderer, input sources, clock, and session orchestration above that
synchronous layer. The full package boundary is defined in
[`tui-crate-boundaries.md`](tui-crate-boundaries.md).

### Screen

```rust
impl<W: CommandWriter> Screen<W> {
    fn size(&self) -> TerminalSize;
    fn resize(&mut self, size: TerminalSize) -> io::Result<()>;
    fn draw(&mut self, draw: impl FnOnce(&mut Frame<'_>)) -> io::Result<()>;
}
```

`Screen` owns working and committed presentation state, cell diffing,
output, and flushing. `draw` lends a frame over the working state, diffs it
against the committed state, writes the difference and the cursor request, and
flushes; only when every step has succeeded does the working state become the
committed state. A failed output leaves the committed state as it was but marks
the physical surface unknown, because an arbitrary prefix may have arrived.
The next `draw` clears that surface and redraws its complete working state from
a blank baseline before it may commit. What the runtime does with the failure
itself — deliver it to an application that subscribed to terminal errors, or
end the run — is defined in
[`tui-runtime-entry.md`](tui-runtime-entry.md).

Each buffer cell is empty, the start of one measured grapheme, or a continuation
owned by that start. Writing a grapheme updates its complete width without
leaving an independently drawable continuation. Diffing treats the two
terminal-default blank representations as visibly equal and emits only owners;
shrinking, removing, or moving a wide grapheme still clears every cell it
previously painted. These states and results are equivalent to Noctui's buffer
and diff model.

`Screen` does not resize itself at draw time. Its size changes only through
`resize`, which the runtime calls when a surface observation has been applied
through the `Sync` barrier of
[`tui-delivery-ordering.md`](tui-delivery-ordering.md), so the frame the
renderer draws into and the size the model reflects are the same size.

Cursor position and visibility requested for one frame belong to the frame and
terminal path. They are not application effects. Where drawing a frame changes
terminal state — hiding the cursor for a frame that requests none — restoring
it is `Screen`'s or `TerminalSession`'s obligation, never the application's.

### Frame

```rust
impl Frame<'_> {
    fn area(&self) -> Rect;
    fn put(&mut self, column: usize, row: usize, cell: &StyledGrapheme);
    fn set_cursor(&mut self, at: Option<Position>);
}
```

A `Frame` is a borrowed, draw-scoped handle to the screen's working
presentation state: its area, the cells, and the cursor request for that draw.
`put` places one styled grapheme at a cell and claims the cells its width
covers. The frame does not own the previous buffer, the diff, the output
stream, or the flush; it cannot commit. It exposes no foreign buffer type.

### Renderer

The renderer is a runtime-internal operation, not a public type. Given the
application's `View`, a `Frame`, and the runtime's selected evaluator, it
resolves the view once against the frame's area. The ordinary evaluator calls
the free stateless `resolve`; an explicitly selected retained evaluator borrows
the runtime-owned `Resolver`. The renderer owns neither evaluator's lifetime.
It then writes every grapheme of the resulting `ResolvedView` through `put` and
turns the view's cursor anchor into the frame's cursor request: the anchor's
exact origin when its reported zero-sized point remains visible after every
cell-clipping stage, and no cursor otherwise. It consumes that accumulated
result and does not recompute visibility from containment in the root
rectangle.

That is the whole rendering operation. It does not own the model, navigation
policy, scheduling, a `Screen`, or session restoration, and it does not fill
any other anchored rectangle. Evaluator ownership and the equivalence between
the direct and retained paths are defined in
[`resolution-reuse.md`](resolution-reuse.md).

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

### Command output

`Screen<W: CommandWriter>` owns Urushi's working and committed `Buffer` values,
calls the Urushi diff operation, and lowers changed positions, graphemes,
resolved terminal styles, cursor state, clear, and flush to backend-independent
commands. It coalesces adjacent positions and repeated styles before emitting
them. The optional Crossterm adapter only serializes those primitive commands;
it does not know about cells, buffers, frame history, or diffing.

### Backend capability model

`urushi-terminal` is a terminal abstraction, not a list of operations extracted
from its current callers and not a renamed copy of Crossterm. Its contracts
cover four semantic capability groups:

- observation: cell dimensions, optional pixel geometry, cursor position,
  text and graphics rendering capabilities, raw-mode state, and
  enhanced-keyboard support;
- output: validated printable text, complete physical text style and hyperlink
  state, cursor movement and appearance, clear and scroll regions, screen and
  wrapping modes, synchronized updates, title, resize, validated APC and DCS
  extension payloads, and flush;
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
decoding, resize observation, and terminal protocol replies around that writer.
Its capability query uses positive Kitty, primary-device-attributes, and
XTGETTCAP color replies and caches the result; it does not infer support from
process environment variables. It is the production backend used by prompts.
The optional `backend::crossterm` module remains the cross-platform adapter. No
Crossterm type appears in an Urushi contract or a surface API.

The boundary excludes whole capability groups rather than individual values:
asynchronous scheduling and stream ownership belong to the runtime source
layer, clipboard transfer is a separate service, and graphics presentation is
the extension described below. Transporting a validated control string does
not make the terminal foundation own the image, protocol encoder, or graphics
lifecycle; it keeps physical output behind `CommandWriter`. Serialization
helper features and arbitrary backend-library commands are not terminal
capabilities. These exclusions do not justify dropping information inside the
synchronous terminal groups that the boundary does own.

### Clock

`Clock` is the runtime's one source of time — what `Subscription::interval`
and `Effect::after` read, and what they wait on. It belongs to
`urushi-tui-app`, not the terminal foundation: time is an execution boundary
rather than a terminal contract. It is backed by Tokio's time by default and
by a clock the test advances by hand in the harness. An application never
reads it directly.

### Replaceable in tests

The test harness of [`tui-delivery-ordering.md`](tui-delivery-ordering.md)
replaces the runtime-internal asynchronous presentation coordinator to observe
submitted views and draw completion without a physical terminal. `urushi-tui`
tests `Screen` separately over a recording `CommandWriter`, including buffer
diffs and failures at clear, changed-cell output, cursor, and flush. A harness
that needed a physical adapter to test delivery ordering would bind the
application tests to an unrelated implementation detail.

### Cell output and terminal graphics

Cell output plus terminal graphics remains an extension boundary. The separate
`urushi-graphics` crate uses resolved core anchors and emits validated APC or
DCS payloads through `urushi-terminal::CommandWriter`; it does not open a
backend or write a physical stream directly. This stateless command path does
not give the cell-only runtime a graphics lifecycle. A runtime that retains
terminal graphics owns the corresponding `urushi-graphics` state behind an
optional feature and must design asset lifetime, cell-and-graphics commit,
partial-output recovery, and fallback together before making graphics part of
its committed frame state. Applications that do not enable that integration do
not pay for or manage graphics state. The package boundary is defined in
[`terminal-graphics-boundary.md`](terminal-graphics-boundary.md).

## Verification

| Guarantee | Required evidence |
| --- | --- |
| Screen failures | Tests at draw, cursor, clear, and flush proving committed state advances only after successful output and failed frames are retried |
| Session restoration | Tests for normal exit, each partial setup failure, multiple cleanup failures, and panic unwinding |

## Why `Screen` is Urushi's concrete frame engine

Two rules above are rules about what the screen layer does: that a frame is
committed only after output succeeds, and that the frame's size changes only
through the `Sync` barrier. Ratatui's terminal does
neither: it autoresizes at the start of every draw, and it swaps its buffers
before it flushes, so a failed flush leaves it believing the frame was shown.
Wrapping it would preserve a foreign frame engine behind an abstraction after
Urushi already needs different commit semantics. `Screen` therefore owns the
buffer and diff directly: it writes, flushes, and commits last.

`Screen` is concrete rather than a public `Terminal` or `FrameTarget` trait.
The target design has one frame engine, and the external substitution points
are the physical `CommandWriter` and the runtime-internal presentation
coordinator used in scheduling tests. Publishing a trait solely for the
removed Ratatui implementation would make the obsolete implementation choice
part of the permanent API.

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
off until an application asks. Those choices form the `urushi-tui-app`
full-screen profile rather than a generic-library default. `urushi-prompt`
independently selects raw mode and bracketed paste while retaining the primary
screen.

Signals are subscriptions because that is how every other source is declared
and because a handler the runtime installed unasked would collide with one the
application installed itself. `Ctrl-C` needs no signal under raw mode, and the
size change already has its source.

## Why `Renderer` is not a public type

Its work is fixed by the view model and `Frame` — resolve once, put every
grapheme, place the cursor — and nothing varies by backend. A public type would
invite a replacement for which there is no reason; the name stays in the
vocabulary as the owner of that work.

## Open representation choices

- retained graphics commit and recovery details beyond the anchored placement
  contract.
