# View and Block Split

The view model splits primitive styling into `TextStyle` and `BlockStyle` and
composes semantic-free layout primitives into a tree resolved to per-grapheme
rows and placements. This document records why it does each of those, why
semantic components are lowered before this boundary, and which alternatives
were rejected. The contracts are defined in
[`component-model.md`](../component-model.md),
[`view-model.md`](../view-model.md), and
[`style-model.md`](../style-model.md); the sizing rules the tree resolves under
have their own files — [`box-sizing.md`](box-sizing.md),
[`area-sharing.md`](area-sharing.md), [`overflow.md`](overflow.md),
[`layout-resolution.md`](layout-resolution.md), and
[`rendered-output-measurement.md`](rendered-output-measurement.md).

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
that alignment itself. That puts a second layout implementation and a second
definition of width beside the first, and the properties stay attached to the
text for a renderer to apply a second time.

The split also makes geometry themeable on its own terms: a `panel` role is a
`BlockStyle`, while text roles are `TextStyle` values. Component presentations
may contain either kind, but are not themselves style values.

## Why semantic components are lowered before View

`View` is the shared language of layout and rendering, not a registry of
component kinds. A concrete component presentation is the last layer that
knows whether a row is a Table header, a marker belongs to a List item, or two
nodes are joined by a Graph edge. Its `compose` operation translates that
meaning into generic primitives before `resolve` sees the tree.

Keeping `View` primitive-only lets a component change independently of the
resolver and lets Urushi and Noctui share one closed layout model without
duplicating every component in it. It also prevents one convenient primitive,
such as Grid, from becoming a semantic intermediate representation every
component is forced through.

## Why neither type is named `Style`

The two are peers — the model privileges neither — and the node names say which
is which at every call site: `Text(String, TextStyle)` beside
`Block(BlockStyle, View)`. An unqualified `Style` would also read as one thing
to a Lip Gloss reader and the other to a Ratatui reader, so it is the one name
here that cannot mean the same thing to both audiences.

## Why styles do not inherit

Inheritance is the obvious move once views nest — a panel with a surface
background would set it once and inner text would pick it up — and it is
deliberately not adopted. `TextStyle` is a complete immutable value, as
[`style-value-model.md`](style-value-model.md) settles; expressing "inherit
unless overridden" requires a patch representation (which Urushi does not have
as an operation anywhere) and would force modifiers from a bitset into an
add/remove set, since a bitset cannot express "do not inherit bold". The cost
is repeating a background across nested blocks. The benefit is that a style
means one thing wherever it is read.

## Why rows hold per-grapheme tokens

A renderer that receives per-grapheme tokens cannot split a grapheme cluster or
a wide character, because it never sees text below that granularity, and it
cannot disagree with the layout pass about a width, because the width it needs
is in the token. A renderer that receives styled text runs re-derives both, and
two independent derivations of the same geometry drift.

## Why the two alignment biases are not unified

`Row` and `Column` place the odd extra row of a `Center` alignment above the
shorter child; `BlockStyle`'s `vertical_align` places it below the padded
content block. Both match Lip Gloss in their own position, so they are separate
rules.

## Why a backend does not compute geometry

Drawing each block by handing its `Rect` to a widget that lays the box out
again would put a second box model in the adapter, and it could not express a
block whose child is a view rather than a string. Both backends therefore
consume the one `ResolvedView`, and the Ratatui adapter turns its target
`Rect` into `Available` rather than into a second layout.

## Rejected designs

- **Semantic component nodes such as `View::Table` or `View::Graph`.** They
  move the presentation algorithm into the resolver, couple the closed layout
  enum to an open set of component concepts, and make every backend-facing
  implementation learn component semantics.
- **Component presentations that receive `Available`.** A nested component's
  share does not exist until its siblings are resolved. Giving composition the
  root area lets it pre-wrap and pre-pad against the wrong rectangle.

- **One style type, with inline text reading only the properties it can honor.**
  The illegal combination stays constructible and the constraint stays a promise.
- **One style type, with geometry-bearing inline text promoted to a block during
  construction.** This makes the model total, and nothing breaks silently, but a
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
