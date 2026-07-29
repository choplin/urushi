# urushi

Box-model styling for terminal output in Rust — inspired by Go's
[lipgloss](https://github.com/charmbracelet/lipgloss).

**Status: early development.** APIs will change without notice.

漆 (*urushi*) is Japanese lacquer: layers of coating that give a surface its
gloss.

## Concept

Rust has excellent TUI foundations ([ratatui](https://ratatui.rs)) and several
prompt libraries, but no shared styling substrate that works across plain CLI
output, interactive prompts, and full TUIs. `urushi` aims to fill that gap:

- **Standalone first.** A `Style` renders to a plain ANSI `String`, so it
  works with `println!` — no terminal setup, raw mode, or event loop.
- **Ride the ratatui ecosystem via an adapter.** A planned `ratatui` cargo
  feature will let the same styles be used as ratatui widgets, mapping the
  fg/bg/modifier subset onto `ratatui::style::Style` and carrying the box
  model in the widget implementation.
- **CJK correctness as a first-class goal.** Width measurement, wrapping,
  borders, and alignment are East Asian width aware.

## Crates

| Crate | Description | Status |
|---|---|---|
| [`urushi`](urushi/) | Style definitions: colors, modifiers, padding, margin, borders, alignment, wrapping | Core rendering works |
| [`urushi-prompt`](urushi-prompt/) | Interactive prompts modeled after [huh](https://github.com/charmbracelet/huh) | Placeholder |

## Example

```rust
use urushi::{Align, Border, Color, Style};

let style = Style::new()
    .foreground(Color::Ansi256(212))
    .border(Border::ROUNDED)
    .border_foreground(Color::MAGENTA)
    .padding((0, 1))
    .align(Align::Center)
    .width(24);

println!("{}", style.render("こんにちは, urushi!"));
```

## Roadmap

- [x] `Style` builder: colors, modifiers, padding, margin, border, width, align
- [x] ANSI-aware width measurement and CJK-aware word wrap
- [ ] Composition helpers (`join_horizontal`, `join_vertical`, `place`)
- [ ] Color profile detection and degradation (truecolor → 256 → 16), `NO_COLOR`, non-TTY
- [ ] Adaptive colors (light/dark terminal backgrounds)
- [ ] Correct re-styling of content that already contains ANSI sequences (nested styles)
- [ ] Theme layer: per-component style sets derived from a small set of semantic tokens
- [ ] `ratatui` feature: `impl Widget`, `From<Style>` for the stylable subset
- [ ] `urushi-prompt`: huh-style `Form` / `Group` / fields with validation and theming

## Acknowledgments

The API design follows [lipgloss](https://github.com/charmbracelet/lipgloss)
closely; `urushi-prompt` will do the same for
[huh](https://github.com/charmbracelet/huh). Thanks to the
[Charm](https://charm.sh) team for showing what a coherent terminal UI stack
looks like.

## License

MIT or Apache-2.0, at your option.
