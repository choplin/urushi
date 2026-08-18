# The TUI View and the Anchor

A full-screen application's `view` function returns
[`urushi::view::View`](../../urushi/src/view/model.rs), the same tree a plain
CLI call site builds, and the node set carries one leaf — an anchor — for a
region whose position layout decides but whose content this crate does not
produce. [`tui-architecture.md`](../tui-architecture.md) states the shape; this
file holds the anchor's rule and records why it is that and not something else.

## The rule

An anchor is a leaf that occupies a rectangle and draws nothing: layout places
it like any other node, and `resolve` reports the rectangle it landed on beside
the resolved rows, for the caller that knows what belongs there to fill. An
anchor carries no geometry of its own, so a sized region is written
structurally, as an anchor inside a block. It names no backend type: an anchor
is a key and a rectangle, and a backend with nothing to put there draws the
blanks it resolved to.

Two things reach the frame that way:

- **Cursor placement.** A zero-sized anchor's reported origin is the cell the
  terminal cursor belongs on. The `Frame` of
  [`tui-terminal-ownership.md`](tui-terminal-ownership.md) carries the request
  for a draw; this is how an application states it.
- **Foreign widgets.** A sized anchor's rectangle is where the runtime's
  `Renderer` draws an embedded Ratatui widget, after the resolved cells are
  written. A `StatefulWidget`'s state stays in the application's model.

Both are additive to the core: the node set grows by one leaf, and a resolved
view reports an empty placement table for a tree containing no anchor.

A view carries no scroll offset, no focus, and no redraw hint. Scrolling is a
view-model question that composes on top of height clipping, focus stays
ordinary model and message logic, and invalidation belongs to Ratatui's cell
diff.

The runtime's `Renderer` consumes `ResolvedView` and its placements directly
rather than going through `ViewWidget`, because it needs the placements from
the same resolution that produced the cells and resolves exactly once per
frame. `ViewWidget` and `RatatuiWidget` stay public: a plain Ratatui
application drawing an Urushi view into a `Rect` it already owns is an audience
this design names, and it has no runtime to ask. The public surface of
`urushi-tui` therefore grows with the runtime rather than shrinking into it,
and the cell-writing path is shared between the widget and the `Renderer`.

Open representation choices: the anchor node's spelling, the key type, and the
placement-returning `resolve` signature.

## Why the one view tree

An earlier revision of that document set the type aside, on the ground that it
was a line-oriented collection of styled spans and so could not be a
full-screen layout tree. The premise is gone: `View` is a rectangle tree, its
area is an input to layout rather than a crop applied afterwards, and the ANSI
backend and the Ratatui backend already consume the one `ResolvedView` it
produces. What remained was not a reason but its residue.

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

Adopting the tree leaves two things a rectangle of graphemes cannot express: the
cell the terminal cursor belongs on, and a region an embedded Ratatui widget
draws into. They look unrelated, and each has an obvious mechanism of its own —
a cursor request on the view, a widget-bearing node in the tree.

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

A scrollable region looks like the third member of the list and is not. Its
content *is* cells this crate produces; what it needs is an offset into them,
which is a question about how the box model clips — height clips inside a closed
frame, and [`overflow.md`](overflow.md) records that scrolling composes on top
of that rule. Answering it through an anchor would hand the application a
region the layout pass has stopped reasoning about, which is exactly what a
viewport must not be.

Focus is not a view property at all. A focused block differs from an unfocused
one by the style the view function gives it, and the theme already carries
focused component roles. Making focus a node would put application semantics —
what is selected, what a key does next — inside a value whose whole purpose is
to describe a rectangle.

Partial redraw hints are refused on the same ground as the public damage model:
Ratatui diffs cell buffers, and a view that carried invalidation would be
describing the draw rather than the frame.

## Why the runtime draws the resolved view itself

`ViewWidget` resolves a view inside its own `render` and keeps nothing but the
cells. That is the right shape for a plain Ratatui application, which owns its
`Rect` and wants a rectangle drawn into it. It is the wrong shape for the
runtime, which needs the placements from that same resolution and must resolve
exactly once per frame; going through the widget would mean resolving twice, or
resolving and then discarding what the cursor and the embedded widgets depend
on.

So the `Renderer` consumes `ResolvedView` and its placements, and the widget
stays public rather than being absorbed. The two share a cell-writing path
rather than a resolution.
