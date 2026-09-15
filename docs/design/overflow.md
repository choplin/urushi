# Overflow

What happens to content that does not fit the box it resolved to: the policy
vocabulary, what a marker costs, which axis and which text a policy governs,
and why the choice belongs to the application. [`view-model.md`](../view-model.md)
summarizes this under "Sizing at a glance"; the size a box resolves to before
overflow applies is [`box-sizing.md`](box-sizing.md).

## The rule

The frame always closes at the used size. Overflow is absorbed by the
content, under a policy the application chooses per block:

```rust
pub enum Overflow {
    Wrap,                     // the default: reflow to the content width
    Clip(Cow<'static, str>),  // cut inside the frame, ending the line with a marker
}

Overflow::clip()             // cut silently
Overflow::ellipsis()         // Clip("…")
Overflow::clip_with("...")   // where an ASCII border would be chosen too
```

There are three constructors: `Overflow::clip()` cuts silently,
`Overflow::ellipsis()` is `Clip("…")`, and `Overflow::clip_with("...")` states
the marker an ASCII-only terminal can show — the same choice `Border::ASCII`
answers for box glyphs. Which glyphs a terminal can render is the
application's knowledge, so the library fixes no marker.

The marker occupies cells of its own, so the content keeps the content width
less the marker's display width — a three-cell `...` costs three. A marker the
box cannot hold beside any content is dropped, leaving a silent cut rather than
a box filled with the marker. In styled text, the marker represents the omitted
suffix and takes the complete style of its first omitted grapheme.

`overflow` governs the width axis. Height always clips inside the frame;
clipping inside a closed frame is a viewport's behavior, so scrolling composes
on top of this rule.

The policy fits the text a block directly contains. A child that is itself a
view absorbs its own overflow when it resolves under the area the containing
block leaves it, so the policy does not reach past one node. A bare `Text`
resolved in a narrow area wraps, the same default a block's content gets.
Choosing another policy requires a block, because the policy is a box property.

Cutting an already-rendered string at a column is a text-layer utility, not a
style property and not part of the box model.

## Why overflow is the application's choice

Whether excess content wraps, clips, or ends in an ellipsis is presentation
policy, and fixing any one of them in the library would be wrong for two of
the three real cases — prose wraps, a viewport clips, a status-bar path
ellipsizes. What the library fixes is the invariant underneath the choice:
the frame closes at the used size, and no overflow policy can cut it.

The height axis takes less of that choice: height has no wrap analogue, and
clipping inside a closed frame is a viewport's behavior, so height clips, and
scrolling composes on top; a vertical marker is a possible extension.

The marker a cut ends with is a parameter of clipping, not a third policy.
Marking a cut does not absorb overflow differently — it is the same cut, made
visible — and fixing the glyph would fix `…` for terminals that cannot show
it, which is the problem `Border::ASCII` already exists to solve on the same
axis. Lip Gloss draws the same line: `ansi.Truncate(s, width, tail)` takes the
tail as a string and passes `""` for a silent cut. Because the marker is
measured in cells like any other content, parameterizing it also removes a
hidden assumption from the budget — a three-cell `...` costs three, where a
fixed `…` had let the implementation subtract one.

Cutting an already-*rendered* string at a column, by contrast, stays a
text-layer utility, so that "frames close" remains an invariant of the box model
rather than a default.

## Rejected designs

- **A `truncate` property alongside a layout-participating `max_width`.** It
  would have preserved the open-frame artifact — a bound cutting the frame —
  as an opt-in.
- **A fixed ellipsis glyph, with `Clip` and `Ellipsis` as sibling policies.**
  Argued above. The cost of carrying the marker is that `Overflow` is not
  `Copy`.
