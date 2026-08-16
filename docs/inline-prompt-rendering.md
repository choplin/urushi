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

In both, the region is **lost**: `anchored`, `reserved_rows`, and `owned_rows`
are reset, and the previously drawn rows are forgotten. Nothing is erased. A
prompt's remains left on screen are a smaller harm than erasing rows belonging
to another writer.

`drawn` is the one field a loss does not reset. It records that this prompt has
put something on screen at some point, which stays true however many regions
have since been abandoned.

A lost region has two continuations, and they are deliberately asymmetric.

**The prompt is still running.** A resize arrives while the user is typing, so
the prompt must keep drawing. The next frame re-establishes the region at the
cursor's current row: it begins with `MoveToColumn(0)` and proceeds as a first
frame. It does **not** emit a line feed first. Starting on the current row
overwrites it, so the only rows left behind are those above the cursor's row at
the moment of loss — none at all for a prompt whose cursor sits on its first
row, which is the common case.

**The prompt is finishing.** Cleanup restores the cursor to visible and emits a
carriage return and line feed, so subsequent output starts below the remains
rather than on top of them. It emits that line feed when `drawn` is true; a
prompt that failed before putting anything on screen must not leave a blank row
behind.

The gate is `drawn`, not `owned_rows`. The two answer different questions and a
loss separates them: a resize immediately followed by a cancellation leaves
`owned_rows` at zero while remains are on screen, and gating on `owned_rows`
would then skip the line feed and let subsequent output land on top of them —
the exact outcome this continuation exists to prevent.

- `owned_rows > 0` — whether there are rows to erase, and how many.
- `drawn` — whether anything of this prompt is on screen at all.

The layer above is responsible for coalescing resize events. Dragging a window
edge produces a stream of them, and re-establishing the region once per event
would multiply whatever remains are left behind. One re-establishment per
settled size is the requirement; the plan stage cannot enforce it.

#### The cost this accepts

Treating every resize as region loss is broad. Both a width change, through
reflow, and a height reduction, by pushing content up, invalidate an absolute
origin, so a narrower rule would still cover nearly every resize. The design
accepts the breadth rather than guessing.

This is not only a safety judgement. It is also a decision to accept visible
residue: after a resize the previous frame's upper rows may stay on screen, and
nothing will ever remove them. The alternative — erasing rows whose position
was inferred rather than known — risks destroying output this prompt does not
own, which is unrecoverable for the user. Residue is ugly and bounded; erasure
is invisible and unbounded.

## Stages

Rendering is four stages. Only the third is shared logic; it is the stage this
design exists to make identical.

| Stage | Input → output | Concern |
| --- | --- | --- |
| Resolve | `View` + `Limits` → `ResolvedView` | Generic. Measure, wrap, and clip content to a box. No prompt policy. |
| Frame | `ResolvedView` + policy → `FramedView` | Prompt-specific. Window a bounded viewport onto the resolved content, vertically and horizontally. |
| Plan | `FramedView` + `InlinePresentation` + geometry → `InlineRenderPlan` | Pure. Decide which commands reach the terminal. **Shared.** |
| Execute | `InlineRenderPlan` → terminal | I/O and byte encoding only. |

**Resolve** is not prompt-specific and is shared with any other consumer of the
view model.

**Frame** carries the policy a prompt needs when its content does not fit:
scroll the viewport so the focused row stays visible, keep the text cursor
visible within its row, and reinstate the validation error and the help line
when scrolling has pushed them out of view. A prompt's usefulness depends on
these; they are not layout details. Keeping them in their own stage stops them
from being entangled with wrapping and clipping, and keeps the plan stage free
of prompt semantics.

**Plan** and **Execute** are described below.

### A prompt resolves unbounded in width

Following the cursor is a windowing decision, and windowing belongs to Frame.
That forces a constraint upstream: **Frame cannot recover content that Resolve
already clipped away.** If a line is clipped to the viewport width before Frame
sees it, and the cursor sits beyond that width, the text it would need to scroll
into view is gone.

A prompt therefore resolves with no width bound and lets Frame take the
horizontal window. Height is still bounded at Resolve, because Frame selects
rows rather than reflowing them.

This is a constraint on how a prompt *calls* Resolve, not a change to Resolve.
`Limits` keeps both bounds for consumers that want them; a prompt simply does
not use the width bound.

The alternative — making the cursor an input to Resolve so it can clip around it
— would put prompt policy inside the stage this design defines as generic, and
would make every other consumer of the view model carry a parameter that means
nothing to it.

The cost is resolving a long line at full length every frame. Prompt rows are
few and short-lived, so this is not on a path where it matters; a viewport that
windows a very large document would need a different arrangement.

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

### Runs have a canonical form

A row is the unit the plan compares to decide whether to redraw, and the unit a
shared conformance corpus compares across implementations. Both break if the
same visible row can be represented by more than one sequence of runs. The Frame
stage therefore emits runs in a canonical form:

- adjacent runs with equal styles are merged, greedily and left to right;
- no run is empty; and
- a run's style is the style **as it will be emitted** — resolved for the
  output profile, in a representation where equal appearance means equal value.

Without the third rule a row can differ structurally while rendering
identically, and every frame redraws every row. It has two distinct failure
modes, and a style type must rule out both:

- **Redundant spellings.** If a default color can be written either as "absent"
  or as an explicit reset, or if modifiers are carried as an add set and a
  subtract set, one appearance has several values. The style a run carries must
  be a normalized form with neither. `TextStyle` — an optional foreground, an
  optional background, and one modifier set, over a color type with no reset
  variant — is that form, which is why no separate "effective" style type is
  needed alongside it.
- **Capability degradation.** Two colors a terminal profile collapses to the
  same output are equal on screen and unequal in a logical style. The style a
  run carries must therefore already be resolved against the profile, not left
  logical for the executor to resolve.

This matters most where rows are aggregated from grapheme-level content on each
frame, because the aggregation is what establishes the form.

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

These variants need not be the whole of an implementation's command type. A
project that also drives a full-screen surface may keep one terminal-command
vocabulary and make these a subset of it, rather than maintaining a second
vocabulary for prompts; whether that applies depends on what else the project
owns below the prompt. What must hold either way is that **the plan stage emits
nothing outside this set** — no alternate screen, no full-screen clear, no
absolute cursor addressing — and that the exclusion is asserted, not assumed.

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
  drawn          : bool        // this prompt has put something on screen
}
```

`anchored` is a single predicate covering both "no origin has been saved yet"
and "the origin is stale because we are mid-growth". Cleanup treats them
identically, so they are one field.

`reserved_rows` never decreases while a prompt runs. `owned_rows` is the extent
cleanup must erase; after a successful frame it equals that frame's height,
because stale rows were cleared during the frame.

`drawn` is set by the first successful write and is never cleared for the
lifetime of the prompt. Every other field describes the region currently being
tracked and is reset when that region is lost; `drawn` describes the screen,
which a loss does not undo.

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

### Recovery depends on clearing a row before writing it

`fold` applies only commands that succeeded, so a `Write` that fails partway
does not raise `owned_rows` — yet its bytes may already be on screen. That row
is nevertheless covered, because the uniform row form places a `ClearLine` on
the row immediately before the `Write`, and the successful `ClearLine` has
already raised the mark.

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

- the command variants a prompt plan emits, and their meaning;
- the two canonicalization invariants on command lists;
- the canonical form of runs within a row;
- `InlinePresentation` and its fields' invariants;
- `step`, and the fold that derives recovery state;
- the clear-before-write invariant that recovery depends on;
- the plan function: same input, same command list, same `next`;
- the region rules, including reserve-before-anchor, region loss, and both
  continuations after a loss; and
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
