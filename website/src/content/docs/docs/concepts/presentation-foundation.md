---
title: One presentation foundation
description: How Urushi shares styling, composition, and layout without conflating terminal surfaces.
---

Urushi treats plain output, prompts, and full-screen TUIs as different terminal
surfaces built on the same presentation foundation. They are not stages in an
application lifecycle, and one surface does not have to grow into another.

## Shared below the surface

The shared model consists of five layers:

| Layer | Responsibility |
|---|---|
| `Theme` | Assign semantic roles to logical styles and canonical component presentations. |
| Components and presentations | Keep semantic data separate from the concrete policy that composes it into a `View`. |
| `View` | Compose renderer-neutral primitives: text, blocks, rows, columns, grids, canvas content, viewports, and anchors. |
| Layout | Resolve the box model and display width against an available area. |
| Terminal | Describe commands, capabilities, events, geometry, and session restoration independently from a physical backend. |

The result is a consistent visual language without forcing every surface into
one event loop. Plain output remains a static write. A prompt owns its blocking
interaction and cleanup. The full-screen TUI has its own TEA-style runtime,
while the Ratatui adapter is a separate path for applications that already own
their loop.

## Renderer-neutral views

A `View` describes presentation before the destination is chosen. It can resolve
to ANSI output for an ordinary CLI, into the full-screen runtime, or into a
caller-owned Ratatui `Buffer` through the optional adapter.

That boundary keeps components reusable while allowing each renderer to preserve
its own rules and capabilities.

Lists, tables, trees, summaries, and warnings are not extra `View` variants.
Their presentations interpret semantic data and lower it into the same
renderer-neutral primitives. Layout then receives the available area exactly
once, regardless of which component produced the view.

## Full-screen runtime and terminal backends

The full-screen architecture is centered on `Application`: a pure program
value with `init`, `update`, `view`, and `subscriptions`. The runtime owns the
live model, effect execution, ordered delivery, frame scheduling, and terminal
restoration. Application code does not own those resources and does not depend
on a backend type.

`urushi_tui_app::run` assembles and runs the end-to-end TEA lifecycle. It draws
through the concrete `urushi-tui` `Screen` and `Frame`, whose Urushi-owned cell
buffers provide diffing and transactional output. The caller-owned Ratatui path
lives independently in `urushi-adapter-ratatui`; neither the frame engine nor
the application runtime depends on Ratatui.

## Semantic themes

Components ask a `Theme` for semantic roles such as focused panels, prompt
options, success, warning, or error. They do not decide concrete terminal
colors themselves. Applications can therefore maintain one visual language
across independently owned surfaces.

## Width is part of presentation

Terminal layout depends on display cells rather than bytes or Unicode scalar
values. Urushi measures grapheme clusters and accounts for East Asian width when
wrapping, aligning, joining borders, and resolving boxes.
