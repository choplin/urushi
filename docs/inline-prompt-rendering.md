# Prompt Rendering

This document defines the architecture for rendering a blocking interactive
prompt: the terminal surfaces a caller can choose, what each presentation owns,
and the default inline rendering path. Four topics have files under
[`design/`](design/): resize ownership and its safety boundary in
[`design/prompt-resize.md`](design/prompt-resize.md), the inline owned region —
how it is claimed, anchored, lost, and released — in
[`design/prompt-region.md`](design/prompt-region.md), field presentation and
viewport degradation in
[`design/prompt-field-presentation.md`](design/prompt-field-presentation.md),
and the render plan — the command vocabulary, the plan's recovery contract,
and how it is verified — in
[`design/prompt-render-plan.md`](design/prompt-render-plan.md).

## Display modes

A prompt has two display modes:

| Mode | Surface | Resize behavior | Selection |
| --- | --- | --- | --- |
| `Inline` | A region in the primary buffer | Terminal reflow, then error or explicit viewport clear | Default |
| `AlternateScreen` | The whole temporary viewport | The prompt lays out and redraws the viewport | Explicit |

The modes share form state, field behavior, validation, themes, and outcomes.
The caller chooses the terminal behavior it needs; capability detection does
not silently move a form between surfaces. Surface-specific options belong to
the selected mode, so an inline start position cannot be accepted and then
ignored by an alternate-screen presentation.

### Inline

`Inline` keeps the active form and its submitted state in the primary buffer
and normal scrollback. It owns only a region of rows, never the screen, and
must leave content outside that region untouched. Terminal soft wrapping owns
how the displayed snapshot reflows when the terminal is resized. The prompt
does not reconstruct or erase that snapshot from its previous physical
geometry, so this mode does not promise deterministic post-resize layout.

`InlineResizePolicy::ReturnError` is the default: the prompt restores the
terminal session and returns `RunError::Resized`, leaving the reflowed snapshot
untouched. `InlineResizePolicy::ClearViewportAndRedraw` is an explicit
destructive opt-in. It clears the visible primary-buffer viewport, places the
cursor at a known origin, and redraws current form state at the new size. It
does not clear scrollback or enter the alternate screen.

Submission keeps the final answered prompt and moves subsequent output below
it. Cancellation and failure erase only a region whose ownership can still be
established; rows that can no longer be located are never erased by inference.

### AlternateScreen

`AlternateScreen` enters a temporary screen for the duration of the form and
owns the complete viewport. It resolves the current form against the available
area and redraws the viewport after every resize, so its layout remains under
application control.

Submission, cancellation, I/O failure, and panic restoration all leave the
alternate screen and restore the primary buffer and terminal modes acquired by
the session. The transient form is not replayed into scrollback. The returned
form outcome and any subsequent durable output belong to the caller.

The exact resize boundary and the alternatives it rules out are defined in
[`design/prompt-resize.md`](design/prompt-resize.md).

## The inline rendering path

The remainder of this document defines the stages and ownership contract of
the default `Inline` mode. Inline rendering must:

- draw a prompt into a region of terminal rows without owning the screen;
- redraw on every keystroke without visible flicker;
- keep the focused row visible when content exceeds the terminal height;
- release the region cleanly on submission, cancellation, and failure;
- leave terminal content outside the region untouched unless the caller
  explicitly selects the viewport-clearing resize policy, including when a
  terminal write fails midway; and
- allow command ordering and state transitions to be tested without a terminal.

## The owned region

A prompt draws inline and never enters the alternate screen. Its ordinary plan
never clears the terminal. It owns a *region*: a run of rows anchored at a
saved cursor position. The caller chooses whether that origin is on a new line,
at column zero of the current line, or at a supplied current position. It may
also cap the drawing width. Only the owned suffix of rows inside that region
may be erased or rewritten. Terminal content above the origin, to the left of
a non-zero origin, and below the last materialized row belongs to whatever
produced it. The sole exception is the explicit
`ClearViewportAndRedraw` resize policy, which abandons the unlocatable region
and takes the visible viewport before establishing a new one.

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

A region can be *lost* when a write fails while its physical extent cannot be
established. Such a region is abandoned, never erased. Resize is a separate
boundary: it hands the displayed snapshot to the primary terminal buffer and
follows the selected inline resize policy rather than inferring a new origin.
The resize boundary is defined in
[`design/prompt-resize.md`](design/prompt-resize.md); origin anchoring and
unlocatable-write recovery are defined in
[`design/prompt-region.md`](design/prompt-region.md).

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

**Frame** carries the policy a prompt needs when its content does not fit. Each
field supplies one `View` body plus prompt-owned semantic regions keyed to
anchors inside it; Resolve lays that body out once, and Frame uses the resolved
regions to retain the active question, error, focus, and nearby Select choices
before optional descriptions, help, or complete inactive fields. The model and
degradation order are defined in
[`design/prompt-field-presentation.md`](design/prompt-field-presentation.md).
Keeping this policy in Frame stops it from being entangled with wrapping and
clipping, and keeps both generic `View` and the plan stage free of prompt
semantics. Frame windows rows vertically only: the *horizontal* window that
follows a text cursor is chosen before Resolve, by the view function that
places the field, so that Resolve runs with the real available area and every
row keeps its overflow policy. Why that is so is recorded in
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
