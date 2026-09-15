# Styled Text

How one plain-text flow carries several complete `TextStyle` values without
letting source segmentation change grapheme, wrapping, or clipping semantics.
[`view-model.md`](../view-model.md) gives the surrounding tree and resolved-scene
model; this document owns the construction invariant and its consequences.

## The values

`TextSpan` is caller input. It owns one string fragment and the complete style
the caller assigns to that fragment. `String` and `&str` convert to a
default-style span, keeping the ordinary unstyled case concise. A span by itself
does not claim that either end is a grapheme boundary.

`StyledText` is the invariant carrier and the sole payload of `View::Text`. It
stores the concatenated source once, private ranges that assign styles to it,
and an optional tab policy used only when the value participates in layout.
Those ranges are canonical:

- every range is non-empty and ranges cover the source in order;
- every range endpoint is a grapheme boundary of the complete source;
- adjacent ranges never carry equal styles.

Empty input and empty source are valid and still lay out as one empty row.
Empty spans disappear. Adjacent spans with equal styles merge. Public
inspection returns source slices and styles, not mutable ranges, so a caller
cannot invalidate the representation after construction.

## Construction decides graphemes once

`StyledText::try_from_spans` first concatenates every fragment, then segments
the complete source according to extended grapheme-cluster rules. It validates
each non-empty input boundary against that result. A boundary inside a cluster
returns `StyledTextError` with the input span index and byte offset.

This order matters because a fragment is not enough context to decide its edge.
For example, `"e"` followed by `"\u{301}"` becomes the single grapheme
`"e\u{301}"`; assigning two styles there has no cell-level meaning. A combining
mark supplied as the only source is still a valid grapheme. The validity is a
property of the concatenated source and its proposed boundaries, not of a
fragment in isolation.

The constructor does not normalize Unicode code points. Canonically equivalent
source strings remain distinct source strings. Grapheme segmentation establishes
cell-safe boundaries; it does not rewrite caller data.

Horizontal tab and newline are the two source controls admitted by
`StyledText`. Other controls remain invalid: unlike a tab whose two supported
paths are explicit, an escape sequence or cursor movement would make both
style scoping and layout indeterminate. Construction checks this contract in
all build profiles, so direct rendering cannot become a raw ANSI escape hatch.

## Tabs split at the layout boundary

One tab has a fixed-width layout replacement. `TabPolicy` holds that width and
an optional printable marker string. With no marker the complete replacement
is spaces. With a marker, the marker inherits the tab's `TextStyle` and its
right side is padded with spaces until the configured width is reached. A
zero-width policy without a marker removes the tab.

The marker may contain several graphemes, including wide ones, but no control
characters. Every marker grapheme must occupy at least one cell, and the
marker's total display width cannot exceed the replacement width. Empty markers
canonicalize to no marker. These checks happen when the policy is constructed,
so layout never has to repair an invalid value. Removing an explicit policy
restores the default replacement of four spaces.

Replacement happens before intrinsic width, wrapping, clipping, and alignment.
It is a fixed substitution per tab, independent of the current column; Urushi
does not inspect or modify terminal tab stops. Every replacement is ordinary
styled graphemes, so a `ResolvedView` never contains a tab and both renderers
consume the same cell geometry.

Direct text rendering has a different contract. `render_text` serializes the
source spans with explicit `RenderSettings`, without resolving layout, and
therefore writes tabs and newlines as authored. The `print`, `println`, `eprint`,
and `eprintln` helpers choose those settings from the exact standard stream.
Layout-bearing output is named separately as `print_view`, `println_view`,
`eprint_view`, and `eprintln_view`.

## Layout sees one flow

Measurement, explicit newlines, word wrapping, and clipping operate on the
joined source. A style change inside a word neither permits nor forces a wrap.
A style change beside whitespace does not replace the whitespace's ordinary
word-breaking role. Hard wrapping and clipping cut only between whole
graphemes, including CJK and emoji sequences.

Clipping assigns a visible marker the style of the first omitted grapheme. The
marker stands for that omitted suffix and does not introduce a separate style
parameter. Alignment fill remains the enclosing `BlockStyle`'s text style. A
bare mixed-style text leaf has no single source style for invented fill, so any
such fill uses the terminal-default `TextStyle`.

Resolution emits the same per-grapheme `StyledGrapheme` rows as uniformly
styled text. ANSI and TUI adapters consume that one resolved scene and do not
repeat segmentation, width measurement, wrapping, or clipping.

## Rejected designs

- **`View::Text(Vec<TextSpan>)`.** A vector exposes input segmentation as the
  durable view value and cannot express the validated, canonical range
  invariant. `StyledText` gives that invariant a name and prevents invalid
  values from entering layout.
- **A grapheme value inside each `TextSpan`.** Whether a fragment edge is a
  grapheme boundary depends on adjacent fragments. Segmenting fragments first
  produces the wrong unit at exactly the boundary the type is meant to make
  safe.
- **One text view per style.** This turns decoration changes into sibling
  geometry. Each child wraps or clips independently, so changing a color can
  change line breaks and truncation.
- **Public mutable byte ranges.** Byte ranges permit endpoints inside a Unicode
  scalar or grapheme and allow later mutation to invalidate an already checked
  value. Range-oriented editing can be added through checked operations if a
  concrete caller needs it; it is not part of the construction API.
- **Literal tabs in `ResolvedView`.** A terminal tab advances from the current
  cursor position to a terminal-owned stop, so it has no backend-independent
  cell width. Preserving it is instead the direct text renderer's explicit
  non-layout contract.
- **Tab policy on `TextStyle` or `BlockStyle`.** A tab belongs to the source
  text flow. Putting its replacement on a style would make output behavior a
  property of decoration; putting it on a surrounding block would introduce
  inheritance and leave bare text without an owner.
