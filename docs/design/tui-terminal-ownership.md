# TUI Rendering and Terminal Ownership

What each name in the runtime's rendering vocabulary — `Renderer`, `Frame`,
`Terminal`, `TerminalSession`, `Backend`, `Clock` — owns and does not own, the
Rust shape each has, the commit guarantee the terminal gives, and what a
session can and cannot promise to restore.
[`tui-architecture.md`](../tui-architecture.md) summarizes the ownership under
"Rendering and runtime ownership"; this file holds the exact boundaries.

## The rule

The terminal layer is Urushi's. `Terminal`, `Frame`, `TerminalSession`, and
`Clock` are traits this crate owns, stated in Urushi's vocabulary — sizes,
rectangles, styled graphemes — and the runtime core, the application, and the
renderer see nothing else. A backend such as Ratatui is one implementation of
those traits, kept inside its own module; replacing it changes no runtime type
and no application. [`architecture.md`](../architecture.md) records why
replacement is the direction rather than a contingency.

### Terminal

```rust
pub trait Terminal {
    type Frame<'a>: Frame where Self: 'a;

    fn size(&self) -> Size;
    fn resize(&mut self, size: Size) -> io::Result<()>;
    fn draw(&mut self, draw: impl FnOnce(&mut Self::Frame<'_>)) -> io::Result<()>;
}
```

`Terminal` owns working and committed presentation state, cell diffing,
output, and flushing. `draw` lends a frame over the working state, diffs it
against the committed state, writes the difference and the cursor request, and
flushes; only when every step has succeeded does the working state become the
committed state. A failed output leaves the committed state as it was, so the
next `draw` diffs against what the terminal actually shows and redraws what the
failed frame would have changed. What the runtime does with the failure itself
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
    fn area(&self) -> Rect;
    fn put(&mut self, x: u16, y: u16, grapheme: &StyledGrapheme);
    fn set_cursor(&mut self, at: Option<Position>);
}
```

A `Frame` is a borrowed, draw-scoped handle to the terminal's working
presentation state: its area, the cells, and the cursor request for that
draw. `put` places one styled grapheme at a cell and claims the cells its
width covers. It does not own the previous buffer, the diff, the output stream,
or the flush; it cannot commit. A backend's frame may expose its own cell
buffer beside this trait for a caller that holds the backend's types — the
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

`TerminalSession` owns the restoration obligations that entering the TUI
session creates. Entering, by default, enables raw mode, the alternate screen,
bracketed paste, focus-change reporting, and keyboard enhancement where the
terminal reports support for it, and hides the cursor until a frame requests
one; mouse capture is off. Each is an option on the entry point's builder, so
an application opts out of one it does not want and into mouse capture when it
needs it. Input that these modes produce — a paste as one `Input::Paste`, a
focus change as `Input::Focus`, a key release when an application asks for
releases — reaches `update` through `Subscription::input` like any key.

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

### Backend

`Backend` is the physical terminal-output boundary, and it belongs to the
backend implementation, not to the runtime vocabulary. The Ratatui-backed
terminal is `RatatuiTerminal<B: ratatui::backend::Backend>`: it borrows
Ratatui's `Buffer`, its cell diff, and its backends — `CrosstermBackend` for
the real terminal, `TestBackend` in memory — and implements `Terminal` over
them. An application meets it only as the default behind `run(app)` or as the
value it passes to the builder's `terminal`.

### Clock

`Clock` is the runtime's one source of time — what `Subscription::interval`
and any timer read, and what they wait on. It is a trait the runtime owns,
backed by Tokio's time by default and by a clock the test advances by hand in
the harness. An application never reads it directly.

### Replaceable in tests

The test harness of [`tui-delivery-ordering.md`](tui-delivery-ordering.md)
replaces the terminal layer at two levels. The runtime core — delivery,
scheduling, barriers — runs against an in-memory `Terminal` the runtime owns,
which records every committed frame and cursor request and depends on no
backend. The Ratatui-backed terminal is tested on its own, over
`TestBackend`. A harness that needed the backend to test the runtime would
bind the runtime's tests to the backend the runtime is built to outgrow.

### Cell output and terminal graphics

Cell output plus terminal graphics remains an extension boundary. The first
implementation should prove the cell-only runtime before promoting a shared
graphics contract. When that work begins, it must design asset lifetime,
cell-and-graphics commit, partial-output recovery, and text-only fallback
together.

## Verification

| Guarantee | Required evidence |
| --- | --- |
| Terminal failures | Tests proving committed state advances only after successful output |
| Session restoration | Integration tests for normal exit, error, interruption, and partial setup failure |

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

That the trait is Urushi's rather than the backend's follows from
[`architecture.md`](../architecture.md): the runtime core should see no
backend type, so that the backend can be replaced without touching the
runtime or any application. A trait at the granularity of "place this styled
grapheme here" is the level Urushi already speaks — `StyledGrapheme` is what
`ResolvedView` holds — so the renderer is written once, against the trait, and
each backend converts one grapheme at a time.

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

## Why the session's defaults are what they are

Raw mode and the alternate screen are what a full-screen application is.
Bracketed paste turns a paste into one input instead of a stream of keys whose
newlines would act as Enter; focus reporting costs nothing an application did
not ask to see; keyboard enhancement makes Escape and modifier combinations
unambiguous where the terminal supports it and is probed first so an
unsupporting terminal is unchanged. Mouse capture is the one default that
takes something from the user — the terminal's own text selection — so it is
off until an application asks.

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
