# Rendered Output and Display Width

How plain text and already-rendered output are told apart, how each is
measured, and why there is exactly one width measure. [`view-model.md`](../view-model.md)
states the boundary under "Plain text and rendered output" and "Display width".

## The rule

The layout pass never inspects text for escape sequences. Whether a string is
plain text or already-rendered ANSI is carried by the type and measured once,
where it is declared.

The plain side is carried by `PrintableLines` for text that spans rows and
`PrintableText` for one row. Display width belongs to the second of these,
because a width is a property of a row of cells; measuring across a line break
would sum cells that never share one. The rendered side is carried by
`RenderedBlock`, and ANSI-aware measurement is reachable only through
`RenderedBlock::from_ansi`. All three types take the caller's declaration on
trust: the domain is declared, never detected.

Passing escape sequences to a `Text` node is therefore a contract violation,
not a supported call with a degraded result. Debug builds assert at the
boundary where the domain is declared; release builds do not check and instead
measure the escapes as ordinary characters, and wrapping or truncation may
split them. Detection is a development aid, never a runtime behavior the model
promises.

`from_ansi` measures what a terminal would show rather than what the byte
stream contains, because a row of rendered output may be a sequence of
operations rather than a sequence of cells. It is resolved once, at that
boundary:

- `\n` and `\r\n` end a row; the `\r` of a `\r\n` pair is not part of the row
  it ends, a lone `\r` does not end one, a trailing newline leaves one empty
  row, and empty text has no rows at all.
- `\r` returns to column 0, backspace steps back one column and stops there,
  and a tab advances to the next tab stop; text written afterwards lands on top
  of text written earlier, and overwriting either half of a wide character
  erases all of it.
- Each cell keeps the escape scope that was open when it was written, and the
  row re-emits only the transitions between them, so a scope closed before a
  carriage return still covers the cells it wrapped.

What comes out is a rectangle of cells that contains no cursor movement. That
invariant is what lets a block be placed at any column of a join without its
content sliding.

Plain-text width is the sum of the display widths of a line's grapheme
clusters, using the shared `text` implementation. The crate exposes no free
function taking a `&str` and returning a width.

## Why the layout pass never inspects text for escape sequences

Whether a string is plain text or already-rendered ANSI cannot be recovered
from the string, so no engine that accepts one can decide it; any attempt is a
heuristic. The property is therefore carried by the type — `RenderedBlock` —
and measured once, where it is declared.

The same boundary rules out a free function taking a `&str` and returning a
width, because such a function has to guess which of the two it was handed.
The earlier public `visible_width` had exactly that ambiguous domain, and it
had let escape handling leak into the plain path; it was removed, and
`wrap_text` made crate-private, when measurement was split by type.

## Why there is one width measure, and why it is the per-grapheme sum

Width is measured in exactly one way: the sum of the display widths of a
line's grapheme clusters. Not `unicode-width`'s measure of the whole string,
and not a measure that skips escape sequences — that third one is what
`RenderedBlock::from_ansi` does, on the other side of the domain boundary.

The two plain-text measures are not interchangeable. `unicode-width` applied to
a whole string honors ligatures that span a cluster boundary: LAM followed by
ALEF renders as one glyph in one cell, and the whole-string measure says one,
while the sum says two. Over every RTL block, those are the only disagreements
— 80 pairs, all LAM-class plus ALEF-class; outside RTL the two never differ.

The sum is the one the model must use, for an ordering reason rather than a
tidiness one. A box fixes its width before its content is wrapped into it,
because wrapping needs a width to wrap to. That only works if "text wrapped to
`w` occupies at most `w`" holds, which in turn requires the wrap decision and
the width total to be the same measure. And the total has to be the sum,
because a row of the resolved view (a `ResolvedView`) *is* a sequence of
per-grapheme tokens whose widths add to the rectangle's width — any other
measure produces a number no row can satisfy.

The cost is that a LAM+ALEF pair is measured one cell wider than a terminal
draws it. Merging the pair into a single token is representable — a token's
symbol is a `String` — but matching the terminal exactly makes the measure
terminal-dependent, which is the same move as ambiguous East Asian width and
belongs with it: an input to `measure` and `resolve`, not a constant. Until
that exists, this is a defined deviation rather than an unknown one.
