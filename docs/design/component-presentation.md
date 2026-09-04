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
adapter, instead of becoming ANSI that another backend cannot interpret.

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

## Why View is primitive-only

Lowering ends semantic ownership. `View` may describe text, boxes, sequential
layout, shared tracks, keyed placements, and a future generic positioned
layout, but it cannot name Table headers, Tree branches, Graph edges, selection,
or another component role.

A semantic node such as `View::Tree` would defer the presentation decision into
the resolver. That would make every component part of the closed layout enum,
couple resolver evolution to component evolution, and require the Noctui
resolver to know the same growing component set. A generic primitive instead
states only the geometry a presentation chose.

This is also why Grid is not a common component intermediate representation.
A table may lower to Grid, but a tree or graph chooses primitives from its own
layout needs. If Grid supports selective separators, their public vocabulary is
positional and generic; it never names a header or another Table role.

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

The exact Canvas coordinate and clipping contract, and the exact inputs of a
future Graph presentation, remain separate design questions. Issue #59 owns
the Canvas feasibility evidence; Issue #60 owns the binding primitive,
coordinate, sizing, clipping, and ordering contract. This boundary only
requires that those choices do not create `View::Graph` or a second
backend-specific compositor.

## Area independence

A presentation cannot receive `Available`. A component inside a `Row` does not
know its share until sibling claims are resolved; a presentation given the
terminal width would know the root area, not its own area. Pre-wrapping or
padding against that number makes the result correct only at the root.

Presentations therefore compose structural intent. `resolve` alone receives
the area and decides final widths, wrapping, clipping, and placement. An
application that chooses a different overall structure at a breakpoint does so
in its own view function before composition, where it owns the same area that
will be passed to `resolve`.

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
- **Give a presentation `Available`.** It supplies the wrong area for nested
  composition and bakes geometry before sibling sharing.
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

The downstream ownership is explicit: Issue #61 decides Grid/Table lowering
and generic separator vocabulary; Issue #62 decides area-independent List/Tree
layout intent; Issue #66 migrates Summary; and Issue #48 migrates Warning.
Those component migrations replace width-dependent wrapping, measured padding,
and repeated-glyph construction with primitive layout intent. Future Canvas
and Graph work must preserve the same lowering boundary. Urushi and Noctui use
the same four layers, names, invariants, and operation; only borrowing and
language-specific method spelling may differ.
