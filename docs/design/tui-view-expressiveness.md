# TUI View Expressiveness

[`tui-architecture.md`](../tui-architecture.md) makes the application `Model`
and its transitions the responsibility of `urushi-tui`, while
[`view-model.md`](../view-model.md) makes `urushi::View` the renderer-neutral
description of one frame. This document states how to judge whether that one
view model is expressive enough for full-screen applications and allocates the
missing responsibilities without introducing a second TUI tree.

## The criterion

For any current TEA `Model` snapshot, application and component presentation
code must be able to construct the intended frame as a `View` without either:

- computing final rectangles that only `resolve(Available)` can know; or
- writing backend cells directly for ordinary Urushi-native content.

That `View` may contain an intrinsically sized Canvas whose renderer-neutral
measurement runs inside `resolve` before its items draw. Canvas is not a
foreign backend escape: it emits the same `ResolvedView` cells and placements
as built-in primitives and remains subject to the same local-area and
terminal-independence rules. Its contract is [`canvas.md`](canvas.md).

The view need not describe how the snapshot was reached. Key presses, focus
movement, selection changes, scrolling commands, drag gestures, animation
ticks, asynchronous work, and redraw scheduling remain outside it. It must
describe what those facts make visible *now*: the selected styling, the
visible slice, the modal layer, the current camera projection, and other
one-frame results.

A foreign region is an intentional extension boundary for a third-party widget
or terminal graphics operation. Its existence does not by itself satisfy this
criterion for visual structures Urushi intends to present natively: reserving
a rectangle is not expressing the content of that rectangle as a `View`.

## Evidence from full-screen applications

The model was tested against representative terminal applications and Ratatui
examples rather than against call sites inside this repository. The evidence
set covered repository browsers and file managers, process dashboards, HTTP
clients, fuzzy finders and command palettes, server dashboards, forms, charts,
free-positioned drawing, and a rataflow-based trace visualizer.

Across those examples, ordinary rows, columns, blocks, grids, and uniformly
styled text cover a substantial base: split panes, headers and status bars,
detail panels, bordered regions, simple lists, tables, trees, summaries, and
warnings. Three recurring one-frame requirements are not adequately expressed
by the current primitive set, and a fourth extension boundary must remain
coherent with them:

1. **Inline styled flow.** Syntax-highlighted source, search matches, logs,
   prompts, and status lines contain style changes inside one wrapping and
   clipping flow. Adjacent `Row` children are independently sized boxes, not
   runs in one text line, and therefore cannot substitute for this primitive.
2. **Viewport projection.** Lists, tables, trees, text panels, and timelines
   show a window beginning at an application-supplied offset. Height clipping
   alone always exposes the beginning and cannot express the current window.
3. **Positioned overlap and draw order.** Popups, help layers, floating panels,
   graph nodes and edges, minimaps, and annotations place content relative to a
   containing region and may overlap it in a defined order. Sequential layout
   and keyed rectangles do not express that scene.
4. **Reserved foreign regions.** Third-party Ratatui widgets and terminal
   graphics still need a generic region whose geometry participates in Urushi
   layout even when its pixels or cells are produced by an adapter. The
   existing anchor establishes the reservation and geometry report. Future
   viewport and positioned transforms must preserve that contract, while the
   caller or adapter that has the backend remains responsible for drawing it.

Zoetrope is the strongest combined boundary test. Its semantic session model
is projected through rataflow into a graph, while one frame combines ordinary
row/column panels with a graph surface, minimap, timeline, conditional detail
panel, and help or information overlays. Pan, zoom, selection, timeline
movement, and panel scrolling belong to application state; the resulting
camera, visible ranges, highlight choices, and layer arrangement are still
facts the frame must express.

The external source inventory and the particular capability each example is
useful for inspecting belong in the project research notebook, not in this
design contract.

## Responsibility allocation

### Core `urushi`

Core owns only renderer-neutral, one-frame presentation mechanics:

- logical text and block appearance;
- sequential and shared-track layout;
- inline runs participating in one wrapping and clipping flow;
- a viewport projection whose offset is supplied by its caller;
- a Canvas for positioned placement, overlap, connected geometry, sparse
  cells, clipping, ordered cell composition, and optional Canvas-wide intrinsic
  sizing supplied explicitly by a built-in presentation;
- keys and reported regions that follow the same transforms and clipping as
  the content around them; and
- reservation of a foreign region without naming Ratatui or another backend.

The `Text`, `Block`, `Row`, `Column`, `Grid`, `Canvas`, and
`AnchorBlock` nodes are members of this vocabulary. Canvas is a finite drawing
surface whose default size is independent of its items. An intrinsic sizing
value may measure the Canvas as a whole, and its items record `View`, `Text`,
`Path`, and `Cells` commands only after the size is known. Its exact contract is recorded in
[`canvas.md`](canvas.md). Viewport projection remains a separate design topic.

Core does not gain semantic nodes such as `View::Graph`, `View::Modal`, or
`View::SelectedRow`. It also does not gain event callbacks, focus movement, or
backend `Rect` and `Buffer` values. `resolve` remains the only operation that
receives the final available area; it performs any Canvas intrinsic measurement
with the derived local constraints before invoking Canvas items.

### Reusable components and presentations

Components own semantic data and concrete ways to lower it to primitive views
or bound Canvas items.
List, Tree, Table, Summary, and Warning remain the canonical common components.
Graphs, charts, sparklines, gauges, tabs, scrollbars, modal frames, and text-field
visuals are component or presentation candidates when they add reusable
semantic or visual policy; they are not reasons to add corresponding semantic
`View` variants.

A presentation may borrow a component-specific, immutable snapshot needed to
describe this frame — for example the selected identity, expanded identities,
visible origin, cursor position, or graph camera. That input is presentation
data, not a shared component-state abstraction. The presentation neither
changes it nor decides what an event means. The exact input type and inherent
method signature follow the needs of that component, as
[`component-presentation.md`](component-presentation.md) records.

### `urushi-tui`

`urushi-tui` owns time and interaction:

- the TEA `Model`, `Message`, `update`, effects, and subscriptions;
- focus, selection, expansion, navigation, modal stacks, and command mapping;
- scroll offsets, pan and zoom cameras, hit testing, clicking, and dragging;
- animation clocks, timeline playback, asynchronous I/O, and redraw policy;
- terminal cursor execution; and
- adapter support for callers that execute foreign widgets or graphics.

The runtime lends the current model to the application's `view` function. The
application selects or derives the immutable presentation inputs for that
frame, components construct built-in nodes or bind Canvas items and sizing, and
the runtime resolves and draws the result. No interaction transition runs
during composition or resolution.

```text
event -> update(Model) -> current Model snapshot
                              |
                              v
                   application view / presentations
                              |
                              v
                      primitive urushi::View
                              |
                    resolve(Available)
                              v
                         ResolvedView
                              |
                  urushi-tui backend adapter
```

## Invariants

The primitive APIs must preserve these constraints:

- inline runs wrap and clip as one flow rather than as independently allocated
  siblings;
- viewport offsets affect content projection, not the ownership of interaction
  state, and nested layout still receives only its allocated area;
- positioned descendants do not receive final backend rectangles before
  resolution;
- overlap has deterministic ordering and clipping across all renderers;
- anchors and other keyed placements are transformed and clipped with their
  containing viewport or positioned scene;
- foreign-region reservation stays renderer-neutral, while executing the
  foreign renderer stays in `urushi-tui`; and
- Urushi and Noctui adopt equivalent types, invariants, defaults, and
  operations once those contracts are decided.

These rules reject both extremes: placing interaction machinery in core, and
leaving ordinary one-frame presentation to application-computed rectangles or
direct buffer writes.
