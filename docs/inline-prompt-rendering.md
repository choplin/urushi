# Inline Prompt Rendering

This document defines the target architecture for drawing an interactive prompt
inline in a terminal.
It is a normative design for planned work, not a description of code that is
already implemented.
[`architecture.md`](architecture.md) remains the source of truth for the
repository as it exists today.

This design is shared with the sibling project noctui. The stages, the state,
the command vocabulary, and the rules below are one design realized twice.
Implementations may differ in how a call is spelled, in error representation,
and in module layout; they may not differ in which types exist, what each
operation computes, or how the state machine behaves.

## Goals

Inline prompt rendering must:

- draw a prompt into a region of terminal rows without owning the screen;
- redraw on every keystroke without visible flicker;
- keep the focused row visible when content exceeds the terminal height;
- release the region cleanly on submission, cancellation, and failure;
- leave terminal content outside the region untouched in every case, including
  when a terminal write fails midway; and
- allow command ordering and state transitions to be tested without a terminal.

## The owned region

A prompt draws inline. It never enters the alternate screen and never clears
the terminal. It owns a *region*: a run of rows anchored at a saved cursor
position. Only rows inside that region may be erased or rewritten. Terminal
content above the origin, and below the last materialized row, belongs to
whatever produced it.

Three rules govern the region.

**Claiming.** The region grows only downward, and only by emitting bare line
feeds that scroll new rows into existence. It never shrinks while a prompt is
running, because rows already scrolled into existence cannot be given back. A
region never exceeds the terminal height.

**Releasing.** A submitted prompt keeps its final rows and moves the terminal
below them, so the answered prompt remains in the scrollback. Every other
outcome erases the region and returns to the origin, leaving no trace.

**Recovery.** A terminal write can fail midway. Cleanup after a failure must
erase every row the failed frame could have touched, and no row outside the
region. Where the region's extent cannot be established, cleanup must erase
nothing.

### The origin is anchored after scrolling, never before

The saved origin is an absolute screen position (`DECSC`). A line feed at the
bottom of the screen scrolls the display: the region's content moves up by one
row while the saved position does not. The origin is then stale, and restoring
to it lands below the region.

The design confines this to a known window rather than correcting it.

A frame that must grow the region emits every line feed **first**, returns to
the region top, and only then saves the origin. All scrolling therefore happens
before the anchor is taken, so a saved origin is never invalidated by later
commands in the same frame. The first frame does not save an origin before
reserving rows; it has nothing to return to and needs nothing.

```text
first frame, height h        growth from h1 to h2
  HideCursor                   RestorePosition
  LineFeed × (h - 1)           MoveDown(h1 - 1)
  MoveUp(h - 1)                LineFeed × (h2 - h1)
  SavePosition                 MoveUp(h2 - 1)
                               SavePosition
```

A bare line feed preserves the cursor column, so `LineFeed` followed by
`MoveUp` returns to the starting column without an explicit column command.

Between the first line feed of such a sequence and the `SavePosition` that ends
it, the region is **unanchored**: its extent cannot be established. Growth
happens only when the prompt's height changes, so this window does not exist on
the ordinary redraw path.

### A region that cannot be located is abandoned, not erased

Two situations leave the region's extent unknown: a write failure inside an
unanchored window, and a terminal resize, which may reflow existing content and
invalidate both the origin and the row count.

In both, the region is **lost**. Cleanup must not erase. It restores the cursor
to visible, emits a carriage return and line feed so that subsequent output
starts on a fresh row, and forgets the region. A prompt's remains left on
screen are a smaller harm than erasing rows belonging to another writer.

## Stages

Rendering is four stages. Only the third is shared logic; it is the stage this
design exists to make identical.

| Stage | Input → output | Concern |
| --- | --- | --- |
| Resolve | `View` + `Limits` → `ResolvedView` | Generic. Measure, wrap, and clip content to a box. No prompt policy. |
| Frame | `ResolvedView` + policy → `FramedView` | Prompt-specific. Choose which rows a bounded viewport shows. |
| Plan | `FramedView` + `InlinePresentation` + geometry → `InlineRenderPlan` | Pure. Decide which commands reach the terminal. **Shared.** |
| Execute | `InlineRenderPlan` → terminal | I/O and byte encoding only. |

**Resolve** is not prompt-specific and is shared with any other consumer of the
view model.

**Frame** carries the policy a prompt needs when its content does not fit:
scroll the viewport so the focused row stays visible, and reinstate the
validation error and the help line when scrolling has pushed them out of view.
A prompt's usefulness depends on these; they are not layout details. Keeping
them in their own stage stops them from being entangled with wrapping and
clipping, and keeps the plan stage free of prompt semantics.

**Plan** and **Execute** are described below.

## The row unit

A `FramedView` is a sequence of rows. A row is a sequence of styled runs:

```text
StyledRun {
  text          : string
  display_width : int
  style         : Style
}
```

A run carries its measured display width because the plan stage must know how
far a write advances the cursor, and because rows are compared for equality
during diffing. Width is measured once, during Resolve, and never recomputed.

Two boundaries follow from this shape:

- **Theme and role resolution happens above the plan.** A run carries a
  resolved `Style`, not a semantic role. The plan stage has no access to a
  theme.
- **SGR encoding happens below the plan.** A run carries text and a style, not
  escape sequences. The executor turns a run into bytes.

Runs rather than individual cells: the plan writes whole rows and never
addresses a cell, so per-grapheme granularity would cost an allocation per
character on the redraw path without being used. Grapheme-level resolution
remains internal to Resolve.

## Commands

```text
Command =
  | HideCursor
  | ShowCursor
  | SavePosition
  | RestorePosition
  | MoveUp(n)
  | MoveDown(n)
  | MoveRight(n)
  | MoveToColumn(column)
  | ClearLine
  | LineFeed
  | CarriageReturnLineFeed
  | Write(Row)
```

The vocabulary is the prompt's own, not a terminal library's. Reserving a row
is a bare line feed, which terminal libraries generally do not model as a
command, and the vocabulary is the artifact kept identical across
implementations.

`LineFeed` and `CarriageReturnLineFeed` are distinct because their purposes
differ: the first materializes a row inside the region, the second releases the
region. Relative cursor movement stops at the terminal boundary instead of
scrolling, so neither can be replaced by a `MoveDown`.

Two invariants make a command list canonical, so that two implementations
given the same input produce the same list:

- **A movement command with a count of zero is never emitted.** A caller that
  computes a zero distance omits the command.
- **A write is always followed by an absolute reposition** — `RestorePosition`
  or `MoveToColumn` — **before any relative movement.** The cursor position
  after a write that reaches the right margin is terminal-dependent; an
  absolute reposition removes the ambiguity.

Columns are zero-based and are converted at encoding time.

There is no "clear to end of line". Because the region starts at column zero
and spans the full width, no row contains content that is not the prompt's, so
every row is cleared the same way. Every row is therefore handled uniformly:
position, `ClearLine`, `Write`. Implementations that special-case the first row
are compensating for a non-zero left edge, which this design does not have.

## Presentation state

```text
InlinePresentation {
  anchored       : bool        // a valid origin is saved
  reserved_rows  : int         // rows materialized below the origin
  owned_rows     : int         // rows cleanup must erase
  rows           : [Row]       // last drawn content, for diffing
}
```

`anchored` is a single predicate covering both "no origin has been saved yet"
and "the origin is stale because we are mid-growth". Cleanup treats them
identically, so they are one field.

`reserved_rows` never decreases while a prompt runs. `owned_rows` is the extent
cleanup must erase; after a successful frame it equals that frame's height,
because stale rows were cleared during the frame.

## The plan and its recovery contract

```text
InlineRenderPlan {
  commands : [Command]
  next     : InlinePresentation   // adopted only after every command succeeds
}
```

The plan carries no per-command state. Recovery is derived instead:

```text
step(state, command) -> state        // total; effect of one command
```

- **On success**, the executor adopts `plan.next`. The planner declares it,
  because only the planner knows that a completed frame collapses `owned_rows`
  to the new height.
- **On failure** at command *k*, the state is `fold(previous, commands[..k])`.
  This is exactly what reached the terminal.

The executor is therefore:

```text
for command in plan.commands:
    write(command)?
    state = step(state, command)
state = plan.next
```

`step` maintains `anchored`, `reserved_rows`, `owned_rows`, and the cursor's
row within the region. It does not maintain `rows`: after a failure the drawn
content is indeterminate, cleanup erases the region regardless, and the field
is reset.

This replaces claiming rows pessimistically before writing. A high-water mark
maintained by `step` is both simpler and more accurate than over-stating the
extent up front, because it reflects what was actually touched.

For `step` to be total, every plan must begin either with the region
unestablished — the cursor is at the region top by definition — or with a
`RestorePosition` before any command that depends on position.

## The region starts at column zero

A prompt region is a rectangle whose left edge is column zero. If the cursor is
not at column zero when a prompt starts, the prompt emits a carriage return and
line feed to reach a fresh row before establishing the region.

The alternative — anchoring the rectangle at the column where the prompt
happened to start — propagates a starting column through every width
calculation, every row's column command, and the cursor position. It buys the
ability to place a prompt on a row that already contains output, which no
prompt API asks for: a prompt owns the rows it draws.

This is a deliberate reduction in capability and is the design's most
reversible decision. Reinstating a non-zero left edge means threading one
value through the Frame and Plan stages.

## What must be identical

Identical across implementations:

- the `Command` variants and their meaning;
- the two canonicalization invariants on command lists;
- `InlinePresentation` and its fields' invariants;
- `step`, and the fold that derives recovery state;
- the plan function: same input, same command list, same `next`;
- the region rules, including reserve-before-anchor and region loss; and
- the concept names above, modulo module qualification.

Free to differ:

- byte encoding and the writer interface;
- error representation;
- how a view is expressed before Resolve; and
- module and package layout.

## Verification

The plan stage is a pure function and is tested by comparing command lists and
presentation transitions, without a terminal:

```text
plan(framed, previous, geometry).commands == [
    HideCursor, LineFeed, MoveUp(1), SavePosition,
    ClearLine, Write(row0),
    RestorePosition, MoveDown(1), ClearLine, Write(row1),
    RestorePosition,
]
```

Recovery is tested by folding a prefix and asserting the resulting state, which
requires no failure injection.

Byte-level assertions belong to the executor and cover encoding only. Style
assertions belong to the renderer that produces `Style`, not to any stage here.

Because the plan stage is identical across implementations, a shared corpus of
`(previous, framed, geometry) → (commands, next)` cases can be executed by
both. That corpus is worth introducing once both sides implement this design;
it cannot be written before the stages and the row unit agree.
