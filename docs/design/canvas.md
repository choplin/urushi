# Canvas

`Canvas` is the renderer-neutral drawing surface for one-frame presentation
that needs free placement, overlap, connected geometry, or cell-level
composition. It lets graphs, charts, annotations, minimaps, timeline marks,
and popups remain inside `View` resolution instead of computing backend
rectangles or writing to a backend buffer.

Canvas is a primitive drawing mechanism, not an application scene model. Pan,
zoom, selection, hit testing, routing policy, and animation remain application
or presentation state. A `CanvasItem` may capture the resulting immutable
frame data and turn it into drawing commands after the Canvas size is known.
The Canvas normally sizes as an independent viewport. A presentation such as
Table may instead supply one explicit intrinsic sizing policy for the whole
Canvas; Canvas never infers that policy from its items.

## The surface owns its size

A Canvas is a finite rectangle before any item draws. It carries exactly one
sizing mode, and its size never comes from item bounds. The default
**viewport sizing** follows these rules:

- on an axis for which its parent supplies a finite allocation, Canvas consumes
  that allocation;
- on an unbounded axis, Canvas requires an explicit extent for that axis; and
- an item may draw beyond any edge without enlarging or moving the Canvas.

This makes a Canvas inside a `Block` with `Fill` width and height consume the
Block's entire inner area. It also makes an independently sized Canvas possible
when both extents are stated. Missing an extent on an unbounded axis is an
invalid layout request rather than a request to inspect the items.

A Canvas may instead carry explicit **intrinsic sizing** supplied by the
presentation that constructs it. The sizing value reports horizontal demand
and floor, then reports vertical demand and floor after the parent has selected
the Canvas width. The parent remains the allocator: intrinsic sizing supplies
claims, not a final rectangle. Once both axes are selected, the Canvas is the
same finite viewport as one using the default mode, and drawing beyond any
edge is still clipped.

Intrinsic sizing is one policy for the Canvas as a whole. It is separate from
the ordered item collection, does not enumerate or measure those items, and
does not require the Canvas to contain only one item. The constructing
presentation owns the meaning of the policy. It may deliberately choose a
viewport smaller than its drawing or combine several items under one aggregate
rule; Canvas does not impose a fit relationship between the two values.

Canvas coordinates are signed cell coordinates local to the surface. The
origin is its top-left cell, positive `x` moves right, and positive `y` moves
down. Negative positions are valid. Drawing is clipped at all four Canvas
edges, and off-surface content never shifts the origin.

The size is independent of the drawing for two reasons. A responsive item must
know the final surface before deciding what to draw, and drawings such as a
grid, a camera projection, or a line leaving the viewport have no useful
content-derived bounding box. Inferring the surface from the items would make
those items and the surface determine one another. Explicit intrinsic sizing
does not reintroduce that cycle: it is a separate bound presentation value
whose staged measurement runs before any item draws.

## Intrinsic sizing binds presentation measurement

The public Canvas holds an opaque sizing value alongside its independent item
collection. Its representation has two semantic variants even though callers
cannot construct the intrinsic one directly:

```rust
pub struct Canvas {
    sizing: CanvasSizing,
    items: Vec<CanvasItem>,
}

pub struct CanvasSizing(CanvasSizingRepr);

enum CanvasSizingRepr {
    Viewport(ViewportSizing),
    Intrinsic(ErasedCanvasMeasure),
}
```

`Canvas::new` installs viewport sizing. An ordinary caller may state the
explicit extents that mode needs. A built-in presentation may replace it with
the opaque value returned by its bound item's private `sizing()` operation.
Supplying another sizing value replaces the previous one; a Canvas never
accumulates policies.

The private measurement capability is:

```rust
struct CanvasRequirements {
    demand: usize,
    floor: usize,
}

trait CanvasMeasure {
    fn width_requirements(&self) -> CanvasRequirements;
    fn height_requirements(&self, width: usize) -> CanvasRequirements;
}
```

`CanvasRequirements` is checked at construction: `floor <= demand`, and zero
demand with zero floor is valid. Width is measured first because wrapping and
track selection need the width the parent chose; height measurement receives
that selected width and may therefore report a taller demand for a narrower
Canvas. Repeated measurement with equal inputs is pure and deterministic.

The arbitrary `CanvasMeasure` implementation boundary and the constructor that
erases one are private initially. Built-in presentations expose ordinary
component APIs and construct intrinsic Canvas sizing internally. Opening the
measurement trait later is additive once an external custom-sizing use case
justifies committing to its invariants.

An intrinsic Canvas has auto claims. A surrounding `Block`, including one with
no visible frame, remains the only way to state `Cells`, `Fill`, minimum and
maximum dimensions, padding, alignment, and overflow. This keeps one box model
and lets the parent override or bound the Canvas's intrinsic claims before the
Canvas asks its items to draw.

Table is the representative binding:

```text
Table + TablePresentation
          |
        compose
          v
   bound TableItem ----------------------+
          |                              |
          +-- sizing() -> intrinsic -----+--> Canvas -> View::Canvas
          +-- draw(context) -------------+
```

The item owns or shares one immutable frame containing the selected Table data,
presentation policy, styles, and frame input. Its `sizing()` value measures
from that same frame, while its item implementation records cell and rule
commands after the final Canvas size is known. The two values enter Canvas
through separate APIs: one Canvas-wide sizing value and one member of an
arbitrary-length item collection. Canonical `TablePresentation::compose`
constructs the pair, so ordinary callers neither coordinate them nor interact
with Canvas directly.

## Items describe a frame; commands draw it

Canvas retains an ordered collection of owned `CanvasItem` values. An item is
domain-level, declarative frame data. After both Canvas axes are final, Canvas
creates a frame-scoped `CanvasContext` and asks each item, in collection order,
to draw into it:

```text
CanvasItem::draw(&self, context: &mut CanvasContext)
```

The context exposes the final Canvas-local size and bounds. An item may use
them to choose a scale, visible range, tick spacing, route, or other responsive
presentation decision. It then records commands; it does not write resolved
cells directly.

Items are comparable as data. Two items are equal when they have the same
concrete item type and equal captured values, not when they share an allocation
or happen to render the same cells. Given equal item data and the same context,
drawing must be deterministic. This preserves `View`'s value equality and
makes equality-based redraw avoidance sound.

Canvas sizing is comparable by the same value rule. Two viewport modes are
equal when their explicit extents are equal. Two intrinsic modes are equal when
their erased concrete measurement types and values are equal. A viewport mode
and an intrinsic mode are unequal. Measurement equality never compares an
allocation, a function address, generated commands, or resolved output, and it
includes every captured value capable of changing a requirement. Canvas
equality combines this sizing equality with ordered item equality.

The exact private mechanism used to own, erase, clone, and compare different
item types is an implementation decision. Rust and MoonBit may use different
wrappers to satisfy their type systems, but both implementations must expose
the same item semantics. Commands are not part of retained View data and do not
participate in View equality; they exist only for one resolve.

## The command vocabulary

The public context API and the recorded command abstraction have the same
semantic vocabulary. A convenience constructor may fill in a documented
default, but it does not lower into a differently shaped internal command.
Canvas starts with four commands because each represents a distinct
rasterization operation and has a useful command-wide composition default:

| Command | Meaning | Default composition |
| --- | --- | --- |
| `View` | Resolve one `View` at a signed origin, optionally with a finite allocation per axis; propagate its cells and anchors. | `Replace` |
| `Text` | Place styled graphemes directly at a signed origin without introducing View allocation semantics. | `Overlay` |
| `Path` | Rasterize connected geometry such as a line, polyline, rectangle, or arc. | `Overlay` |
| `Cells` | Place sparse, already-rasterized cell contributions. This is the escape hatch for item-specific rasterizers. | `Overlay` |

A `Path::line` or `Path::rectangle` constructor still creates a `Path` command;
there is no parallel `context.line(...)` operation that secretly lowers to a
different abstraction.

`Points` is not a separate standard command. Points already expressed at cell
coordinates are sparse `Cells`. World-space or subcell points require
item-specific choices of transform, marker, sampling, and aggregation; the
owning item makes those choices and emits `Cells`. Braille plotting is one
example: an item may aggregate a 2-by-4 subcell grid into Unicode Braille cell
contributions, but Canvas and the `Cells` command know nothing about Braille.

### Resolving a View command

A View command carries a signed origin and an optional finite allocation on
each axis. Whether an allocation is required depends on the placed View:

- an intrinsic axis, or a top-level `Block` axis stated as `Cells`, can settle
  without an allocation; and
- a top-level axis whose result depends on `Fill` requires the command to
  supply a finite allocation on that axis.

Omitting an allocation required by `Fill` is an error. Canvas bounds are not an
implicit allocation: a View may deliberately begin outside the surface or be
larger than it. One View command covers both intrinsic and allocated placement;
separate `ViewAt` and `ViewIn` item types would encode the same operation twice.

The View resolves in its complete local geometry and is then projected onto
the Canvas. Clipping does not cause a second layout or reflow. This preserves
the general layout rule that a decided size is never revised and no node is
assembled twice.

## Composition is paired with a command

Every recorded command consists conceptually of two independent values:

```text
RecordedCommand = { command, composition }
```

The command determines how input becomes cell contributions. The composition
determines how each contribution combines with the cell already produced by
earlier commands. The context has no mutable global composition mode. A
high-level command constructor selects its documented default, and callers can
select another compatible composition explicitly.

Composition is cell-local and deterministic: it receives the existing cell and
one structured contribution and returns the new cell. It cannot read or write
neighboring cells. Contributions emitted by one command for the same cell are
applied in their deterministic emission order. The standard rules are:

- `Replace` treats the contribution as complete cell content; a contributed
  blank is content and erases what was behind it.
- `Overlay` applies the fields present in the contribution and preserves fields
  the contribution leaves absent. Absence, rather than a space or default
  style, expresses transparency.

The composition vocabulary is extensible. A specialized rule can, for example,
union Braille dot masks or merge box-drawing connections. Such rules belong to
the commands that require them; Canvas does not recognize either drawing
domain. Keeping composition separate from command semantics lets the same
`Path` use ordinary overlay or box-junction merging without turning those into
different geometry commands.

One recorded command uses one composition rule. A composite item that needs
different rules records multiple commands. This keeps command application
predictable and avoids a composition branch on every contribution.

Wide grapheme integrity remains the surrounding compositor's responsibility,
outside the cell-local composition rule. Clipping or replacing any occupied
cell of a wide grapheme must not leave a continuation cell or half a glyph
behind. The internal cell representation may differ between implementations,
but the resolved result is grapheme-atomic.

## Ordering, clipping, and anchors

Items are invoked in collection order, commands remain in recording order, and
their contributions compose in that order. There is no separate `z_index` or
shape-wide opacity rule. Ordering is defined at cell granularity, so a sparse
later command changes only cells to which it contributes.

Anchors produced by a View command remain ordinary resolved-view metadata. An
anchor's signed local origin is translated by the View command's origin and
later by the Canvas's offset when the parent assembles it. The caller resolves
the parent and uses the same `anchor(key)` lookup as for any other anchor.

Clipping and composition do not erase anchors: an anchor reports where layout
put its region even when its cells are clipped or overwritten. One key still
names one region. Canvas items cannot query anchors recorded by earlier items,
and Canvas introduces no separate arbitrary-anchor command without a use case.
Supporting negative placement requires `AnchoredRect` origins and offset
arithmetic to use a signed coordinate domain; containment checks consider all
four edges of the final resolved rectangle.

## Resolve walkthrough

Consider a graph surface inside a bordered Block whose two axes are `Fill`.
The parent assigns the Block 64 by 22 cells; after its frame, Canvas receives a
finite 62-by-20 allocation. Its items capture a camera, graph data, node Views,
and a minimap policy.

1. Default viewport measurement and parent area sharing settle the Block and
   Canvas at 62 by 20. No graph node or edge is inspected to determine that
   size.
2. Canvas creates a context whose local bounds are `(0, 0, 62, 20)`.
3. The graph item reads those bounds, projects visible world coordinates, and
   records edge `Path` commands using a box-junction composition rule.
4. It records node `View` commands in display order. An intrinsic node at
   `(-2, 3)` needs no allocation: it resolves completely, then its left two
   columns are clipped. A node whose root width is `Fill` records the finite
   width intended for that node.
5. A label item records `Text` with its default overlay. A popup item records a
   `View` with `Replace`, so the popup's intentional blank cells cover graph
   cells beneath it.
6. The minimap item uses the Canvas size to select its projection, aggregates
   subcell samples, and records sparse `Cells` with a dot-union composition.
7. Canvas rasterizes the recorded commands and applies every cell contribution
   in recording order. It clips at all four surface edges while preserving
   wide-grapheme ownership.
8. Anchors from node and popup Views are translated by their command origins.
   Canvas returns one rectangle of cells plus those anchors; parent assembly
   adds the Block's content offset, and the ordinary root `ResolvedView`
   contains the final cells and anchor coordinates.

The same model also covers simpler cases. A popup is one `View` command, a
timeline can combine `Text`, `Path`, and sparse `Cells`, and a scatter plot can
rasterize its points directly to `Cells`. The graph-specific camera, routing,
and sampling policies stay in items or their owning presentation rather than
becoming Canvas state.

An intrinsically sized Table takes the other measurement path without changing
the drawing path. Its sizing value reports the unbounded column demand and
floor, the parent selects a width, and it reports the row demand at that width.
After the parent selects the height, the Canvas creates the same final context
and invokes the Table item. Cell placement and the rule network may extend
beyond the selected rectangle; Canvas applies the ordinary crop rather than
revising either axis or reflowing the Table.

## Alternatives rejected

**Derive Canvas bounds from its items.** This creates a cycle for responsive
items and gives viewport-like drawings no stable surface. Canvas instead owns
a finite size before drawing. Explicit intrinsic sizing is supplied separately
by the constructing presentation and never discovers item bounds.

**Add a separate Region node for measured presentation output.** Region and
Canvas would both retain erased presentation behavior, receive a local area,
and produce renderer-neutral cells, while Region would need another public
output-construction API for operations Canvas already expresses as commands.
Optional intrinsic sizing gives Canvas the missing layout participation without
changing its viewport, item, composition, or clipping contracts.

**Make the sizing provider the only Canvas item.** A Canvas commonly combines
several independently ordered items. Sizing is one Canvas-wide value and items
remain a separate arbitrary-length collection. A component item may offer a
convenience `sizing()` derived from the same bound frame without coupling the
generic Canvas APIs.

**Provide only positioned child Views.** This handles opaque boxes but forces
edges, marks, subcell plots, and sparse decorations back through artificial
Views or backend writes. The four commands cover the distinct rasterization
needs found in the target applications.

**Make every primitive shape a command.** Separate point, line, rectangle,
Braille, and marker commands confuse input convenience with rasterization
semantics. Constructors may build `Path`, while specialized sampling lowers to
`Cells`.

**Put one composition mode on the context.** Composition depends on the
recorded operation: a popup replaces, ordinary text overlays, and crossing
edges may merge. Global mutable mode would make unrelated commands depend on
ambient state.

**Make composition part of command semantics.** A path remains connected
geometry whether its cells overlay or merge. Keeping the values separate
allows the same command to use another compatible rule without duplicating the
command vocabulary.

**Retain commands in the View.** Commands depend on the final Canvas size and
are an execution artifact. Retaining them would either prevent responsive
drawing or put stale derived data into View equality.

**Make Canvas a graph or interaction scene.** Persistent topology, cameras,
selection, hit testing, and events have different owners. Canvas represents
only the renderer-neutral drawing of the current frame.

## Cross-language contract

Urushi and Noctui use the same Canvas sizing modes, staged intrinsic
measurement, command types, coordinate rules, defaults, ordering, clipping,
composition semantics, anchor results, and value equality. Language and
runtime constraints may change the spelling and private type-erasure machinery
only. A representation is acceptable when both implementations produce the
same `ResolvedView` for equivalent input.
