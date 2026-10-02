---
title: Configure text appearance
description: Set colors, attributes, underlines, and OSC 8 hyperlinks.
---

`TextStyle` describes the appearance of one text run. It has no padding,
border, or dimensions; use [Layout](/docs/core/layout/) when the
content needs a rectangle.

| Capability | Use it for | Main API | Behavior |
|---|---|---|---|
| Foreground and background | Color text or the cells behind it | `foreground`, `background` | Accepts ANSI, ANSI 256, or RGB `Color` values |
| Attributes | Emphasis and terminal text effects | `bold`, `dim`, `italic`, `underlined`, `blink`, `reverse`, `hide`, `strikethrough`; `add_attribute(s)` | Adds the selected attributes to the style |
| Underline style and color | Single, double, curly, dotted, or dashed emphasis | `underline_style`, `underline_color`, `underline` | Shape and color are one underline value; unsupported forms may be narrowed by the renderer |
| OSC 8 hyperlink | Give visible text a terminal link target | `hyperlink`; `Hyperlink::new` | Supporting terminals make the text clickable; other renderers keep the visible label |
| Hyperlink parameters | Give an OSC 8 link an identifier or future terminal-defined metadata | `Hyperlink::parameter` | Parameters are emitted in insertion order and safely encoded |
| Resets | Remove selected appearance from a reusable value | `reset_foreground`, `reset_background`, `reset_attributes`, `reset_underline`, `reset_hyperlink` | Returns that property to its unset/default state |

`TextStyle::new()` starts with every field unset. Builders consume and return
the value, so clone a reusable base before deriving variants.

## Set colors and attributes

This complete example prints three independently styled lines. It includes a
background color so foreground and cell fill can be compared directly:

```rust
use std::io;

use urushi::{Color, TextStyle, View};

fn main() -> io::Result<()> {
    urushi::println_view(&View::text(
        "ready",
        TextStyle::new().foreground(Color::GREEN).bold(),
    ))?;
    urushi::println_view(&View::text(
        "secondary",
        TextStyle::new().dim().italic(),
    ))?;
    urushi::println_view(&View::text(
        "attention",
        TextStyle::new()
            .foreground(Color::BLACK)
            .background(Color::YELLOW),
    ))
}
```

<pre class="terminal-preview" aria-label="Rendered terminal output showing green bold, dim italic, and yellow background text"><code><span class="ansi-green ansi-bold">ready</span>
<span class="ansi-dim ansi-italic">secondary</span>
<span class="ansi-bg-yellow">attention</span></code></pre>

On a capable terminal, `ready` is green and bold; `secondary` is dim and
italic; and `attention` uses contrasting foreground and background cells. The
visible text remains the same when the destination supports fewer features.
Use `add_attribute(s)` and `remove_attribute(s)` when working with explicit
`TextAttribute` values instead of the convenience builders.

The remaining attribute builders affect a run as follows:

<pre class="terminal-preview" aria-label="Text attribute preview"><code><span class="ansi-underline">underlined</span>  <span class="ansi-strikethrough">strikethrough</span>  <span class="ansi-reverse">reverse</span>
<span class="ansi-bold">bold</span>        <span class="ansi-dim">dim</span>            <span class="ansi-italic">italic</span>
<span class="ansi-blink">blink</span>       hidden: <span class="ansi-hidden-cell"><span class="ansi-hidden">hidden text</span></span></code></pre>

`blink` requests terminal-controlled blinking, which this static preview does
not animate. `hide` makes the graphemes invisible while they continue to occupy
cells; the blank area after `hidden:` is therefore intentional.

## Configure an underline

`underlined()` selects a single underline in the foreground color. Set its
shape or color independently when the terminal presentation needs more detail:

```rust
use urushi::{Align, Color, TextStyle, UnderlineStyle, View};

let view = View::column(Align::Left, [
    ("single", UnderlineStyle::Single),
    ("double", UnderlineStyle::Double),
    ("curly", UnderlineStyle::Curly),
    ("dotted", UnderlineStyle::Dotted),
    ("dashed", UnderlineStyle::Dashed),
].map(|(label, shape)| {
    View::text(
        label,
        TextStyle::new()
            .underline_style(shape)
            .underline_color(Color::YELLOW),
    )
}));
urushi::println_view(&view)?;
# Ok::<(), std::io::Error>(())
```

<pre class="terminal-preview" aria-label="Single, double, curly, dotted, and dashed yellow underlines"><code><span class="ansi-underline" style="text-decoration-color:#ffd75f">single</span>
<span class="ansi-underline-double">double</span>
<span class="ansi-underline-curly">curly</span>
<span class="ansi-underline-dotted">dotted</span>
<span class="ansi-underline-dashed">dashed</span></code></pre>

The logical style retains the requested shape and color. The output renderer
uses the underline features selected by its `RenderSettings`; terminal output
derives those settings from detected capabilities.

## Add an OSC 8 hyperlink

Attach a URI directly for the ordinary case. Construct `Hyperlink` when the
OSC 8 link also needs parameters such as `id`:

```rust
use std::io;

use urushi::{Color, Hyperlink, TextStyle, View};

fn main() -> io::Result<()> {
    let target = Hyperlink::new("https://example.com/docs")
        .parameter("id", "guide");
    let view = View::text(
        "Open documentation",
        TextStyle::new()
            .foreground(Color::CYAN)
            .underlined()
            .hyperlink(target),
    );

    urushi::println_view(&view)
}
```

<pre class="terminal-preview" aria-label="Rendered terminal hyperlink in cyan with an underline"><code><a href="https://example.com/docs">Open documentation</a></code></pre>

When hyperlinks are supported, the renderer surrounds the visible label with
an OSC 8 open and close sequence, so the terminal can make it clickable. When
they are unsupported or disabled, the same label is rendered without those
sequences. Color and underline capability selection is independent from
hyperlink selection.

`Hyperlink::new` accepts any UTF-8 string; it does not parse or validate URL
syntax. It percent-encodes control characters so the URI cannot terminate the
OSC 8 sequence. `parameter` also percent-encodes the OSC 8 delimiters `:`, `=`,
and `;` in parameter names and values. Parameters remain open-ended because
terminals currently define `id` and may define more keys later.

Use `RenderSettings::hyperlinks(true)` only when a serializer has already
chosen hyperlink support. Ordinary terminal output should keep using
`println_view`, which narrows the style to detected capabilities.

## Reset part of a style

Reset builders remove only the named property. Other properties remain:

```rust
use urushi::{Color, TextStyle};

let linked = TextStyle::new()
    .foreground(Color::CYAN)
    .bold()
    .underlined()
    .hyperlink("https://example.com");

let plain_target = linked
    .reset_foreground()
    .reset_attributes()
    .reset_underline()
    .reset_hyperlink();

assert_eq!(plain_target, TextStyle::new());
```

The same reset is visible when a shared base style is used for two runs:

<pre class="terminal-preview" aria-label="Before and after resetting a text style"><code><span class="ansi-cyan ansi-bold ansi-underline">before reset</span>
after reset</code></pre>

The corresponding `reset_background` builder removes a background color.
These methods return the property to the same unset/default state used by
`TextStyle::new()`; they do not retain a separate reset instruction.
