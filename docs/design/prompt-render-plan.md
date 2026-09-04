# The Prompt Render Plan

The command vocabulary a prompt's plan stage emits, the invariants that make a
command list canonical, the plan's recovery contract, where the cursor's
horizontal window is chosen, and how all of it is verified without a terminal.
[`inline-prompt-rendering.md`](../inline-prompt-rendering.md) states the stages
and the contract; the region the plan draws into is
[`prompt-region.md`](prompt-region.md).

## The rule

### Commands

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

There is no separate "clear to end of line" command. `ClearLine` means clearing
the owned part of a row: from the region's selected left edge to the terminal's
right edge. At column zero this clears the whole row; at a non-zero left edge it
preserves the content to the left. Every row is therefore handled uniformly:
position, `ClearLine`, `Write`. The drawing width may stop before the terminal's
right edge, but ownership and clearing do not; [`prompt-region.md`](prompt-region.md)
records that deliberate assumption.

### The plan and its recovery contract

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

For `step` to be total, every plan must begin either with the region
unestablished — the cursor is at the region top by definition — or with a
`RestorePosition` before any command that depends on position.

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
function as well as in Resolve.

### Verification

The plan stage is a pure function and is tested by comparing command lists and
presentation transitions, without a terminal:

```text
plan(framed, previous, geometry).commands == [
    HideCursor, CarriageReturnLineFeed, LineFeed, MoveUp(1), SavePosition,
    RestorePosition, ClearLine, Write(row0),
    RestorePosition, MoveDown(1), ClearLine, Write(row1),
    RestorePosition,
]
```

Recovery is tested by folding a prefix and asserting the resulting state, which
requires no failure injection.

Byte-level assertions belong to the executor and cover encoding only. Style
assertions belong to the renderer that produces `Style`, not to any stage here.

## Why recovery is a fold rather than per-command state

The earlier shape carried a recovery snapshot on every command and claimed rows
pessimistically before writing, so that cleanup after a partial write would cover
whatever had been drawn. Both the snapshot and the pessimistic claim exist to
answer one question — what reached the terminal — that a fold answers exactly, by
replaying the commands that succeeded.

The pessimistic claim is then not merely redundant but less accurate than the
high-water mark a fold maintains, which reflects the rows actually touched rather
than the rows a frame might have touched.

What the fold does not answer is the state after a *successful* frame, because
collapsing the owned extent to the new height is knowledge the planner has and
the commands do not carry. That is why the plan declares its own `next` rather
than deriving it, and why the two are described separately.

## Why the cursor's horizontal window is chosen before Resolve

The obvious arrangement is the opposite one: let Resolve keep the whole line and
let the Frame stage, which already windows vertically, take the horizontal
window too. The design tried that first and withdrew it.

The Frame stage cannot window what Resolve has already absorbed, so that
opposite arrangement requires resolving the prompt with no width bound. In the
view model an unbounded axis is what a measurement resolves under, so this does
not ask for a looser layout; it asks for the intrinsic size. The sizing and
overflow rules that need an area stop applying, which costs every row the
overflow policy its block declared: a validation error longer than the terminal
stops wrapping and becomes reachable only by its first screenful. One row's
cursor is not worth every row's layout. [`box-sizing.md`](box-sizing.md) and
[`overflow.md`](overflow.md) define the rules at stake.

Widening a single block instead fails on the same rules, which cap a box at the
area it is given.

What remains is to window the field's value before it becomes a `View`. The
knowledge is already there: a field holds its value and cursor, and the view
function that places it knows the width and the structure. The view model names
cutting a rendered string at a column a text-layer utility rather than a
box-model operation, so the windowing lands there.

The cost is that how much width a field ends up with is computed in the view
function as well as in Resolve. It is bounded: the view function is the prompt's
own code placing its own structure, and a view function is expected to hold the
size that `resolve` will be given.
