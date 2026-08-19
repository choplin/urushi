# The Prompt's Owned Region

How an inline prompt claims, anchors, loses, and releases the run of terminal
rows it owns, and why. [`inline-prompt-rendering.md`](../inline-prompt-rendering.md)
states the region rules and the presentation state; this file holds their
exact form and the reasoning behind them.

## The rule

### Presentation state

```text
InlinePresentation {
  anchored       : bool        // a valid origin is saved
  reserved_rows  : int         // rows materialized below the origin
  owned_rows     : int         // rows cleanup must erase
  rows           : [Row]       // last drawn content, for diffing
  drawn          : bool        // this prompt has put something on screen
}
```

`anchored` is a single predicate covering both "no origin has been saved yet"
and "the origin is stale because the region is mid-growth". Cleanup treats them
identically, so they are one field.

`reserved_rows` never decreases while a prompt runs. `owned_rows` is the extent
cleanup must erase; after a successful frame it equals that frame's height,
because stale rows were cleared during the frame.

`drawn` is set by the first successful write and is never cleared for the
lifetime of the prompt. Every other field describes the region currently being
tracked and is reset when that region is lost; `drawn` describes the screen,
which a loss does not undo.

### The region

A region row spans the full terminal width, so the prompt owns a row's end as
well as its start, and its left edge is column zero: if the cursor is not at
column zero when a prompt starts, the prompt emits a carriage return and line
feed to reach a fresh row before establishing the region.

Only the terminal knows which column the cursor is on, and asking costs a round
trip: a status request goes out, and the reply arrives on the terminal's input
beside the user's keystrokes. **The request belongs on the stream the prompt
draws to**, so that a prompt whose output reaches the terminal can always ask,
whatever the program has done with its other streams. Reading the reply is the
input layer's job, because that reply and the user's keystrokes arrive on one
stream and only one reader may consume it.

When the answer cannot be obtained, the prompt assumes column zero. The row a
prompt starts on is almost always fresh, and assuming otherwise would put a
blank row above every prompt whose terminal declines to answer.

### Anchoring the origin

The saved origin is an absolute screen position saved with DEC Save Cursor
(`DECSC`). A line feed at the bottom of the screen scrolls the display: the
region's content moves up by one row while the saved position does not. The
origin is then stale, and restoring to it lands below the region.

The design confines this to a known window rather than correcting it.

A frame that must grow the region emits every line feed **first**, returns to
the region top, and only then saves the origin. All scrolling therefore happens
before the anchor is taken, so a saved origin is never invalidated by later
commands in the same frame. The first frame does not save an origin before
reserving rows; it has nothing to return to and needs nothing.

```text
first frame, height h         growth from h1 to h2
  HideCursor                    RestorePosition
  CarriageReturnLineFeed?       MoveDown(h1 - 1)
  LineFeed × (h - 1)            LineFeed × (h2 - h1)
  MoveUp(h - 1)                 MoveUp(h2 - 1)
  SavePosition                  SavePosition
```

The first frame's carriage return and line feed is the one that reaches a fresh
row, and it is emitted only when the prompt did not start at column zero. The
row it reaches is the region top, so it costs no reserved row of its own.

A bare line feed preserves the cursor column, so `LineFeed` followed by
`MoveUp` returns to the starting column without an explicit column command.

Between the first line feed of such a sequence and the `SavePosition` that ends
it, the region is **unanchored**: its extent cannot be established. Growth
happens only when the prompt's height changes, so this window does not exist on
the ordinary redraw path.

### Losing a region

Two situations leave the region's extent unknown: a write failure inside an
unanchored window, and a terminal resize, which may reflow existing content and
invalidate both the origin and the row count.

In both, the region is **lost**: `anchored`, `reserved_rows`, and `owned_rows`
are reset, and the previously drawn rows are forgotten. Nothing is erased.

`drawn` is the one field a loss does not reset. It records that this prompt has
put something on screen at some point, which stays true however many regions
have since been abandoned.

A lost region has two continuations, and they are deliberately asymmetric.

**The prompt is still running.** A resize arrives while the user is typing, so
the prompt must keep drawing. The next frame re-establishes the region at the
cursor's current row: it begins with `MoveToColumn(0)` and proceeds as a first
frame. It does **not** emit a line feed first. Starting on the current row
overwrites it, so the only rows left behind are those above the cursor's row at
the moment of loss.

**The prompt is finishing.** Cleanup restores the cursor to visible and emits a
carriage return and line feed, so subsequent output starts below the residue
rather than on top of it. It emits that line feed when `drawn` is true; a
prompt that failed before putting anything on screen must not leave a blank row
behind.

The two fields answer different questions, and a loss separates them:

- `owned_rows > 0` — whether there are rows to erase, and how many.
- `drawn` — whether anything of this prompt is on screen at all.

The gate is `drawn`, not `owned_rows`.

The layer above is responsible for coalescing resize events. Dragging a window
edge produces a stream of them, and re-establishing the region once per event
would multiply the residue left behind. One re-establishment per settled size is
the requirement; the plan stage cannot enforce it.

## Why a lost region is abandoned rather than erased

Treating every resize as region loss is broad. Both a width change, through
reflow, and a height reduction, by pushing content up, invalidate an absolute
origin, so a narrower rule would still cover nearly every resize. The design
accepts the breadth rather than guessing which resizes are survivable.

This is not only a safety judgment. It is also a decision to accept visible
residue: after a resize the previous frame's upper rows may stay on screen, and
nothing will ever remove them. The alternative — erasing rows whose position was
inferred rather than known — risks destroying output this prompt does not own,
which the user cannot recover. Residue is ugly and bounded; erasure is invisible
and unbounded.

The two ways a prompt continues after a loss — still drawing, and finishing — are
treated asymmetrically, and that follows from the same weighing. While the prompt
is still drawing, it re-establishes on the cursor's own row and overwrites what
it can, because that space is about to be redrawn anyway. When it is finishing,
it pushes below the remains instead, because anything it overwrote there would
stay overwritten.

## Why the line feed after a loss is gated on `drawn`

`drawn` and `owned_rows` answer different questions — whether anything of this
prompt is on screen at all, and whether there are rows to erase — and a loss is
what separates them. A resize immediately followed by a cancellation leaves
`owned_rows` at zero while residue is on screen. Gating the closing line feed on
`owned_rows` would then skip it and let subsequent output land on top of that
residue, which is the exact outcome the finishing continuation exists to
prevent. That is why the gate is `drawn`, which a loss does not reset.

## Why the region starts at column zero

Anchoring the region at whatever column the prompt happened to start on
propagates a starting column through every width calculation, every row's column
command, and the cursor position. It buys the ability to place a prompt on a row
that already contains output, which no prompt API asks for: a prompt owns the
rows it draws.

Dropping it also removes the first row's special case. With a left edge at column
zero, no row can have another writer's content to its left, so every row is
cleared the same way and the command vocabulary loses its clear-to-end-of-line
variant.

This is a deliberate reduction in capability, and among the decisions recorded
for the prompt it is the cheapest to reverse. Reinstating a non-zero left edge
means threading one value through the Frame and Plan stages.
