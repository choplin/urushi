# Inline Prompt Rendering: why this shape

The load-bearing choices in
[`inline-prompt-rendering.md`](../inline-prompt-rendering.md), which defines how
a prompt draws inline, were made against the alternatives below.

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
cursor is not worth every row's layout. [`view-model.md`](../view-model.md)
defines the resolution rule.

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

## Why normalization folds only what is inert

The canonical form of a run rests on the style's canonical fold, whose rule is
defined in [`style-model.md`](../style-model.md). Why the rule admits only inert
folds, why it is phrased around inertness rather than appearance, when the fold
happens, and what its one unclosable residue costs are recorded in
[`style-value-model.md`](style-value-model.md).

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
here it is the cheapest to reverse. Reinstating a non-zero left edge means
threading one value through the Frame and Plan stages.

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
