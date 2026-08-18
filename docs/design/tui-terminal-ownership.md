# TUI Rendering and Terminal Ownership

What each name in the runtime's rendering vocabulary — `Renderer`, `Frame`,
`Terminal`, `TerminalSession`, `Backend` — owns and does not own, the commit
guarantee the terminal gives, and what a session can and cannot promise to
restore. [`tui-architecture.md`](../tui-architecture.md) summarizes the
ownership under "Rendering and runtime ownership"; this file holds the exact
boundaries.

## The rule

### Renderer

A `Renderer` translates a TUI `View` into drawing operations during
`Terminal.draw`. It resolves the view once against the frame's area, writes the
resulting `ResolvedView` into the frame's cell buffer, and then serves the
placements that resolution reported: the cursor request, and any Ratatui widget
an application placed at a sized anchor. It reuses the cell-writing path of the
existing Urushi Ratatui adapters rather than drawing through `ViewWidget`, which
resolves internally and keeps nothing but the cells; why is recorded in
[`tui-view.md`](tui-view.md). It does not own the model, scheduling, a backend,
or session restoration.

### Frame

A `Frame` is a borrowed, draw-scoped handle to terminal-owned working
presentation state. At minimum, it provides access to the current cell buffer
and the cursor request for that draw. It does not own the previous buffer,
backend, diff algorithm, output stream, or flush operation.

The final API may use `ratatui::Frame` directly or place an Urushi-owned adapter
around it. Either choice must preserve this borrowed ownership model.

### Terminal

`Terminal` owns or delegates ownership of working and committed presentation
state, the backend, cell diffing, output, and flushing. It treats a
presentation as committed only after output succeeds. Ratatui does not give
this guarantee — it swaps its buffers before the backend flush, so a failed
flush leaves it believing the frame was shown — so `Terminal` keeps the local
recovery state that restores it: the last committed presentation is retained,
and a failed output leaves the next draw to redraw against it rather than
against the frame that never reached the terminal.

Cursor position and visibility requested for one frame belong to the frame and
terminal path. They are not application effects. Where drawing a frame changes
terminal state — hiding the cursor for a frame that requests none — restoring
it is `Terminal`'s or `TerminalSession`'s obligation, never the application's.

### TerminalSession

`TerminalSession` owns restoration obligations caused by entering the TUI
session, including raw mode, alternate-screen state, and cursor visibility when
the session changes it. It restores the state it is responsible for on normal
exit and on supported error and interruption paths.

The session does not promise to reconstruct exact pre-session state that the
terminal cannot report reliably, such as an unknown original cursor position.
That limitation must remain explicit in the public contract.

### Backend

`Backend` is the physical terminal-output boundary. Backend-specific types and
failure rules stay behind adapters so applications remain expressed in
Urushi-owned concepts. Clock, message source, and backend boundaries must be
replaceable in tests.

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

## Open representation choices

- the concrete `Renderer` type;
- the choice between Ratatui's `Frame` and a narrow Urushi adapter; and
- the graphics presentation and recovery model.
