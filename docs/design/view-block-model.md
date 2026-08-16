# View and Block Split

This document records why the view model splits presentation into `TextStyle`
and `BlockStyle`, why views compose as a four-node tree resolved to
per-grapheme rows, and which alternatives were rejected. The contract itself is
defined in [`view-model.md`](../view-model.md) and
[`style-model.md`](../style-model.md).

## Why text cannot carry geometry

A `TextStyle` attached to text has no padding, no border, and no dimensions,
because a position that renders inline text cannot honor them. A row of text
occupies one row. Give one of those texts a border and it occupies three, and
the renderer folding that row has no correct answer: whatever it does, either
the row stops being a row or the box stops being a box.

Splitting the types makes that combination unrepresentable rather than
forbidden. A single style type carrying every property would need a rule — "do
not put a border on inline text" — enforced by whoever writes the next
component, and a rule that lives only in a reviewer's memory is not a
constraint. The one-way property conversion is the same guarantee at the
vocabulary level: geometry cannot reach a `TextStyle`, so the illegal
combination is unrepresentable rather than merely discouraged.

A position inside a row that legitimately needs geometry — a table cell aligning
its content within a column width, for instance — is a block. Expressing such a
cell as text carrying alignment properties forces its component to implement
that alignment itself, which puts a second layout implementation and a second
definition of width beside the first, and leaves the properties still attached
to the text for a renderer to apply a second time.

The split also makes geometry themeable on its own terms: a `panel` role is a
`BlockStyle`, served the way `ComponentStyles::list`, `tree`, and `table` serve
their dedicated style values, while text roles are `TextStyle` values.

## Why neither type is named `Style`

The two are peers — the model privileges neither — and the node names say which
is which at every call site: `Text(String, TextStyle)` beside
`Block(BlockStyle, View)`. An unqualified `Style` would also read as one thing
to a Lip Gloss reader and the other to a Ratatui reader, so it is the one name
that cannot mean the same thing to everyone.

## Why styles do not inherit

Inheritance is the obvious move once views nest — a panel with a surface
background would set it once and inner text would pick it up — and it is
deliberately not adopted. `TextStyle` is a complete immutable value, as
[`style-value-model.md`](style-value-model.md) settles; expressing "inherit
unless overridden" requires a patch representation, which Urushi does not have
as an operation anywhere, and would force modifiers from a bitset into an
add/remove set, since a bitset cannot express "do not inherit bold". The cost
is repeating a background across nested blocks. The benefit is that a style
means one thing wherever it is read.

## Why rows hold per-grapheme tokens

A renderer that receives per-grapheme tokens cannot split a grapheme cluster or
a wide character, because it never sees text below that granularity, and it
cannot disagree with the layout pass about a width, because the width it needs
is in the token. A backend that receives styled text runs re-derives both, and
two independent derivations of the same geometry drift.

## Why the layout pass never inspects text for escape sequences

Whether a string is plain text or already-rendered ANSI cannot be recovered
from the string, so no engine that accepts one can decide it; any attempt is a
heuristic. The property is therefore carried by the type — `RenderedBlock` —
and measured once, where it is declared.

## Rejected designs

- **One style type, with inline text reading only the properties it can honor.**
  The illegal combination stays constructible and the constraint stays a promise.
- **One style type, with geometry-bearing inline text promoted to a block during
  construction.** This makes the model total and nothing breaks silently, but a
  style then means different things depending on what it contains, and a theme
  serves values whose applicable half depends on where the caller attaches them.
  Type separation says the same thing without the classification step.
- **Geometry as bare parameters on `Block`, with no block style value.** A block
  needs colors for its border, padding, and alignment fill, so the parameters
  and a style travel together at every call site. Bundling them is the same
  thing with a name, and only the named value is themeable.
- **A line-oriented model — a line holding inline segments, one of which may be
  a block.** A block's child is itself a multi-row view, so the recursion
  appears regardless; the line spine then adds a second, weaker way to write a
  `Row`, and it keeps alive the intuition that a line contains spans, which is
  what makes a bordered inline element look constructible.
- **Naming one of the two `Style`.** Whichever one takes it becomes the default
  in the reader's mind, and the model has no default. It also forces a choice
  between Lip Gloss, where `Style` is the box, and Ratatui, where `Style` is the
  run of text — a name that means the opposite thing to half the audience.
- **A constraint-solving layout tree with flex-like grow and shrink.** Out of
  proportion to a sizing vocabulary of intrinsic size plus optional fixed and
  maximum dimensions. `Row`, `Column`, and `Block` cover it, and a solver can be
  added later without changing the node set.
