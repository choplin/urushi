# Inline Prompt Rendering

This document defines the architecture for drawing an interactive prompt
inline in a terminal.

## Goals

Inline prompt rendering must:

- draw a prompt into a region of terminal rows without owning the screen;
- redraw on every keystroke without visible flicker;
- keep the focused row visible when content exceeds the terminal height;
- release the region cleanly on submission, cancellation, and failure;
- leave terminal content outside the region untouched in every case, including
  when a terminal write fails midway; and
- allow command ordering and state transitions to be tested without a terminal.

## Presentation state

A prompt tracks the state below while it draws. The region, its origin, and the
frame that draws it are defined in the sections that follow.

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

## The owned region

The region rules are what keep terminal content outside the prompt untouched,
including after a failed write.

A prompt draws inline. It never enters the alternate screen and never clears
the terminal. It owns a *region*: a run of rows anchored at a saved cursor
position. A region row spans the full terminal width, so the prompt owns a
row's end as well as its start. Only rows inside that region may be erased or
rewritten. Terminal content above the origin and content below the last
materialized row belong to whatever produced it.

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

### Anchoring the origin

The saved origin is an absolute screen position saved with DEC Save Cursor
(`DECSC`). A line feed at the bottom of the screen scrolls the display: the region's content moves up by one
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

The breadth of treating every resize as a loss, the residue this accepts, why
the two continuations are asymmetric, and why the gate is `drawn` are argued in
[`design/inline-prompt-rendering.md`](design/inline-prompt-rendering.md).

## Stages

Rendering is four stages. Only the third is pure logic with no I/O and no
prompt policy of its own; it is the stage this design exists to pin down.

| Stage | Input → output | Concern |
| --- | --- | --- |
| Resolve | `View` + `Available` → `ResolvedView` | Generic. Size content into a box under the available area. No prompt policy. |
| Frame | `ResolvedView` + policy → `FramedView` | Prompt-specific. Choose which rows a bounded viewport shows, and emit their runs in canonical form. |
| Plan | `FramedView` + `InlinePresentation` + geometry → `InlineRenderPlan` | Pure. Decide which commands reach the terminal. |
| Execute | `InlineRenderPlan` → terminal | I/O and byte encoding only. |

**Resolve** is not prompt-specific and is shared with any other consumer of the
view model defined in [`view-model.md`](view-model.md).

**Frame** carries the policy a prompt needs when its content does not fit:
scroll the viewport so the focused row stays visible, and reinstate the
validation error and the help line when scrolling has pushed them out of view.
A prompt's usefulness depends on these; they are not layout details. Keeping them in their own stage stops them from
being entangled with wrapping and clipping, and keeps the plan stage free of
prompt semantics.

### The cursor's horizontal window is chosen before Resolve

A text cursor can sit beyond the viewport width, and following it there is
horizontal scrolling. The window that follows it is chosen before a `View`
exists. A text field holds its value and its cursor; the view function that
places that field knows the available width and the structure it is placing the
field into. It puts the **visible window of the value** into the `View`, with
the cursor's column expressed within that window. Choosing that window is a
text-layer operation, not a box-model one.

Consequences:

- Resolve is called with the real available area, so every row absorbs its own
  overflow under the policy its block declares, and sizing behaves as the view
  model specifies.
- Frame windows rows and does not reflow them. It has no horizontal concern.
- The cursor reaches Plan as a position inside an already-windowed row.

This puts one layout fact — how much width a field ends up with — in the view
function as well as in Resolve. What that was chosen over, and what it costs, is
argued in
[`design/inline-prompt-rendering.md`](design/inline-prompt-rendering.md).

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
- **Select Graphic Rendition (SGR) encoding happens below the plan.** A run
  carries text and a style, not escape sequences. The executor turns a run into
  bytes.

Runs rather than individual cells: the plan writes whole rows and never
addresses a cell, so per-grapheme granularity would cost an allocation per
character on the redraw path without being used. Grapheme-level resolution
remains internal to Resolve.

### Runs have a canonical form

A row is the unit the plan compares to decide whether to redraw. That
comparison breaks if the same visible row can be represented by more than one
sequence of runs. The Frame stage therefore emits runs in a canonical form:

- adjacent runs with equal styles are merged, greedily and left to right;
- no run is empty; and
- a run's style is the style **as it will be emitted** — resolved for the
  terminal profile, in a representation where equal appearance means equal
  value, and in the canonical form
  [`style-model.md`](style-model.md#canonical-form) defines.

The style a run carries is a `TextStyle`, defined in
[`style-model.md`](style-model.md). Run aggregation is where the form is
established, because rows are aggregated from grapheme-level content on each
frame.

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
command.

These variants need not be the whole of the crate's command type: the
full-screen runtime may keep one terminal-command vocabulary and make these a
subset of it, rather than maintaining a second vocabulary for prompts. What
must hold either way is that **the plan stage emits nothing outside this set**
— no alternate screen, no full-screen clear, no absolute cursor addressing —
and that the exclusion is asserted, not assumed.

`LineFeed` and `CarriageReturnLineFeed` are distinct because their purposes
differ: the first materializes a row inside the region, the second releases the
region. Relative cursor movement stops at the terminal boundary instead of
scrolling, so neither can be replaced by a `MoveDown`.

Two invariants make a command list canonical, so that the same input always
produces the same list:

- **A movement command with a count of zero is never emitted.** A caller that
  computes a zero distance omits the command.
- **A write is always followed by an absolute reposition** — `RestorePosition`
  or `MoveToColumn` — **before any relative movement.** The cursor position
  after a write that reaches the right margin is terminal-dependent; an
  absolute reposition removes the ambiguity.

Columns are zero-based and are converted at encoding time.

There is no "clear to end of line". Because the region starts at column zero
and a region row spans the full terminal width, no row contains content that is
not the prompt's, so every row is cleared the same way. Every row is therefore
handled uniformly: position, `ClearLine`, `Write`. Implementations that
special-case the first row are compensating for a non-zero left edge, which this
design does not have.

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

Failure is observed where the writer reports it. An executor that buffers and
flushes several commands at once observes it at the flush, so *k* is the first
command of the flush that failed, and the state is the fold up to the last flush
that succeeded. Commands of the failed flush may have reached the terminal in
part; whatever they drew is residue the next frame or cleanup does not know
about, and it is accepted on the same ground as the residue a lost region
leaves: understating what was touched risks residue, overstating it risks
erasing rows the prompt does not own.

`step` maintains `anchored`, `reserved_rows`, `owned_rows`, and the cursor's
row within the region. It does not maintain `rows`: after a failure the drawn
content is indeterminate, cleanup erases the region regardless, and the field
is reset.

This replaces per-command recovery state and a pessimistic claim made before
writing; [`design/inline-prompt-rendering.md`](design/inline-prompt-rendering.md) records why both were dropped.

### Clearing before writing

`fold` applies only commands that succeeded, so a `Write` that fails partway
does not raise `owned_rows` — yet its bytes may already be on screen. That row
is nevertheless covered, because the uniform row form places a `ClearLine` on
the row immediately before the `Write`, and the successful `ClearLine` has
already raised `owned_rows` to cover that row.

The accuracy of recovery therefore rests on an invariant, not on `step` alone:

> Every row is cleared before it is written, in the same frame.

Dropping the clear for rows believed to be empty, or for content believed to be
appended rather than replaced, would be a plausible optimization and would
silently leave residue after a failed write. Any change to the uniform row form
must preserve this invariant or replace it with something that covers a
partially completed write.

For `step` to be total, every plan must begin either with the region
unestablished — the cursor is at the region top by definition — or with a
`RestorePosition` before any command that depends on position.

## The region starts at column zero

A prompt region is a rectangle whose left edge is column zero. If the cursor is
not at column zero when a prompt starts, the prompt emits a carriage return and
line feed to reach a fresh row before establishing the region.

This is a deliberate reduction in capability.
[`design/inline-prompt-rendering.md`](design/inline-prompt-rendering.md) records
what it was chosen over and what reversing it would cost.

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
