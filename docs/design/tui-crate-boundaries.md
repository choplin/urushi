# TUI Crate Boundaries

This document answers how Urushi separates full-screen frame presentation, the
TEA application framework, physical terminal access, and optional Ratatui
integration.

## Decision

The four responsibilities have four package owners:

| Crate | Owns | Does not own |
| --- | --- | --- |
| `urushi-terminal` | Commands, events, queries, raw-mode control, session restoration, and one physical interactive terminal connection | Views, frames, cell buffers, diffing, or application scheduling |
| `urushi-tui` | A synchronous `Screen`, draw-scoped `Frame`, cell buffers, wide-grapheme ownership, diffing, transactional output, and failed-output recovery | Application state, an event loop, effects, subscriptions, terminal input, session entry, or a foreign buffer representation |
| `urushi-tui-app` | `Application`, `Runtime`, effects, subscriptions, delivery ordering, draw scheduling, view resolution, terminal input, session ownership, and optional cell-plus-graphics orchestration | Cell-buffer semantics, graphics encoding, physical terminal protocols, or foreign widget types |
| `urushi-adapter-ratatui` | Conversion of Urushi styles and resolved cells into a caller-owned Ratatui `Buffer`, stateless widgets, and anchor translation | Frame history, cell diffing, a runtime, terminal I/O, or session ownership |

The dependency direction follows those owners:

```text
urushi-terminal <- urushi <- urushi-tui <- urushi-tui-app
                         ^
                         |
              urushi-adapter-ratatui
```

`urushi-tui` also depends directly on `urushi-terminal` for output commands and
terminal geometry. `urushi-tui-app` depends directly on `urushi-terminal` for
input, queries, and session restoration, and its optional `graphics` feature
depends on `urushi-graphics` for protocol selection and lifecycle state. The
Ratatui adapter needs neither TUI crate for its ordinary caller-owned-buffer
path.

## Why these are crate boundaries

Each package serves an independently useful call site with a different
dependency budget.

A caller that owns an event loop can use `urushi-tui::Screen` to present
full-screen cell frames without adopting TEA or an async executor. A TEA
application opts into `urushi-tui-app` and receives model ownership, ordered
delivery, effects, subscriptions, and frame scheduling above that synchronous
engine. A Ratatui application opts into `urushi-adapter-ratatui` to draw an
Urushi `View` inside a buffer it already owns; it does not acquire Urushi's
screen history or application loop.

Cargo features inside one package would hide these independent public
surfaces behind configuration while retaining one release and dependency
boundary. Separate crates make the allowed dependency direction and the
absence of Ratatui and Tokio from the low-level engine mechanically visible.

## The low-level frame engine is concrete

`urushi-tui` supplies one Urushi-owned `Screen` implementation rather than a
public trait whose only production implementation is selected by the runtime.
The `Screen` owns working and committed buffers. `draw` lends one `Frame`, then
computes changed cells, writes them and the cursor request, flushes, and adopts
the working buffer only after every output step succeeds. A partial output
failure makes the physical surface unknown; the next draw clears it and sends
a complete frame before it may commit.

This is separate from `urushi_terminal::TerminalBackend`. A terminal backend
owns the physical bidirectional connection, parser, process-side modes, and
queries. A `Screen` borrows or owns an output capability for that connection
and owns only full-screen presentation history. The type name `Terminal` is not
used for the frame engine because it obscures that distinction.

The cell model and diff operation are Urushi-owned. Their `Empty`, grapheme
start, and continuation states; wide-cell ownership; default-blank equality;
and diff results remain equivalent to Noctui's buffer model. Ratatui is neither
the specification nor an implementation dependency of this path.

## Application composition

`urushi-tui-app` resolves each application `View` and writes its
`StyledGrapheme` values through a borrowed `urushi_tui::Frame`. Its runtime
creates or receives one physical terminal connection, uses that connection for
input, queries, and session restoration, and gives its output side to the
`Screen`. The application sees none of those resources.

The runtime-internal asynchronous presentation boundary remains separate from
the synchronous `Screen`. It coordinates a blocking draw worker with the
delivery loop and supplies a test double for scheduling tests; it is not an
alternative frame backend and is not a public cross-crate abstraction.

## Ratatui integration

`urushi-adapter-ratatui` retains the stateless integrations useful to an
application that already owns a Ratatui loop and buffer: `RatatuiStyle`,
`ViewWidget`, `RatatuiWidget`, cell-write modes, resolved-cell writing, and
anchor placement. Layout remains `urushi::resolve`; the adapter translates the
result and computes no competing geometry.

The adapter does not provide the application runtime's frame engine.
`RatatuiTerminal` and an equivalent runtime frame-target abstraction are not
part of the target design. This also isolates Ratatui version compatibility
from `urushi-tui` and `urushi-tui-app`.

## Rejected alternatives

- **Keep all three surfaces in `urushi-tui` behind features.** This leaves
  unrelated caller-owned-loop, TEA, and Ratatui APIs under one dependency and
  release boundary, and makes the package name insufficient to reveal which
  lifecycle a caller adopts.
- **Name the application crate `urushi-tui-runtime`.** Runtime is also a
  natural description of the lower frame and terminal execution machinery.
  `urushi-tui-app` names the distinguishing public concept instead.
- **Name the Ratatui crate `urushi-ratatui`.** That conventional spelling does
  not state whether Ratatui is Urushi's backend or an optional integration.
  `urushi-adapter-ratatui` makes its one-way adapter role explicit.
- **Retain a public `Terminal` or `FrameTarget` trait for the frame engine.** A
  second implementation is not a target use case after Urushi owns its buffer
  and diff. The application runtime's tests replace their internal
  presentation coordinator, while low-level output tests use a recording
  command writer. A public substitution point can be introduced later only
  for a concrete consumer need.
- **Put buffers and diffing in another foundational crate.** They currently
  have one consumer and change with `Screen`'s commit invariant. A module in
  `urushi-tui` keeps that rule cohesive without another public package.
