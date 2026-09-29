# The TUI View and the Anchor

A full-screen application's `view` function returns
[`urushi::view::View`](../../urushi/src/view/model.rs), the same tree a plain
CLI call site builds, and a box in that tree may carry a key — an anchor — for
a region whose position layout decides but whose content this crate does not
produce. [`tui-architecture.md`](../tui-architecture.md) states the shape; this
file holds the anchor's rule and records why it is that and not something else.

## The rule

An anchor is a block that carries a `Key` — an `AnchorBlock`: layout sizes it
by the rules every block follows, and `resolve` reports the rectangle its frame left beside the
resolved rows, for the caller that knows what belongs there to fill. An anchor
carries no geometry of its own — the box states the size, as it does everywhere
else — and its content is an ordinary child, so an empty one resolves to
blanks. It names no backend type: an anchor is a key and a rectangle, and a
backend with nothing to put there draws the blanks it resolved to.

The reported rectangle is the one the box would have given a child: inside the
border and the padding, past the margin. That is what a caller filling the
region is filling, so a border the box stated frames the drawing rather than
being covered by it. Anchors are reported in tree order, a box before what it
encloses, and one key names one region: two anchors carrying one key are a
contract violation rather than something layout resolves.

A rectangle states where layout put the complete logical region, not what
survived into the rows, and is never bounded or moved onto a clipping edge. A
visible intersection is reported alongside it: the complete rectangle when
visible, a smaller rectangle when partially clipped, and absent when outside.
Bounding the logical rectangle would round a cursor projected out of sight onto
the last row, which is the one thing a caller placing a cursor must not be
told. The separate intersection answers where drawing is valid without
destroying the position that produced it.

More generally, every stage that removes cells intersects the accumulated
visible region before parent translation: Block content clipping, Canvas and
Viewport boundaries, and the final degenerate safety crop. The logical
rectangle is never cut. Once a region is outside a local clip, later placement
cannot make it visible again; ancestors may only narrow the accumulated
intersection. `Viewport` does not introduce a special anchor rule, but makes
this existing distinction observable at arbitrary origins.

For a zero-sized cursor anchor, visibility tests the point against half-open
cell bounds. A point on the right or bottom edge is outside; showing a cursor
after the final grapheme therefore requires the presentation to allocate the
cell where the cursor is to be drawn.

Two things reach the frame that way:

- **Cursor placement.** An anchor with no box around it covers no cells, and
  its reported origin is the cell the terminal cursor belongs on. The `Frame` of
  [`tui-terminal-ownership.md`](tui-terminal-ownership.md) carries the request
  for a draw; this is how an application states it.
- **Foreign regions.** A sized anchor's rectangle is where a caller that holds
  the backend's buffer draws something this crate does not produce — a chart,
  a canvas, a third-party widget — after writing the resolved cells. That
  caller is an application that owns its loop and draws an Urushi view into
  its own buffer; the runtime's renderer serves the cursor anchor only, as
  [`tui-terminal-ownership.md`](tui-terminal-ownership.md) records.

Both are additive to the core: adding a key to a box moves no geometry,
`resolve` keeps its signature, and a resolved view reports no anchor for a tree
containing none.

A view carries no scroll command, focus transition, or redraw hint. Scrolling
and focus stay ordinary model and message logic, while the current frame may
carry the resulting viewport origin and boundary behavior as a pure projection,
along with focused appearance. That projection transforms anchor rectangles
through the same coordinates and clips as cells; its exact visibility report
is defined in [`view-projection.md`](view-projection.md). Invalidation belongs
to `urushi-tui::Screen` and its cell diff. The division between interaction
ownership and one-frame expression is recorded in
[`tui-view-expressiveness.md`](tui-view-expressiveness.md).

The `urushi-tui-app` `Renderer` consumes `ResolvedView` and its anchored
rectangles directly rather than going through `ViewWidget`, because it needs
them from the same resolution that produced the cells and resolves exactly
once per frame. `ViewWidget` and `RatatuiWidget` stay public: a plain Ratatui
application drawing an Urushi view into a `Rect` it already owns is an audience
this design names, and it has no runtime to ask. The public surface of
`urushi-adapter-ratatui` serves that audience independently; both paths consume
the same `ResolvedView` semantics without sharing a foreign buffer type.

## Why the anchor is a keyed box rather than a node of its own

"Carries no geometry of its own" is the whole difficulty of making it a leaf. A
leaf must answer how large it is, and an anchor has no answer, because the two
uses want opposite ones. A leaf that takes whatever area reaches it gives a
widget its region, but then a cursor anchor under a bounded height takes that
height and the row around it grows to match — a key that changes the layout it
was meant to observe. A leaf that is always empty leaves the cursor right and
gives a widget nothing, unless a box around it is read as an exception, which
makes the leaf's meaning depend on its parent anyway.

The box is already the thing that answers how large. `BlockStyle` states cells,
fills, bounds, borders, padding, and margin, and one child is exactly what a
region contains. So the anchor is a block that also carries a key, and the
question never arises: it is sized by the rules every other box follows, and a
cursor is that box with nothing stated and no content, which resolves to no
cells and so cannot disturb what surrounds it.

The key belongs to the node rather than to `BlockStyle`. A style is an
immutable presentation value that a theme produces and call sites share, as
[`style-value-model.md`](style-value-model.md) records; a key is identity, and a
themed style carrying one would name the same region at every place it was
used. It is the `Key` of [`tui-application.md`](tui-application.md) rather than
a name of its own, because an application already identifies its subscriptions
that way and a region is one more thing it names.

## Why the one view tree

An earlier revision of that document set the type aside, on the ground that it
was a line-oriented collection of styled spans and so could not be a
full-screen layout tree. The premise is gone: `View` is a rectangle tree, its
area is an input to layout rather than a crop applied afterwards, and the ANSI
backend, the full-screen `Screen`, and the Ratatui adapter already consume the
one `ResolvedView` it produces. What remained was not a reason but its residue.

What the choice turns on now is what a second tree would cost. One layout model
spans plain output, prompts, and full-screen applications, so a component —
a list, a tree, a table — is written once and placed anywhere. A full-screen
tree of its own would duplicate the box model, put every component behind a
conversion boundary, and give one application two vocabularies for the same
rectangle. [`box-sizing.md`](box-sizing.md) records that a
full-screen surface, where the terminal's `Rect` is the primary fact of layout,
is the shape the area-driven model was built for; declining to use it there
would be declining the case it was designed for.

Rejected:

- **A TUI-only view tree, with `urushi::View` embeddable as a subtree.** It buys
  nothing the anchor does not, at the price of two layout models, two sets of
  alignment rules, and a conversion every component crosses.
- **A `view` function that composes Ratatui widgets directly.** Layout would
  belong to Ratatui, Urushi's box model would reach only the leaves, and the
  application would be written against Ratatui with Urushi as decoration.

## Why one anchor rather than two mechanisms

The anchor addresses two kinds of information a resolved rectangle of
graphemes cannot carry: the cell the terminal cursor belongs on, and a region
an embedded Ratatui widget draws into. They look unrelated, and each has an
obvious mechanism of its own — a cursor request on the view, a widget-bearing
node in the tree.

They are one problem. In both, the *position* is a layout outcome the
application cannot compute, and the *content* is not cells this crate produces.
An application cannot state a cursor cell from its model, because only the
layout pass knows where the text field landed; it cannot place a chart by
coordinate for the same reason. So the mechanism is a leaf that occupies a
rectangle, draws nothing, and reports where it landed — and both needs are
served by where it landed.

Two separate mechanisms would also cost the core its independence. A
widget-bearing node puts Ratatui inside `urushi`, which
[`architecture.md`](../architecture.md) forbids and which would make the
renderer-neutral tree neutral in name only. An anchor is a key and a rectangle:
the ANSI backend draws the blanks it resolved to, and a backend that knows what
belongs there fills it.

An anchor carries no geometry of its own. A sized region is written
structurally, as an anchor inside a block, which is the same answer
[`box-sizing.md`](box-sizing.md) gives to an exact content
dimension — the tree disambiguates two intents that a property would have had to
rank.

## Why scrolling and focus are not part of it

A projected region looks like the third member of the list and is not. Its
content *is* cells this crate produces, so `Viewport` remains in the ordinary
view tree and transforms those cells and anchors together. An anchor would
instead hand the caller a region the layout pass has stopped painting. The
coordinate operation is defined in
[`view-projection.md`](view-projection.md); choosing and updating its origin is
not part of the anchor or the viewport node.

Focus is not a view property at all. A focused block differs from an unfocused
one by the style the view function gives it, and the theme already carries
focused component roles. Making focus a node would put application semantics —
what is selected, what a key does next — inside a value whose whole purpose is
to describe a rectangle.

Partial redraw hints are refused on the same ground as the public damage model:
`Screen` diffs cell buffers, and a view that carried invalidation would be
describing the draw rather than the frame.

## Why the runtime draws the resolved view itself

`ViewWidget` resolves a view inside its own `render` and keeps nothing but the
cells. That is the right shape for a plain Ratatui application, which owns its
`Rect` and wants a rectangle drawn into it. It is the wrong shape for the
runtime, which needs the anchored rectangles from that same resolution and must resolve
exactly once per frame; going through the widget would mean resolving twice, or
resolving and then discarding what the cursor depends on.

So the `Renderer` consumes `ResolvedView` and its placements, while the widget
stays public in `urushi-adapter-ratatui` rather than being absorbed into the
application framework. Each output boundary translates the same resolution;
neither owns a second layout pass or view tree.
