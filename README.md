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

- **Standalone first.** A `BlockStyle` renders to a block that displays as a
  plain ANSI string, so it works with `println!` — no terminal setup, raw mode,
  or event loop.
- **Ride the ratatui ecosystem via an adapter.** The `urushi-tui` crate lets
  the same styles be used as ratatui widgets, mapping the
  fg/bg/modifier subset onto `ratatui::style::Style` and carrying the box
  model in the widget implementation.
- **CJK correctness as a first-class goal.** Width measurement, wrapping,
  borders, and alignment are East Asian width aware.

See [`docs/architecture.md`](docs/architecture.md) for how the shared
foundation is divided into plain CLI, interactive prompt, and Ratatui-facing
surface layers, including which parts are implemented today.

## Crates

| Crate | Description | Status |
|---|---|---|
| [`urushi`](urushi/) | Style definitions: colors, modifiers, padding, margin, borders, alignment, wrapping | Core rendering works |
| [`urushi-prompt`](urushi-prompt/) | Theme-aware `Input`, `Select`, and `Confirm` fields with synchronous validation | Core prompt flow works |
| [`urushi-tui`](urushi-tui/) | Ratatui style conversion and box-model widgets; provisional home for the future full-screen runtime | Adapter works; runtime is not implemented |

## Example

Run the English showcase to see one labeled example per styling feature. Each
entry changes one subject at a time, including colors, modifiers, spacing,
alignment, joins, and individual border sides:

```sh
cargo run --example showcase
```

Run the Japanese variant for the same one-feature-at-a-time catalog with
CJK-aware width and alignment:

```sh
cargo run --example cjk_showcase
```

### Theme-aware plain CLI output

Define one light theme and one dark theme, choose `ColorScheme` explicitly,
then resolve a component role through the profile of the writer that will
receive it. The runnable example uses the public path from `ThemeSet` to
`BlockStyle::render`:

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

### The same Theme in ratatui

Depend on the TUI adapter when the application also uses ratatui:

```toml
[dependencies]
urushi = "0.1.0"
urushi-tui = "0.1.0"
```

Resolve a component from the same `Theme` used by plain output, apply the
terminal's `TerminalProfile`, then pass its widget adapter to a ratatui frame.
Colors and modifiers stay in the Theme; the TUI layer does not define a second
palette.

```rust
use urushi_tui::ratatui::RatatuiStyleExt as _;

let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));
frame.render_widget(panel.widget("保存しました"), frame.area());
```

Apply `TerminalProfile` before either rendering adapter. This keeps truecolor,
256-color, 16-color, monochrome, and disabled output consistent between plain
ANSI strings and ratatui. Detect the profile for the writer owned by your
terminal setup, or construct an explicit profile when the application already
knows the backend capability.

`urushi_tui::ratatui::RatatuiStyleExt::widget` carries margin, border, padding, dimensions, and alignment
into the ratatui `Buffer`: the target `Rect` is the area the box resolves under, so the frame
closes inside it and wide graphemes are never split. It is stateless and
does not initialize or restore the terminal. `RatatuiStyle::from(&style)` converts a
`TextStyle` when only foreground, background, and text modifiers are needed;
that conversion carries no geometry, because a `TextStyle` has none.

Run the complete Theme → plain CLI / ratatui example with:

```sh
cargo run -p urushi-tui --example themed_ratatui
```

### The same Theme in interactive prompts

Run the English prompt wizard from the repository root:

```sh
nix develop --command cargo run -p urushi-prompt --example wizard
```

Then run the CJK variant to exercise East Asian text input, width handling,
and localized field help:

```sh
nix develop --command cargo run -p urushi-prompt --example cjk_wizard
```

The example passes one `Theme` and the terminal's `TerminalProfile` to a form
containing `Input`, `Select`, and `Confirm`. The prompt resolves semantic roles
from that Theme; it does not define a separate palette.

Use this sequence for a terminal smoke test:

1. Press Enter with the name empty. The form shows a validation error and
   remains on the input.
2. Enter a CJK name such as `花子`, then press Enter. Use the arrow keys to
   change the language.
3. Press Enter to reach confirmation, then press Shift-Tab. The form returns
   to the language field without losing the selection. Press Enter again.
4. Press `y` or `n` to choose and submit directly, or use Left/Right and Enter.
   Tab does not submit the final field.
   A submitted form prints the selected name and language below the prompt.
5. Run the example again and press Escape or Ctrl-C. The form cancels, removes
   its inline prompt region, and restores raw mode and cursor visibility.

Resize the terminal while editing the CJK name to check narrow layouts. The
cursor remains inside the prompt viewport, and redraws do not clear text to
the left of the prompt's starting position.

## Roadmap

- [x] `TextStyle` / `BlockStyle` builders: colors, modifiers, padding, margin, border, width, align
- [x] ANSI-aware width measurement and CJK-aware word wrap
- [x] Composition helpers (`join_horizontal`, `join_vertical`)
- [x] Color profile detection and degradation (truecolor → 256 → 16), `NO_COLOR`, non-TTY
- [ ] Adaptive colors (light/dark terminal backgrounds)
- [x] Nested styles as a view tree (`View::text` / `block` / `row` / `column`) resolved in one layout pass, rather than re-styling already-rendered text
- [x] Theme layer: per-component style sets derived from a small set of semantic tokens
- [x] `urushi-tui`: box-model Widget and loss-aware Ratatui style conversion
- [x] `urushi-prompt`: themed `Form` / `Group` with `Input`, `Select`, `Confirm`, and synchronous validation

## Acknowledgments

The API design takes [lipgloss](https://github.com/charmbracelet/lipgloss) as
its primary reference; `urushi-prompt` does the same with
[huh](https://github.com/charmbracelet/huh). Thanks to the
[Charm](https://charm.sh) team for showing what a coherent terminal UI stack
looks like.

## License

MIT or Apache-2.0, at your option.
