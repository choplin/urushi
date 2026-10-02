---
title: TUI reference
description: Complete index of the native frame engine, application runtime, and Ratatui adapter APIs.
---

This page is the API index for Urushi's three full-screen crates. Use
[TUI overview](/docs/tui/) to choose an ownership model and
[Interactive application](/docs/tui/application/) for a complete runtime app.
Use [Effects and subscriptions](/docs/tui/effects-and-subscriptions/) and
[Message delivery and drawing](/docs/tui/delivery-and-drawing/) for the runtime's
asynchronous and ordering contracts,
[Native Screen and Frame](/docs/tui/screen-and-frame/)
for a caller-owned native loop, or [Ratatui integration](/docs/tui/ratatui/)
for a caller-owned Ratatui buffer.

## Application runtime: `urushi-tui-app`

[Complete generated API](/api/urushi_tui_app/index.html)

| Area | Public surface |
|---|---|
| Application model | `Application::{init, update, view, subscriptions}` |
| Entry points | `run`, `Runtime::{new, run}` |
| Runtime dependencies | `Runtime::{backend, executor, clock}` |
| Session options | `raw_mode`, `alternate_screen`, `bracketed_paste`, `focus_change`, `keyboard_enhancement`, `mouse`, `hide_cursor` |
| Graphics feature | `Runtime::graphics`, re-exported `GraphicsPreference` |
| Effects | `Effect::{none, shutdown, perform, perform_latest, future, future_latest, after, after_latest, batch, map}` |
| Subscriptions | `Subscription::{none, input, surface, interval, signal, terminal_errors, stream, stream_with, run, run_with, run_blocking, run_blocking_with, batch, map}` |
| Admission and delivery | `Admission::{bounded, latest}`, `Sender`, `SendError` |
| Runtime abstractions | `Executor`, `Execution`, `Task`, `BlockingTask`, `Clock`, `TokioExecutor`, `TokioClock` |
| Runtime errors | `Error::{Terminal, Runtime}` |
| Event values | `Input`, `KeyEvent`, `KeyCode`, `KeyKind`, `KeyEventState`, `MediaKeyCode`, `ModifierKeyCode`, `Modifiers`, `MouseEvent`, `MouseKind`, `MouseButton`, `Surface`, `SurfaceSize`, `CellPixels`, `FocusChange`, `Signal` |

The `crossterm` feature is enabled by default. The optional `graphics` feature
adds runtime-owned terminal image presentation; applications still use a
direct `urushi-graphics` dependency to construct images.

## Native frame engine: `urushi-tui`

[Complete generated API](/api/urushi_tui/index.html)

| Type | Public surface and role |
|---|---|
| `Screen<W>` | `new`, `size`, `writer`, `into_inner`, `resize`, `invalidate`, `modify_surface`, `draw`, `draw_with` |
| `Frame<'_>` | `area`, `put`, `set_cursor` |
| `Rect` | `new`, `from_size`, `origin`, `size` |
| `Position`, `TerminalSize` | Re-exported terminal geometry values |
| `crossterm` module | Default-feature backend re-export |

`Screen` is synchronous and owns no event loop, model, executor, or terminal
session. `urushi-tui-app` supplies those policies when a complete runtime is
needed.

## Ratatui adapter: `urushi-adapter-ratatui`

[Complete generated API](/api/urushi_adapter_ratatui/index.html)

| Area | Public surface |
|---|---|
| Complete views | `ViewWidget::{new, cell_write_mode}` |
| Single blocks | `RatatuiStyleExt::widget`, `RatatuiWidget::{new, cell_write_mode}` |
| Resolve once | `available`, `draw_resolved`, `draw_resolved_with_mode` |
| Foreign regions | `anchor_placement`, `RatatuiAnchor::{logical, destination, source_column, source_row}` |
| Style conversion | `RatatuiStyle`, `RatatuiStyle::into_inner` |
| Cell composition | `CellWriteMode::{Merge, Replace}` |

The adapter owns no terminal, event loop, application state, or graphics
lifecycle. It writes Urushi cells into a Ratatui `Buffer` supplied by the
caller.
