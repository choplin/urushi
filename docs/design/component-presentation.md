# Component Presentation Boundary

[`component-model.md`](../component-model.md) defines the four public layers:
semantic data, concrete presentation, primitive `View`, and `ResolvedView`.
This document records why the boundary and its public vocabulary have that
shape.

## Why composition is a separate operation

Semantic data must survive a change of theme, backend, available area, and
structural presentation. Putting marker callbacks, cell styles, or resolved
positions on the data makes those choices part of what a list, tree, table, or
graph *is*. Keeping composition separate lets one borrowed data value be shown
by multiple borrowed policies without copying or mutating either one.

The operation returns `View` rather than rendered text. Logical styles and
layout intent therefore survive until the one resolver and the selected output
adapter, instead of becoming ANSI that another backend cannot interpret. A
presentation whose algorithm depends on its eventual local area may bind its
owned frame into a Canvas item and supply one intrinsic sizing value for that
Canvas rather than precomputing the algorithm during composition.

## Why the composing type is a presentation

`TextStyle`, `BlockStyle`, and `GridStyle` are declarative inputs attached to
existing primitive nodes. They do not inspect semantic data or decide which
nodes exist. The former component types named `ListStyle`, `TreeStyle`, and
`TableStyle` did both: they held styles and callbacks, then interpreted data to
construct a tree.

Naming those values `...Presentation` makes the executable responsibility
visible. The appearance values they contain remain ordinary primitive styles;
the presentation owns the structural algorithm that applies them. Keeping the
two together is deliberate while only one implementation uses that exact
combination. Splitting out another public style object merely because a future
presentation might share it would add a pair callers always have to keep in
sync without serving a current second use.

The verb is `compose`: it names construction of a renderer-neutral tree without
suggesting terminal output. It is a responsibility and naming convention, not
a universal signature. The ordinary inherent method may take only semantic
data, while a presentation that needs more information for one frame may also
borrow its own immutable presentation input through a component-specific
signature or additional method. `render` remains an output-boundary verb,
while `view` remains the type and the application-level function that describes
a whole application screen.

## Why canonical calls live on Theme

The ordinary caller wants the canonical, theme-derived presentation. Requiring
the complete path

```rust
theme.components().tree().compose(&tree)
```

at every call would expose selection machinery without adding information.
`theme.tree(&tree)` is an exact delegating shortcut. `Theme` is the subject
that chooses the canonical presentation, so the shortcut neither puts
presentation behavior on semantic data nor implies that the canonical shape is
the component's only possible shape.

The complete path remains available for local customization. Alternate
structural presentations are named explicitly and do not receive a shortcut
unless one later becomes Urushi's canonical presentation.

## Why presentations do not share a trait

`compose` is a convention over independent combinations such as
`TreePresentation`/`Tree` and `TablePresentation`/`Table`. Their signatures
need not match: a static list may need only its semantic data, while an
interactive table or graph presentation may borrow an additional typed
snapshot for selection, visible origin, or camera. No external caller
currently needs one collection or generic function over those unrelated data
models and presentation inputs. A common trait would either erase those
distinct input types or accumulate associated types solely to describe calls
already expressed by inherent methods.

Internal traversal reuse stays private. A public trait becomes justified only
by an external generic use, not by the number of similarly named methods.

Canvas intrinsic sizing does not contradict this decision. A presentation has
already bound its component snapshot, policy, styles, and frame input into one
owned item before Canvas asks the corresponding sizing value to measure it.
Presentations remain unrelated concrete interpreters; the private measurement
capability shares only the staged width and height questions every parent
layout needs.

## Why View has no semantic component variants

`View` never exposes component meaning in its own vocabulary. It may describe
text, boxes, sequential layout, shared tracks, keyed placements, and a generic
Canvas, but it cannot name Table headers, Tree branches, Graph edges, selection,
or another component role.

A semantic node such as `View::Tree` would defer the presentation decision into
the resolver. That would make every component part of the built-in layout enum,
couple resolver evolution to component evolution, and require the Noctui
resolver to know the same growing component set. A bound presentation item may
instead interpret its own captured inputs and record generic Canvas commands;
the resolver invokes it without matching on the originating component.

This is also why Grid is not a common component intermediate representation.
The canonical Table presentation uses an intrinsically sized Canvas and leaves
Grid as a directly useful shared-track container. A tree or graph independently
chooses built-in primitives or Canvas from its own layout and drawing needs.

## Graph as the boundary test

A future Graph makes all four layers visible:

- topology, node and edge identity, and semantic content belong to Graph data;
- layered, force-directed, or explicitly positioned structure, node
  presentation, and edge routing belong to a named concrete presentation;
- XY placement, overlap, clipping, and draw order belong to generic layout
  primitives when those primitives are adopted; and
- the final cells and placements belong to `ResolvedView`.

Pan, zoom, drag, selection, hit testing, and event handling are application or
interaction state. They do not enter the semantic Graph merely because one
interactive graph UI uses them, and they do not enter a Canvas primitive merely
because that UI draws onto one.

That separation does not make the current camera or selection invisible to the
presentation. `urushi-tui` owns and updates those values; a Graph presentation
may borrow their immutable snapshot to compose the frame that results. It
cannot mutate them, interpret input events, or prescribe their transitions.
The general one-frame expressiveness rule is recorded in
[`tui-view-expressiveness.md`](tui-view-expressiveness.md).

Canvas owns the generic coordinate, sizing, clipping, command, and composition
contract recorded in [`canvas.md`](canvas.md). A future Graph presentation
owns its graph-specific projection, routing, and immutable frame inputs. This
boundary prevents either side from creating `View::Graph` or a second
backend-specific compositor.

## Area independence

A presentation's `compose` operation cannot receive `Available`. A component
inside a `Row` does not know its share until sibling claims are resolved; a
presentation given the terminal width would know the root area, not its own
area. Pre-wrapping or padding against that number makes the result correct only
at the root.

Presentations therefore compose structural intent or bind it into an owned
Canvas item with one Canvas-wide intrinsic sizing value. `resolve` alone
receives the area: it asks that sizing value for width requirements, selects the
local width after sibling sharing, asks for height requirements at that width,
and finalizes the Canvas viewport before any item draws. The presentation never
receives the root area during composition. An application that chooses a
different overall structure at a breakpoint does so in its own view function
before composition, where it owns the same area that will be passed to
`resolve`.

## Table binds one Canvas frame

The canonical `TablePresentation` owns the reusable presentation policy. Its
`compose` operation binds the selected Table data, presentation, logical
styles, visible-row offset, and any immutable frame input into one owned Table
item. The item exposes a private `sizing()` derived from that same bound frame;
composition installs the returned value as the Canvas's one sizing policy and
adds the item to the Canvas's independent ordered item collection.

The sizing value and item may share immutable storage, but allocation identity
has no semantic meaning. The sizing value reports column-based width
requirements and height at the selected width. After the final Canvas size is
known, the item records cell and rule commands. The Canvas contract owns the
measurement order, command recording, composition, and clipping; the Table
presentation owns the column algorithm, role styling, rule selection, and the
meaning of its presets. The exact Canvas flow is in
[`canvas.md`](canvas.md), while Table/Grid separation and line-network
ownership are in [`grid.md`](grid.md).

A per-cell style strategy is executable presentation policy, so
`TablePresentation` stores it as an owned, type-erased, comparable value rather
than a function address. Equality first requires the same concrete strategy
type and then delegates to that concrete value's `PartialEq`. A user-defined
strategy may implement that equality manually, but it must compare every value
that can change measurement, text, style, or placement. Allocation identity,
function addresses, generated Canvas commands, and resolved output are not
valid equality mechanisms. Canonical presets hide this machinery from ordinary
callers.

## Rejected designs

- **Keep `...Style::view(&data)`.** It preserves a short existing call but uses
  the same suffix for passive primitive settings and semantic interpreters.
  Documentation cannot remove that false equivalence from the call itself.
- **Put `view(&Theme)` on semantic data.** It shortens the canonical call but
  makes the data type choose its presentation and makes an alternate shape look
  secondary to the component's meaning. A Theme shortcut is equally short and
  leaves the choice with the theme.
- **Split appearance and algorithm for every component now.** Current callers
  would always carry two values for one real presentation. Extract shared
  appearance only after two implemented presentations need it.
- **Add one public `Presentation` or `Component` trait.** There is no external
  generic use, and the semantic and frame-input models are intentionally
  independent.
- **Put semantic component nodes in `View`.** It moves component evolution into
  the resolver and turns the primitive tree into a closed registry of product
  concepts.
- **Give presentation composition `Available`.** It supplies the wrong area for
  nested composition and bakes geometry before sibling sharing. A Canvas-wide
  intrinsic sizing value participates in ordinary local layout instead.
- **Add a separate Region primitive.** Region would duplicate Canvas's erased
  presentation execution and renderer-neutral output boundary, then require a
  second drawing API. Optional intrinsic Canvas sizing supplies the missing
  measurement without adding another `View` node.
- **Adopt a single all-in-one Graph widget model.** Combining graph topology,
  layout, interaction, and backend rendering prevents the same semantic graph
  and presentation from participating in Urushi's ordinary `View` pipeline.

## Consequences

The public target renames the existing component presentation values to
`ListPresentation`, `TreePresentation`, and `TablePresentation`, moves Summary
and Warning composition into concrete presentation values, names their
aggregate `ComponentTheme`, and reserves `...Style` for declarative values.
Canonical Theme shortcuts keep ordinary calls short.

This is a deliberate pre-alpha source break rather than a deprecation cycle.
The implementation issues remove the old names and methods when they introduce
their replacements; no compatibility aliases or forwarding `view` methods
remain. The source-level migration is mechanical:

| Current call or type | Target API |
| --- | --- |
| `ListStyle`, `TreeStyle`, `TableStyle` | `ListPresentation`, `TreePresentation`, `TablePresentation` |
| `style.view(&data)` | `presentation.compose(&data)` |
| `theme.components().list().view(&list)` | `theme.list(&list)` for the canonical call, or `theme.components().list().compose(&list)` for customization |
| `summary.view(theme.components(), width)` | `theme.summary(&summary)` or `theme.components().summary().compose(&summary)`; `resolve` receives the width |
| `warning.view(theme.components(), width)` | `theme.warning(&warning)` or `theme.components().warning().compose(&warning)`; `resolve` receives the width |
| `ComponentStyles` | `ComponentTheme`, including terminal helper parameters that consume theme-derived component values |

Component migrations replace width-dependent wrapping, measured padding, and
repeated-glyph construction during composition with primitive layout intent or
an intrinsically sized Canvas. Urushi and Noctui use the same four layers,
names, invariants, and operation; only type-erasure, dynamic equality,
borrowing, and language-specific method spelling may differ.
