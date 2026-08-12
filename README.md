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
- **Ride the ratatui ecosystem via an adapter.** An optional `ratatui` cargo
  feature lets the same styles be used as ratatui widgets, mapping the
  fg/bg/modifier subset onto `ratatui::style::Style` and carrying the box
  model in the widget implementation.
- **CJK correctness as a first-class goal.** Width measurement, wrapping,
  borders, and alignment are East Asian width aware.

## Crates

| Crate | Description | Status |
|---|---|---|
| [`urushi`](urushi/) | Style definitions: colors, modifiers, padding, margin, borders, alignment, wrapping | Core rendering works |
| [`urushi-prompt`](urushi-prompt/) | Theme-aware `Input`, `Select`, and `Confirm` fields with synchronous validation | Core prompt flow works |

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

### The same Theme in ratatui

Enable the optional adapter when the application also uses ratatui:

```toml
[dependencies]
urushi = { version = "0.1.0", features = ["ratatui"] }
```

Resolve a component from the same `Theme` used by plain output, apply the
terminal's `TerminalProfile`, then pass its widget adapter to a ratatui frame.
Colors and modifiers stay in the Theme; the TUI layer does not define a second
palette.

```rust
let panel = profile.resolve_style(theme.style(ComponentRole::PanelFocused));
frame.render_widget(panel.widget("保存しました"), frame.area());
```

Apply `TerminalProfile` before either rendering adapter. This keeps truecolor,
256-color, 16-color, monochrome, and disabled output consistent between plain
ANSI strings and ratatui. Detect the profile for the writer owned by your
terminal setup, or construct an explicit profile when the application already
knows the backend capability.

`Style::widget` carries margin, border, padding, fixed width, and alignment
into the ratatui `Buffer`, including CJK-aware clipping. It is stateless and
does not initialize or restore the terminal. `RatatuiStyle::from(&style)` is
available when only foreground, background, and text modifiers are needed;
that conversion deliberately omits the box model and border colors.

Run the complete Theme → plain CLI / ratatui example with:

```sh
cargo run -p urushi --example themed_ratatui --features ratatui
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

- [x] `Style` builder: colors, modifiers, padding, margin, border, width, align
- [x] ANSI-aware width measurement and CJK-aware word wrap
- [x] Composition helpers (`join_horizontal`, `join_vertical`)
- [x] Color profile detection and degradation (truecolor → 256 → 16), `NO_COLOR`, non-TTY
- [ ] Adaptive colors (light/dark terminal backgrounds)
- [x] Correct re-styling of content that already contains ANSI sequences (nested styles)
- [x] Theme layer: per-component style sets derived from a small set of semantic tokens
- [x] `ratatui` feature: box-model Widget and loss-aware stylable-subset conversion
- [x] `urushi-prompt`: themed `Form` / `Group` with `Input`, `Select`, `Confirm`, and synchronous validation

## Acknowledgments

The API design follows [lipgloss](https://github.com/charmbracelet/lipgloss)
closely; `urushi-prompt` will do the same for
[huh](https://github.com/charmbracelet/huh). Thanks to the
[Charm](https://charm.sh) team for showing what a coherent terminal UI stack
looks like.

## License

MIT or Apache-2.0, at your option.
