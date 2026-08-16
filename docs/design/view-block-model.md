# View Model

This document defines how Urushi represents composed terminal output: the two
style types, the view tree, how the tree resolves to a rectangle, and what each
renderer receives. It is a design criterion for changes to `view`, `style`,
`render`, and the Ratatui adapter.

## Rule

Presentation splits into two values, and geometry belongs to only one of them:

```rust
/// Everything a terminal can express about a run of text.
pub struct TextStyle { fg, bg, modifiers }

/// A rectangle, and the style filling the geometry it creates.
pub struct BlockStyle {
    padding, margin,
    border, border_top, border_right, border_bottom, border_left,
    border_foreground, border_background,
    width, height, max_width, max_height,
    align, vertical_align,
    text: TextStyle,
}
```

A view is a tree of four nodes:

```rust
pub enum View {
    Text(String, TextStyle),
    Block(BlockStyle, Box<View>),
    Row(VerticalAlign, Vec<View>),
    Column(Align, Vec<View>),
}
```

- `Text` is a leaf: plain text and the style applied to it.
- `Block` applies one `BlockStyle` around exactly one child.
- `Row` places children side by side.
- `Column` stacks children.

These are the four things terminal output does: carry text, put a box around
something, place things beside each other, stack them. Components construct
views through `View::text`, `View::block`, `View::row`, and `View::column`;
there is one way to express each node.

### Why text cannot carry geometry

A `TextStyle` attached to text has no padding, no border, and no dimensions,
because a position that renders inline text cannot honor them. A row of text
occupies one row. Give one of those texts a border and it occupies three, and
the renderer folding that row has no correct answer: whatever it does, either
the row stops being a row or the box stops being a box.

Splitting the types makes that combination unrepresentable rather than
forbidden. A single style type carrying every property would need a rule — "do
not put a border on inline text" — enforced by whoever writes the next
component, and a rule that lives only in a reviewer's memory is not a
constraint.

A position inside a row that legitimately needs geometry — a table cell aligning
its content within a column width, for instance — is a block. Expressing such a
cell as text carrying alignment properties forces its component to implement
that alignment itself, which puts a second layout implementation and a second
definition of width beside the first, and leaves the properties still attached
to the text for a renderer to apply a second time.

The split also makes geometry themeable on its own terms: a `panel` role is a
`BlockStyle`, served the way `ComponentStyles::list`, `tree`, and `table` serve
their dedicated style values, while text roles are `TextStyle` values.

Neither type is named `Style`. The two are peers — the model privileges
neither — and the node names say which is which at every call site:
`Text(String, TextStyle)` beside `Block(BlockStyle, View)`. An unqualified
`Style` would also read as one thing to a Lip Gloss reader and the other to a
Ratatui reader, so it is the one name that cannot mean the same thing to
everyone.

`TextStyle` alone produces no rectangle. `TextStyle::paint` wraps text in its
SGR scope; `BlockStyle::render` produces a rectangle.

## Composition

Every node resolves to a rectangle.

- A `Text` node's rectangle is its own lines, padded to the width of its widest
  line.
- `Row(align, children)`: width is the sum of child widths, height is the
  greatest child height. A shorter child is offset vertically by `align` and
  padded with blank rows of its own width.
- `Column(align, children)`: width is the greatest child width, height is the
  sum of child heights. A narrower child's rows are padded to the full width
  according to `align`.
- Padding introduced by composition carries the child's own fill style — a
  `Block`'s `BlockStyle::text`, so a background-colored block keeps its
  background across alignment rows — and an empty `TextStyle` for a `Text` child.

Alignment belongs to the `Row` or `Column`, not to its children: a child cannot
align itself inside a height that is only known once its siblings are measured.

Two alignment biases coexist and are not unified. `Row` and `Column` place the
odd extra row of a `Center` alignment *above* the shorter child;
`BlockStyle`'s `vertical_align` places it *below* the padded content block, as
[`style-model.md`](../style-model.md) specifies. Both match Lip Gloss in their
own position, so they are separate rules.

A block's style does not flow into its child. Inheritance is the obvious move
once views nest — a panel with a surface background would set it once and inner
text would pick it up — and it is deliberately not adopted. `TextStyle` is a
complete immutable value, and [`style-model.md`](../style-model.md) rejects
implicit parent-to-child flow; expressing "inherit unless overridden" requires a
patch representation, which Urushi does not have as an operation anywhere, and
would force modifiers from a bitset into an add/remove set, since a bitset
cannot express "do not inherit bold". A `Block`'s style therefore applies to the
geometry that block creates — border glyphs, padding, alignment fill — and each
child carries its own complete value. The cost is repeating a background across
nested blocks. The benefit is that a style means one thing wherever it is read.

## Resolution

One layout pass turns a tree into a rectangle, and every renderer consumes that
rectangle:

```rust
pub struct Size { width: usize, height: usize }

/// An outer clip applied after intrinsic layout.
pub struct Limits { max_width: Option<usize>, max_height: Option<usize> }

/// One grapheme, the width it occupies, and its logical style.
pub struct StyledGrapheme { symbol: String, width: usize, style: TextStyle }

pub struct ResolvedView { size: Size, rows: Vec<Vec<StyledGrapheme>> }

pub fn measure(view: &View) -> Size;
pub fn resolve(view: &View, limits: Limits) -> ResolvedView;
```

Every row's widths sum to `size.width`, and the row count equals `size.height`.
Styles in a `ResolvedView` are logical: a `TerminalProfile` is applied when a
renderer serializes it, so terminal capability resolution stays at the output
boundary.

Rows hold per-grapheme tokens rather than styled text runs. A renderer then
cannot split a grapheme cluster or a wide character, because it never sees text
below that granularity, and it cannot disagree with the layout pass about a
width, because the width it needs is in the token. A backend that receives text
runs re-derives both, and two independent derivations of the same geometry drift.

`BlockStyle::render` is the single-block case of the same pass: it resolves
`Block(style, Text(content, style.text))` with unbounded limits. There is one
implementation of the box model in the workspace.

Clipping is layered. `BlockStyle`'s `max_width` and `max_height` crop a block
during resolution; `Limits` — a terminal width, or a Ratatui `Rect` — is applied
last, so the smaller bound wins.

## Plain text and rendered output

The layout pass never inspects text for escape sequences.

Whether a string is plain text or already-rendered ANSI cannot be recovered from
the string, so no engine that accepts one can decide it; any attempt is a
heuristic. The property is therefore carried by the type and measured once,
where it is declared:

```rust
pub struct RenderedBlock { text: String, size: Size }

impl RenderedBlock {
    /// Adopts a string produced elsewhere. The caller asserts it is rendered
    /// output, and this is the one place ANSI-aware measurement happens.
    pub fn from_ansi(text: impl Into<String>) -> Self;
    pub fn size(&self) -> Size;
    pub fn as_str(&self) -> &str;
}
```

- A `Text` node holds plain text. Its width is grapheme display width.
- `BlockStyle::render` and `AnsiRenderer::render` return a `RenderedBlock`,
  which implements `Display`.
- `join_horizontal` and `join_vertical` take and return `RenderedBlock`. They
  compose rendered output — text this crate did not lay out, or output destined
  straight for a writer — and each input carries the size it was measured at, so
  they never re-measure ANSI text.
- A `RenderedBlock` does not re-enter the view tree. Content that participates
  in layout is expressed as a tree.

Passing escape sequences to a `Text` node has a defined consequence rather than
an undefined one: their graphemes are measured as ordinary text, so the block
comes out deterministically too wide. Runtime detection is not part of the
contract.

This is what lets the two backends agree: both consume a `ResolvedView` whose
contents are graphemes with known widths and logical styles, containing no
escape sequences at all.

## Backends

- `AnsiRenderer` resolves a view and serializes each row, coalescing adjacent
  graphemes of equal effective style into one SGR scope.
- `urushi-tui`'s `ViewWidget` derives `Limits` from the target `Rect`, resolves
  the view, converts each grapheme's logical `TextStyle` through `RatatuiStyle`, and
  writes cells. `RatatuiWidget` draws a single `BlockStyle` through the same
  path.
- The core crate holds no Ratatui dependency: `ResolvedView` and `Limits` are
  Urushi values.

A backend does not compute geometry. Drawing each block by handing its `Rect` to
a widget that lays the box out again would put a second box model in the
adapter, and it could not express a block whose child is a view rather than a
string.

## Display width

Display width is decided once, in the layout pass, using the shared `text`
implementation, and carried per grapheme in the `ResolvedView`. No component and
no renderer defines its own notion of display width, and none re-measures one
the layout pass already decided.

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

## Required verification

Changes to this model must test:

- a row mixing plain text with a bordered block resolves to one rectangle whose
  height is the block's, in both backends;
- a shared corpus of views asserted against both the ANSI string and the Ratatui
  buffer, so the backends cannot diverge silently;
- `BlockStyle::render` results for border side combinations, fixed and maximum
  dimensions, alignment, and wrapping;
- `Row` and `Column` alignment including the odd-row `Center` bias, and
  `BlockStyle`'s opposite bias;
- wide characters and grapheme clusters surviving composition and clipping in
  both backends;
- that theme, prompt, and component call sites resolve the style type each
  position requires.
