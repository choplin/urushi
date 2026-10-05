---
title: Text width reference
description: Complete reference for printable text, graphemes, styled text, spans, and tabs.
---

This page is the task-oriented index to the text API. Use the generated Rust
API reference for every signature, generic bound, trait implementation, and
source link:

- [Complete `urushi` API](https://docs.rs/urushi/latest/urushi/index.html)
- [`PrintableLines`](https://docs.rs/urushi/latest/urushi/struct.PrintableLines.html),
  [`PrintableText`](https://docs.rs/urushi/latest/urushi/struct.PrintableText.html), and
  [`Grapheme`](https://docs.rs/urushi/latest/urushi/struct.Grapheme.html)
- [`TextSpan`](https://docs.rs/urushi/latest/urushi/struct.TextSpan.html),
  [`StyledText`](https://docs.rs/urushi/latest/urushi/struct.StyledText.html), and
  [`StyledTextError`](https://docs.rs/urushi/latest/urushi/struct.StyledTextError.html)
- [`TabPolicy`](https://docs.rs/urushi/latest/urushi/struct.TabPolicy.html) and
  [`InvalidTabMarker`](https://docs.rs/urushi/latest/urushi/enum.InvalidTabMarker.html)

## Printable text

| Type | Public API |
|---|---|
| `PrintableLines` | `new`, `lines` |
| `PrintableText` | `new`, `as_str`, `width`, `truncate`, `graphemes` |
| `Grapheme` | `new`, `as_str`, `width` |

These are borrowed transparent string views. Constructors validate the
plain-text contract and panic when terminal controls violate it. `truncate`
returns a grapheme-aligned prefix fitting the requested cell width.

## Styled text

| Type | Public API |
|---|---|
| `TextSpan` | `new`, `text`, `style` |
| `StyledText` | `new`, `try_from_spans`, `as_str`, `spans`, `tab_policy`, `reset_tab_policy`, `get_tab_policy`, `width` |
| `StyledTextError` | `span` and `byte_offset` identify a grapheme-splitting boundary |

Styled spans are grapheme-aligned. A single grapheme cannot carry conflicting
partial styles.

## Tabs

`TabPolicy::spaces(width)` replaces each tab with that many spaces.
`TabPolicy::with_marker(width, marker)` additionally selects the expansion
marker and may return `InvalidTabMarker`. Accessors are `width()` and
`marker()`.

`InvalidTabMarker` distinguishes a control character, a zero-width grapheme,
and a marker wider than the replacement width.

The default policy uses four-cell stops and blank expansion.

## Layout guarantees

- Widths are terminal cells.
- Grapheme clusters remain atomic.
- CJK and emoji width participates in every View layout.
- Raw terminal controls are not accepted as printable text.
- Canvas never leaves half of a wide grapheme after clipping or overlap.
