# View and Block Split

This document records why the view model splits presentation into `TextStyle`
and `BlockStyle`, why views compose as a four-node tree resolved to
per-grapheme rows, why sizing resolves under the available area the way it
does, and which alternatives were rejected. The contract itself is defined in
[`view-model.md`](../view-model.md) and [`style-model.md`](../style-model.md).

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

## Why the area is an input to layout

The previous model applied bounds after assembly: `max_width` and the outer
`Limits` truncated the finished rectangle row by row — Lip Gloss's
`applyBorder`-then-`Truncate` order, inherited with its artifacts. Two boxes
occupying the same six columns then meant different things:

```
width(4) + border      -> 6x4        max_width(6) + border  -> 6x3
  ┌────┐                               ┌─────
  │abcd│                               │abcde
  │ef  │                               └─────
  └────┘
```

The first assembles a content box and derives the frame from it; the second
cuts the assembled rectangle, severing the derivation that placed the border,
so the right edge vanishes. A border derives from the content box it
surrounds: any bound that arrives after the box is assembled can only cut the
frame open. Making `Available` an input — sizes flow down once, resolved
sizes flow up once — is what lets the frame close at whatever size the bound
forces, and it is the shape a full-screen surface needs anyway, where the
terminal's `Rect` is the primary fact of layout.

CSS reached the same separation — sizing, track distribution, and overflow as
three vocabularies — and this model borrows those concepts, not their
implementation:

| Urushi | CSS | |
|---|---|---|
| `width(Cells)` / `min_` / `max_` | `width`, `min-width`, `max-width` (border-box) | margin outside, as in CSS |
| auto (absent `width`) | `max-content` sizing | |
| `Fill(n)` | Grid's `fr` track | named after Ratatui's `Constraint::Fill` |
| `Available` | the containing block | |
| `Overflow::Wrap`/`Clip(marker)` | `white-space`, `overflow`, `text-overflow` | one enum: a terminal has no paint layer to make `overflow` its own axis |
| min-content floor | `min-content` | grapheme atomicity, not word atomicity |
| — | `overflow: visible` | impossible: no paint layer; a resolved rectangle is the output |
| — | percentages | ratios are `Fill` weights; a percentage length is a possible `Length` extension |
| view-function branching | media queries | conditional structure stays outside layout |

## Why every sizing property measures the outer box

Distribution and `Available` necessarily measure the box that sits in the
area — frame included — so a content-basis `width` would leave the vocabulary
measuring two different boxes, which is precisely the asymmetry the old
`width`/`max_width` pair suffered from, generalized. In a terminal the outer
box is also the visible one: a border occupies cells a reader counts, and
`┌────┐` is a six-cell box to anyone looking at it. The intent "size my
content" does not need a property at all: it is a size on an unframed inner
block, where outer and content coincide, so the tree disambiguates the two
intents structurally and no precedence rule between two width properties ever
arises.

## Why `Fill`, and why nothing is renegotiated

Without `Fill`, "a fixed sidebar and main takes the rest" — the most ordinary
full-screen layout — is inexpressible under any non-negotiating rule, which
is why this much is reclaimed from the rejected constraint-solving design.
The boundary against that design stays sharp: a size is decided from an area
and pure measurements of the subtree below it, results flow up once, and the
only iteration is the numeric freeze loop over one axis's floors when an area
is too small. No cross-axis coupling, no size revised once decided, no
propagation of one sibling's resolution into another's content. Measuring a
subtree more than once does not cross that line — a measurement is a question
about that subtree alone, so its answer cannot depend on what a sibling
resolved to, which is exactly what a solver's iteration does.

Slack is deliberately not renegotiated either: a `Fill` child capped by its
own `max_width` leaves the remainder unused rather than triggering
redistribution. Capping a group is an enclosing block's `max_width`, and the
container that resolves below its area is placed by ordinary alignment — the
same structure as CSS's `max-width` with auto margins. The distribution rule
stays one sentence, and the cost is an addition at the call site.

`Fill` weights divide the remainder directly, so equal weights are an equal
split. Per-child grow factors — CSS flex rather than CSS Grid — were rejected
for exactly this: a grow factor distributes slack *on top of* intrinsic
sizes, so two `grow(1)` children of unequal content do not split an area
50/50, and the flex `basis: 0` trick exists to cancel what the factor did.

## Why overflow is the application's choice

Whether excess content wraps, clips, or ends in an ellipsis is presentation
policy, and fixing any one of them in the library would be wrong for two of
the three real cases — prose wraps, a viewport clips, a status-bar path
ellipsizes. What the library fixes is the invariant underneath the choice:
the frame closes at the used size, and no overflow policy can cut it. Height
has no wrap analogue, and clipping inside a closed frame is a viewport's
behavior, so height clips and scrolling composes on top; a vertical marker is
a possible extension.

The marker a cut ends with is a parameter of clipping, not a third policy.
Marking a cut does not absorb overflow differently — it is the same cut, said
out loud — and fixing the glyph would fix `…` for terminals that cannot show
it, which is the problem `Border::ASCII` already exists to solve on the same
axis. Lip Gloss draws the same line: `ansi.Truncate(s, width, tail)` takes the
tail as a string and passes `""` for a silent cut. Because the marker is
measured in cells like any other content, parameterizing it also removes a
hidden assumption from the budget — a three-cell `...` costs three, where a
fixed `…` had let the implementation subtract one. Cutting an already-*rendered* string at a
column stays a text-layer utility so that "frames close" remains an
invariant of the box model rather than a default.

## Why conditional structure stays outside layout

"Hide the sidebar when the terminal is narrow" and "stack vertically below 80
cells" are decisions about which tree to build. CSS cannot express them in
layout properties either — they live in media queries, a layer outside
layout — and a TEA-style view function already holds the size `resolve` will
be given, so the branch costs nothing. What the model owes the application
is computable breakpoints — public `measure`, declared minimums,
`frame_size` — not trees that rewrite themselves. A declarative
priority-collapse vocabulary would re-open negotiation for a case the branch
already covers.

## Scope across implementations

noctui realizes this same view model (`src/view/resolve.mbt`), currently as
the two stages Urushi's previous model had: intrinsic resolution then a
`Limits` clip. The repositioning of the area — `Available` as layout input,
frames closing at used size, the degenerate-only crop — is a decision about
the shared model and applies to both implementations. The vocabulary is
Urushi-side work for now: noctui's block style has no box model yet (no
`width`, no `max_width`, no border), so nothing there is renamed; when its
box model lands, it starts from this vocabulary rather than migrating to it.

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
- **A constraint-solving layout tree with flex-like grow and shrink.** Still
  rejected, with the boundary drawn above: what was reclaimed is `Fill`; what
  stays rejected is negotiation — per-child grow *and* shrink factors,
  cross-axis coupling, and re-layout of resolved children.
- **Sizing by post-hoc crop** — the previous model, where `max_width` and
  `Limits` truncated the assembled rectangle. Recorded above: a bound that
  arrives after assembly can only cut the frame open. The crop survives only
  as the degenerate safety net.
- **A `truncate` property alongside a layout-participating `max_width`.** It
  would have preserved the open-frame artifact as an opt-in. Cutting rendered
  output is a text-utility concern; keeping it out of the box model keeps
  "frames close" an invariant rather than a default.
- **A fixed ellipsis glyph, with `Clip` and `Ellipsis` as sibling policies.**
  Recorded above: the marker is what varies, not the absorption rule, so the
  taxonomy put a terminal-capability choice out of the application's reach and
  hard-coded a one-cell budget. The cost of carrying it is that `Overflow` and
  `BlockStyleProperty` are no longer `Copy`.
- **Content-box sizing, or a second `content_width` property.** Recorded
  above: two boxes in one vocabulary, or two width properties needing a
  precedence rule; the unframed-inner-block idiom expresses the intent
  structurally.
- **Per-child `grow` factors.** Recorded above: slack on top of intrinsic
  sizes cannot express an equal split.
- **Fixed height as a minimum** — the previous rule, where content taller
  than `height(n)` grew the box. It made `height` unable to state "this tall,
  period", which a viewport needs, and treated the axes asymmetrically for no
  reason wrapping does not already cover. Overflow absorbs the excess inside
  the frame, and growth-on-content is what auto sizing is for.
