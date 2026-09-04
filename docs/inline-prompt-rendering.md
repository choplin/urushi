# Inline Prompt Rendering

This document defines the architecture for drawing an interactive prompt
inline in a terminal: what a prompt owns on screen, the stages a frame passes
through, and the contract that keeps redraw and cleanup safe. Two topics have
files under [`design/`](design/): the owned region — how it is claimed,
anchored, lost, and released — in
[`design/prompt-region.md`](design/prompt-region.md), and the render plan — the
command vocabulary, the plan's recovery contract, and how it is verified — in
[`design/prompt-render-plan.md`](design/prompt-render-plan.md).

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
position. The caller chooses whether that origin is on a new line, at column
zero of the current line, or at a supplied current position. It may also cap the
drawing width. Only the owned suffix of rows inside that region may be erased
or rewritten. Terminal content above the origin, to the left of a non-zero
origin, and below the last materialized row belongs to whatever produced it.

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

A region can be *lost* — its extent unknown — after a terminal resize or a
write failure while the origin is being re-anchored. A lost region is
abandoned, never erased: the prompt re-establishes on the cursor's current row
if it is still running, and pushes below the residue if it is finishing. How
the origin is anchored, what a loss resets, and why residue is accepted over
erasure are defined in [`design/prompt-region.md`](design/prompt-region.md).

## Presentation state

A prompt tracks the state below while it draws:

```text
InlinePresentation {
  anchored       : bool        // a valid origin is saved
  reserved_rows  : int         // rows materialized below the origin
  owned_rows     : int         // rows cleanup must erase
  rows           : [Row]       // last drawn content, for diffing
  drawn          : bool        // this prompt has put something on screen
}
```

`reserved_rows` never decreases while a prompt runs; `owned_rows` is the
extent cleanup must erase. `drawn` is set by the first successful write and
never cleared: every other field describes the region currently being tracked
and is reset when that region is lost; `drawn` describes the screen, which a
loss does not undo. The exact meaning of each field, and which decisions each
one gates, are defined in [`design/prompt-region.md`](design/prompt-region.md).

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
A prompt's usefulness depends on these; they are not layout details. Keeping
them in their own stage stops them from being entangled with wrapping and
clipping, and keeps the plan stage free of prompt semantics. Frame windows rows
vertically only: the *horizontal* window that follows a text cursor is chosen
before Resolve, by the view function that places the field, so that Resolve
runs with the real available area and every row keeps its overflow policy. Why
that is so is recorded in
[`design/prompt-render-plan.md`](design/prompt-render-plan.md).

**Plan** and **Execute** are defined by the plan's contract, below.

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

Because a row is the unit the plan compares to decide whether to redraw, the
Frame stage emits runs in a canonical form, defined in
[`design/style-canonical-form.md`](design/style-canonical-form.md).

## The plan and its recovery contract

```text
InlineRenderPlan {
  commands : [Command]
  next     : InlinePresentation   // adopted only after every command succeeds
}
```

The plan is a list of commands drawn from the prompt's own small vocabulary —
cursor visibility, save and restore of the origin, relative movement, clear
line, line feed, and write — and never anything outside it: no alternate
screen, no full-screen clear, no absolute cursor addressing. Every row is
handled uniformly — position, clear, write — from the region's selected left
edge. A narrower drawing width changes layout but does not narrow the owned
suffix that clearing and recovery may touch.

The plan carries no per-command state. On success the executor adopts
`plan.next`; on failure at command *k*, the state is the fold of the commands
that succeeded, which is exactly what reached the terminal. The accuracy of
that recovery rests on one invariant:

> Every row is cleared before it is written, in the same frame.

The command vocabulary and its canonical-list invariants, the fold, how a
buffered executor observes failure, and how the plan is verified are defined
in [`design/prompt-render-plan.md`](design/prompt-render-plan.md).
