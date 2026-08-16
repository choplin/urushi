# Inline Prompt Rendering: why this shape

[`inline-prompt-rendering.md`](../inline-prompt-rendering.md) defines how a
prompt draws inline. This file records why each of its load-bearing choices was
made and what it was chosen over.

The design is shared with the sibling project noctui, which raises the cost of
changing any of these: a rule that moves has to move twice.

## Why a lost region is abandoned rather than erased

Treating every resize as region loss is broad. Both a width change, through
reflow, and a height reduction, by pushing content up, invalidate an absolute
origin, so a narrower rule would still cover nearly every resize. The design
accepts the breadth rather than guessing which resizes are survivable.

This is not only a safety judgement. It is also a decision to accept visible
residue: after a resize the previous frame's upper rows may stay on screen, and
nothing will ever remove them. The alternative — erasing rows whose position was
inferred rather than known — risks destroying output this prompt does not own,
which the user cannot recover. Residue is ugly and bounded; erasure is invisible
and unbounded.

The asymmetry between the two continuations follows from the same weighing.
While the prompt is still drawing it re-establishes on the cursor's own row and
overwrites what it can, because that space is about to be redrawn anyway. When
it is finishing it pushes below the remains instead, because anything it
overwrote there would stay overwritten.

## Why the cursor's horizontal window is chosen before Resolve

The obvious arrangement is the opposite one: let Resolve keep the whole line and
let Frame, which already windows vertically, take the horizontal window too.
That was the design's first answer and it was withdrawn.

Frame cannot window what Resolve has already absorbed, so the arrangement
requires resolving the prompt with no width bound. In the view model an
unbounded axis is what a measurement resolves under — see
[`view-model.md`](../view-model.md) — so this does not ask for a looser layout,
it asks for the intrinsic size. The sizing and overflow rules that need an area
stop applying, which costs every row the overflow policy its block declared: a
validation error longer than the terminal stops wrapping and becomes reachable
only by its first screenful. One row's cursor is not worth every row's layout.

Widening a single block instead fails on the same rules, which cap a box at the
area it is given.

What remains is to window the value before it becomes a `View`. The knowledge is
already there — a field holds its value and cursor, and the view function that
places it knows the width and the structure — and the view model names cutting a
rendered string at a column a text-layer utility rather than a box-model
operation, which is where this lands.

The cost is that how much width a field ends up with is computed in the view
function as well as in Resolve. It is bounded: the view function is the prompt's
own code placing its own structure, and a view function is expected to hold the
size that `resolve` will be given.

## Why normalization folds only what is inert

Two runs can look identical while holding different values, and the type rules
out most of that: one underline value rather than a modifier bit plus a colour,
a colour type with no reset spelling, one modifier set rather than an add set
and a subtract set. What the type cannot rule out is duplication *between*
fields, and there the design normalizes instead.

The admission rule is narrow on purpose. A value may be folded when dropping it
cannot change the output whatever the terminal does — an underline colour on a
run with no underline paints nothing, on every terminal. An equivalence that
holds only because a terminal is assumed to implement an attribute a particular
way is not a fold.

Reversed video is the case this excludes, and it is why the rule is phrased
around inertness rather than around appearance. Swapping a foreground and a
background looks the same as setting reverse, so an appearance-based rule would
admit it. But a dim attribute may apply to the declared foreground or to the
effective one depending on the terminal, so rewriting one form into the other
can change what is drawn. The selection row of a list is exactly where the two
spellings meet and exactly where redraws are most frequent, so the temptation is
real; it is still refused.

Folding after the style is final, rather than as it is built, is forced by the
style being an immutable value with a builder: any fold applied earlier is undone
by the next call that changes the foreground. A profile stage is one instance of
"final", not the reason for the rule.

One residue is not closable. When the foreground is absent its concrete colour is
the terminal's default, unknown here, so an underline colour equal to it cannot
be recognized. This weakens only the first purpose of a canonical form — not
redrawing a row that has not changed. The second — two implementations answering
the same input with the same value — holds regardless, because the rule is the
same on both sides. A canonical form owes a shared corpus determinism, not
minimality.

## Why the region starts at column zero

Anchoring the rectangle at whatever column the prompt happened to start on
propagates a starting column through every width calculation, every row's column
command, and the cursor position. It buys the ability to place a prompt on a row
that already contains output, which no prompt API asks for: a prompt owns the
rows it draws.

Dropping it also removes the first row's special case. With a left edge at column
zero no row can have content of another writer's to its left, so every row is
cleared the same way and the command vocabulary loses its clear-to-end-of-line
variant.

This is a deliberate reduction in capability and is the design's most reversible
decision. Reinstating a non-zero left edge means threading one value through the
Frame and Plan stages.

## Why recovery is a fold rather than per-command state

The earlier shape carried a recovery snapshot on every command and claimed rows
pessimistically before writing, so that cleanup after a partial write would cover
whatever had been drawn. Both exist to answer one question — what reached the
terminal — that a fold answers exactly, by replaying the commands that succeeded.

The pessimistic claim is then not merely redundant but less accurate than the
high-water mark a fold maintains, which reflects the rows actually touched rather
than the rows a frame might have touched.

What the fold does not answer is the state after a *successful* frame, because
collapsing the owned extent to the new height is knowledge the planner has and
the commands do not carry. That is why the plan declares its own `next` rather
than deriving it, and why the two are described separately.
