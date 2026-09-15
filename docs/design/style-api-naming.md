# Style API Naming

Urushi gives the ordinary style-construction operation the shortest name while
keeping setting, resetting, and reading unambiguous. This rule applies to style
values and to nearby APIs that expose the same configurable-property shape; it
is not a repository-wide rule that every accessor begins with `get_`.

## Configurable properties

A configurable property is a stored field for which the same public type
exposes a consuming builder that replaces the field and an operation that reads
it. Such a property uses this family when the operations exist:

| Operation | Form | Example |
| --- | --- | --- |
| Set the complete value | `property(value)` | `foreground(color)` |
| Restore its intrinsic default | `reset_property()` | `reset_foreground()` |
| Read the current value | `get_property()` | `get_foreground()` |

The setter owns the bare property name because immutable builder chains are the
ordinary construction path. `get_` makes the less frequent read distinct
without lengthening every write. A reset names one operation consistently even
when the default has different domain meanings: `reset_foreground()` removes a
color instruction, `reset_width()` restores automatic sizing,
`reset_padding()` restores zero padding, and `reset_border_left()` restores an
enabled edge.

`reset_property()` exists only when the containing style or configuration type
implements `Default`. The property's intrinsic default is the value of that
field in `Self::default()`. A reset restores that one field and does not
reconstruct the containing value. For example, `reset_border()` removes the
glyph set but retains the independently configured edge fields.

Set-valued style properties expose changes as `add_*` and `remove_*` rather
than overloading the complete-value setter. `add_modifier(set)` and
`remove_modifier(set)` therefore say whether their argument is unioned or
subtracted. Common fixed values may also have concise conveniences such as
`bold()` and `underlined()`; the adjective leaves `underline(value)` available
for setting the complete underline value.

The same shape applies outside the style structs when one public type exposes
that same consuming-builder and field-read pair. For example,
`RenderSettings::color_level(value)` pairs with `get_color_level()`, and a
presentation's `style(role, value)` pairs with `get_style(role)`. The same rule
gives `StyledText` the family `tab_policy(value)`, `reset_tab_policy()`, and
`get_tab_policy()`.
A role argument may select one member of a logical property without changing
this shape. An associated constructor does not count as a competing setter: it
creates the containing value rather than updating a property on an existing
`self`.

The stored field uses the same property noun as its public method family. A
field may pluralize that noun when it stores a role-indexed collection, or add
a role qualifier such as `item_style`, but it does not retain an abbreviated
or obsolete synonym. For example, `color_level(value)` stores `color_level`,
and `item_style(value)` stores `item_style`.

This document does not name write-only builders, mutable setters, or operations
whose write and read live on different types; those are not this API shape.

## Read-only accessors

A read-only accessor keeps its domain noun. It does not acquire `get_` merely
because it returns a value:

```rust
theme.components().list();
capabilities.color_level();
hyperlink.uri();
block.text_style();
```

These calls have no competing public setter on the same type. Prefixing them
would add noise without resolving ambiguity and would incorrectly make this a
general getter convention.

`BlockStyle::text_style()` is also a structural accessor rather than a leaf
property pair. The block forwards ordinary fill configuration through
`foreground`, `background`, the modifier operations, underline operations, and
`hyperlink`; `BlockStyle::from_text_style` supplies construction from a complete
`TextStyle`. It therefore has no complete-text-style replacement builder whose
name the accessor must yield.

Canonical component and CLI presentations follow this boundary. A
`ComponentTheme` exposes `list()`, `tree()`, and `table()`, but does not accept
a complete Presentation back into itself. Likewise, a `CliTheme` exposes its
canonical Summary and Warning presentations but does not replace them. A caller
that needs another policy clones the canonical presentation or owns a separate
one and calls `compose`; the Theme remains the stable owner of its canonical
shortcut. Theme role styles remain configurable because selecting those shared
styles is part of the Theme's responsibility.

## Rejected alternatives

### Prefix every setter with `with_` or `set_`

This makes the most frequent operation longer in order to reserve the shortest
name for a less frequent read. `set_` also suggests mutation even though these
builders consume and return a value.

### Give paired getters the bare property name

Rust cannot overload `foreground()` and `foreground(color)` by arity. Choosing
the bare name for the getter forces the ordinary setter to take a prefix and
reverses the intended ergonomic priority.

### Prefix every getter with `get_`

This applies a collision solution where no collision exists. Accessors such as
`components().list()` describe navigation through the domain and are clearer
without a mechanical prefix.

### Name defaults `without_*`, `no_*`, or by domain-specific states

Those forms make equivalent operations depend on representation:
`without_foreground`, `auto_width`, and a sentinel such as `Color::None` would
all mean "restore this property to its default" but require different caller
vocabulary. `no_*` is also ambiguous between a query and a builder. A typed
sentinel enlarges the value domain and can represent a non-color as though it
were a color. `reset_*` states the shared operation without those ambiguities.

### Replace a Theme's complete canonical Presentation

Allowing `ComponentTheme` or `CliTheme` to accept an arbitrary Presentation
turns it into a general presentation registry and makes its canonical shortcut
mutable during construction. Applications already retain the more explicit
and composable option: own or clone a Presentation and call `compose` directly.
