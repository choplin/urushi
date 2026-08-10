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

Run the end-to-end showcase to see colored boxes, nested styles, CJK-aware
alignment, and horizontal and vertical composition:

```sh
cargo run --example showcase
```

### Theme-aware plain CLI output

Define one light theme and one dark theme, choose `ColorScheme` explicitly,
then resolve a component role through the profile of the writer that will
receive it. The runnable example uses the public path from `ThemeSet` to
`Style::render`:

```sh
cargo run -p urushi --example themed_output
```

`TerminalProfile::detect_for` must be called for the actual writer: use
`stdout` for normal output and `stderr` for diagnostics, rather than carrying a
profile between streams. File and pipe writers are non-TTY, so their resolved
styles have no ANSI escape sequences while retaining borders, padding,
alignment, and visible text. A non-empty `NO_COLOR` similarly removes colors
while retaining modifiers; use `TerminalProfile::new` for a deterministic
application override.

## Roadmap

- [x] `Style` builder: colors, modifiers, padding, margin, border, width, align
- [x] ANSI-aware width measurement and CJK-aware word wrap
- [x] Composition helpers (`join_horizontal`, `join_vertical`)
- [x] Color profile detection and degradation (truecolor → 256 → 16), `NO_COLOR`, non-TTY
- [ ] Adaptive colors (light/dark terminal backgrounds)
- [x] Correct re-styling of content that already contains ANSI sequences (nested styles)
- [x] Theme layer: per-component style sets derived from a small set of semantic tokens
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
