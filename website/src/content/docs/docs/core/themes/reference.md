---
title: Themes reference
description: Complete reference for Theme, tokens, roles, presets, modes, and component themes.
---

This page is the task-oriented index to the theme API. Use the generated Rust
API reference for every signature, generic bound, trait implementation, and
source link:

- [Complete `urushi` API](/api/urushi/index.html)
- [`Theme`](/api/urushi/struct.Theme.html),
  [`SemanticTokens`](/api/urushi/struct.SemanticTokens.html), and
  [`ComponentTheme`](/api/urushi/struct.ComponentTheme.html)
- [`ThemePreset`](/api/urushi/struct.ThemePreset.html),
  [`ThemeMode`](/api/urushi/enum.ThemeMode.html), and
  [`ComponentRole`](/api/urushi/enum.ComponentRole.html)

## Theme

| API | Meaning |
|---|---|
| `Theme::from_tokens(tokens)` | Derive canonical component presentations |
| `Theme::new(tokens, components)` | Supply tokens and a complete ComponentTheme |
| `tokens()` | Borrow semantic tokens |
| `components()` | Borrow component styles and presentations |
| `list`, `table`, `tree`, `scrollbar` | Compose canonical component Views |
| `text_style(role)` | Resolve any `TextThemeRole` |
| `block_style(role)` | Resolve any `BlockThemeRole` |

`text_style` and `block_style` return owned styles because application roles
may derive them. In a draw loop, resolve a stable role once into application
state and borrow or clone that cached value; do not repeatedly derive the same
role for every cell. Built-in component styles are also available by reference
through `components()`.

## SemanticTokens

Public fields are `text`, `text_muted`, `background`, `surface`, `accent`,
`accent_text`, `success`, `warning`, `error`, and `border`.

## Built-in roles

- `ComponentRole`: body, muted, accent, success, error, and the question,
  answer, placeholder, cursor, option, selected option, button, focused button,
  help, and error roles used by prompts.
- `PanelRole`: panel and focused panel.
- `ListRole`: item and enumerator.
- `TreeRole`: root, item, and connector.
- `TableRole`: header and cell.
- `ScrollbarRole`: thumb, track, begin, and end.

`TextThemeRole` and `BlockThemeRole` are public extension traits for
application-defined role enums.

## ComponentTheme

`ComponentTheme::from_tokens` derives the canonical component layer.
`get_text_style`, `get_panel`, `get_panel_focused`, and the component-specific
style getters expose role styles. `list`, `table`, `tree`, and `scrollbar`
expose canonical presentations. Builder methods replace shared text roles,
panel styles, and component-specific list, tree, table, or scrollbar roles.

## Presets

`ThemePreset::all()` returns the complete built-in catalog in name order.
`get(exact_name)` performs exact lookup. Accessors are `name` and `scheme`;
`theme()` builds a Theme. Presets implement `Display` and `Debug`.

## Light and dark selection

`ColorScheme` is `Light` or `Dark`. `ThemeSet::new(light, dark)` stores both;
`light`, `dark`, and `select` return references.

`ThemeMode` has explicit `Light` and `Dark` variants plus
`Auto { fallback: ColorScheme }`. `resolve(optional_background)` returns the
selected scheme without performing terminal I/O.
