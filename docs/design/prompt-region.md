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

The caller chooses how the prompt establishes its left edge `L`:

| Start | First-frame command | `L` |
| --- | --- | --- |
| `NewLine` (default) | `CarriageReturnLineFeed` | 0 |
| `CurrentLine` | `MoveToColumn(0)` | 0 |
| `CurrentPosition { column }` | none | `column` |

The caller supplies `column` because code that embeds a prompt after its own
output already knows where that output left the cursor. The prompt never asks
the terminal for its position.

The drawing width defaults to the terminal width remaining after `L`. A caller
may cap it, but cannot extend it past the terminal's right edge. Both the view
function and Resolve receive this effective width, so every line wraps against
the same rectangle.

If a resize makes a `CurrentPosition` column unavailable, the left edge is
clamped to the terminal's last column and the effective drawing width becomes
one cell. This keeps both cursor commands and layout inside the resized
terminal while preserving the configured column if the terminal widens again.

The chosen left edge applies to every row, not only the first. Restoring the
saved position reaches `L`, and a bare vertical move preserves it. This is what
keeps a prompt that begins mid-row rectangular rather than laying out only its
first row against a narrower area.

A prompt owns every row it draws from `L` to the end of the row. The drawing
width may be narrower than that owned suffix: it limits layout, not ownership.
Clearing therefore starts at `L` and continues to the row's end. Content to the
left of `L` survives; content to its right may not share a row with the prompt.

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
  <start command>               MoveDown(h1 - 1)
  LineFeed × (h - 1)            LineFeed × (h2 - h1)
  MoveUp(h - 1)                 MoveUp(h2 - 1)
  SavePosition                  SavePosition
```

For the default `NewLine` start, the carriage return and line feed reaches a
fresh row. That row is the region top, so the command costs no reserved row of
its own. `CurrentLine` instead moves to column zero of the current row, while
`CurrentPosition` saves the caller-declared position directly.

A bare line feed preserves the cursor column, so `LineFeed` followed by
`MoveUp` returns to the starting column without an explicit column command.

Between the first line feed of such a sequence and the `SavePosition` that ends
it, the region is **unanchored**: its extent cannot be established. Growth
happens only when the prompt's height changes, so this window does not exist on
the ordinary redraw path.

### The resize boundary

A saved origin and an owned row count are physical terminal state. A
primary-buffer resize invalidates their use as addresses for the prompt's
reflowed content. Resize is not region loss and does not invoke either lost
region continuation below. Its exact handling and rationale are defined in
[`prompt-resize.md`](prompt-resize.md).

### Losing a region

A write failure inside an unanchored window can leave the region's extent
unknown without a trustworthy logical cursor at which recovery can begin. The
region is then **lost**: `anchored`, `reserved_rows`, and `owned_rows` are
reset, and the previous physical rows are forgotten. Nothing is erased.

`drawn` is the one field a loss does not reset. It records that this prompt has
put something on screen at some point, which stays true however many regions
have since been abandoned.

A lost region has two continuations, and they are deliberately asymmetric.

**The prompt is still running.** The next frame re-establishes the region on
the cursor's current row at the selected left edge and proceeds as a first
frame. It does not emit a line feed first. Starting on the current row
overwrites only the suffix the replacement region is about to own.

**The prompt is finishing.** Cleanup restores the cursor to visible and emits a
carriage return and line feed, so subsequent output starts below the residue
rather than on top of it. It emits that line feed when `drawn` is true; a
prompt that failed before putting anything on screen must not leave a blank row
behind.

The two fields answer different questions, and a loss separates them:

- `owned_rows > 0` — whether there are rows to erase, and how many.
- `drawn` — whether anything of this prompt is on screen at all.

The gate is `drawn`, not `owned_rows`.

## Why an unlocatable region is abandoned rather than erased

After an unanchored partial write, the renderer does not know how much of the
planned frame reached the terminal or where every touched row now lies. Erasing
an inferred extent could destroy output the prompt does not own. Abandoning it
may leave visible residue, but that damage is bounded to prompt output; guessed
erasure is invisible and unbounded.

## Why the line feed after a loss is gated on `drawn`

`drawn` and `owned_rows` answer different questions — whether anything of this
prompt is on screen at all, and whether there are rows to erase — and a loss is
what separates them. A failed unanchored write followed by cancellation leaves
`owned_rows` at zero while residue may be on screen. Gating the closing line
feed on `owned_rows` would then skip it and let subsequent output land on top
of that residue, which is the exact outcome the finishing continuation exists
to prevent. That is why the gate is `drawn`, which a loss does not reset.

## Why drawing width does not narrow ownership

A width cap is useful even for a standalone prompt: a readable form need not
stretch across a two-hundred-column terminal. That layout choice does not imply
that unrelated output can safely share the rest of each row. Redraw, shrinking
content, cleanup, and partial-write recovery all need one uniform rule for what
the prompt may erase.

The selected rule is that every prompt row belongs to the prompt from its left
edge through the terminal's right edge. It preserves the uniform
position-clear-write form and the clear-before-write recovery invariant while
allowing content to the left of a `CurrentPosition` prompt to survive.

Allowing unrelated content to the right would require a different ownership
model: an exact-width erase operation, recovery state that tracks that narrower
extent, and a decision about residue outside it. A width cap alone does none of
those things.
