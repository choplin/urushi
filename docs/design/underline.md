# The Underline Value

An underline carries a shape and a color, and both live in a single optional
property. [`style-model.md`](../style-model.md) states the rule; this file
holds the value, its builders, how each backend spells and degrades it, and
why it is one value rather than a flag and a color.

## The rule

```rust
enum UnderlineStyle { Single, Double, Curly, Dotted, Dashed }

struct Underline {
    style: UnderlineStyle,
    color: Option<Color>,   // None: drawn in the foreground color
}
```

`TextStyle::underline` is `Option<Underline>`; `None` is no underline. There is
no `Modifier::UNDERLINED` flag and no free-standing underline color property.

```rust
TextStyle::new().underline();                              // single, foreground color
TextStyle::new().underline_style(UnderlineStyle::Curly);   // keeps any color already set
TextStyle::new().underline_color(Color::RED);              // adds a single underline if absent
```

No builder produces a color nothing draws. `underline_value` returns the whole
`Option<Underline>`, and `remove(TextStylePropertyKey::Underline)` removes the
underline and its color together. `BlockStyle` mirrors all of these for its fill
text.

At the output boundary:

- ANSI rendering spells a single underline `4` rather than the equivalent
  `4:1`, which a terminal that does not parse subparameters still understands;
  the other shapes are spelled `4:2` to `4:5`, and a terminal that does not
  parse subparameters draws them as a single underline or not at all. No
  profile degrades a shape: which shapes a terminal renders is the
  application's knowledge, like which border glyphs and clip markers it
  renders. An absent underline color emits nothing rather than SGR 59: `Color`
  has no reset spelling, and the reset closing every painted scope already
  restores the default.
- Ratatui has no underline shape, so every underline degrades to Ratatui's
  `Modifier::UNDERLINED` in the `urushi-tui` adapter, and the underline color
  is dropped — reaching it would require the `underline-color` feature, which
  pulls in a backend the adapter does not depend on.
- An underline survives `RenderSettings` with `ColorLevel::None` when its shape
  remains selected, while its color degrades with foreground and background.

## Why an underline is a value rather than a flag and a color

The property vocabulary decides whether two styles with the same appearance are
the same value, and a run's style is the unit a redraw compares. An underline
is where the naive vocabulary fails twice. An `UNDERLINED` modifier flag spells
Select Graphic Rendition (SGR) `4`, which is SGR `4:1`, so a separate shape
property would give a single underline two spellings; and an underline color
paints nothing on a run with no underline, so a free-standing color property
would be invisible in the output while still making two values unequal. Both
are closed by making the underline one optional value that owns its shape and
its color — which is why `Modifier` lost its underline flag rather than gaining
a sibling shape property.

What a single field cannot decide — an underline color equal to the foreground —
is left to the canonical fold of
[`style-canonical-form.md`](style-canonical-form.md). The division between the
two — what the type forbids and what the fold normalizes away — is the general
rule: unrepresentability wherever one field decides, normalization only where a
comparison between fields is required.
