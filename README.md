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

- **Standalone first.** A `View` resolves independently from output and renders
  to a `String`; `urushi::print_view` and `urushi::println_view` handle ordinary terminal
  output without raw mode or an event loop.
- **Ride the ratatui ecosystem via an adapter.** The `urushi-tui` crate draws
  urushi styles and view trees into a ratatui `Buffer`, mapping the
  fg/bg/modifier subset onto `ratatui::style::Style` and keeping the box model
  in urushi's layout pass. A ratatui application can use urushi as its UI
  library without giving up its own loop.
- **CJK correctness as a first-class goal.** Width measurement, wrapping,
  borders, and alignment are East Asian width aware.

See [`docs/architecture.md`](docs/architecture.md) for how the shared
foundation is divided into plain CLI, interactive prompt, and Ratatui-facing
surface layers, including which parts are implemented today.

## Crates

| Crate | Description | Status |
|---|---|---|
| [`urushi`](urushi/) | Style definitions: colors, modifiers, padding, margin, borders, alignment, wrapping | Core rendering works |
| [`urushi-cli`](urushi-cli/) | Opinionated `Summary` and `Warning` presentation for human-facing, non-interactive CLI output | Core presentations work |
| [`urushi-terminal`](urushi-terminal/) | Workspace-independent terminal contracts, size, and output-capability inspection | `Frame` / `Terminal` contracts and detection work |
| [`urushi-prompt`](urushi-prompt/) | Theme-aware `Input`, `Select`, and `Confirm` fields with synchronous validation | Core prompt flow works |
| [`urushi-tui`](urushi-tui/) | The `ratatui` adapter — style conversion, box-model widgets, and the cell-writing path they share with a renderer — and the home of the full-screen runtime | Adapter works; runtime is not implemented |

## Two ways to build a full-screen TUI

A full-screen application needs someone to own the terminal: entering and
restoring it, reading events, and deciding when a frame is drawn. urushi
supports two answers, and they differ only in who that owner is.

### The application owns the loop, urushi is its UI library

Available today. The application keeps its ratatui `Terminal`, its event loop,
and its own state, and reaches for urushi where it would otherwise hand-build
styling and layout:

| The application owns | urushi provides |
|---|---|
| Terminal entry and restoration, raw mode, the event loop, when a frame is drawn | Logical `Theme` and renderer-neutral `View` values |
| Model state, focus, scroll offset, key handling | The box model — margin, border, padding, dimensions, alignment — resolved in one layout pass |
| Which `Rect` each part of the screen gets | The `View` tree and reusable components that return one |

Draw a whole view tree with `ViewWidget`, or a single themed block with the
`RatatuiStyleExt::widget` shorthand. Both are ordinary stateless ratatui
widgets that write only to the buffer they are handed:

```rust
use urushi_tui::ratatui::{RatatuiStyleExt as _, ViewWidget};

terminal.draw(|frame| {
    frame.render_widget(ViewWidget::new(&view), frame.area());
    frame.render_widget(panel.widget("保存しました"), status_area);
})?;
```

These widgets merge their styles with earlier writes to the same ratatui cells
by default. Select `CellWriteMode::Replace` when the resolved view should reset
and fully own only the cells it draws.

The target `Rect` is the area the view resolves under, so the box fits inside
it rather than overflowing it, and wide graphemes are never split. An
application that needs the resolution itself — to measure it, or to draw it
more than once — resolves under `available` and writes the result with
`draw_resolved`, which is the same cell-writing path the widgets take:

```rust
use urushi::resolve;
use urushi_tui::ratatui::{available, draw_resolved};

let resolved = resolve(&view, available(area))?;
draw_resolved(&resolved, area, frame.buffer_mut());
```

Two things this approach does not carry yet: a resolved view reports no cursor
position, so an application placing a caret computes it itself, and text inside
a ratatui `Buffer` is plain — ANSI escape sequences are not interpreted there.

### urushi owns the loop, the application describes state

In development. [`docs/tui-architecture.md`](docs/tui-architecture.md) defines a
TEA-style runtime in `urushi-tui`: the application supplies a model, an update
function, and a view, while the runtime owns event delivery, effect execution,
frame scheduling, and terminal lifecycle. It builds on the same view tree and
the same cell-writing path as the adapter above, so a view written for one is a
view for the other.

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
compose a `View`, then pass it to `urushi::println_view`. The convenience function
inspects stdout, resolves with the terminal width, selects supported rendering
features, and writes the result:

```sh
cargo run -p urushi --example themed_output
```

`print_view` and `println_view` target stdout; `eprint_view` and `eprintln_view`
target stderr. The corresponding names without `_view` write `StyledText`
without resolving layout.
Redirected output is an unbounded plain dump. A non-empty `NO_COLOR` removes
colors while retaining other supported features. Code writing an arbitrary
`std::io::Write` target uses `urushi_terminal::detect`, `resolve`, and `render`
directly and can select different `RenderSettings` from the detected maximum.

For opinionated command summaries and warnings, derive the separate CLI theme
from the same core theme:

```rust
use urushi_cli::{CliTheme, Summary, Warning};

let cli = CliTheme::from_theme(&theme);
let result = cli.summary(&Summary::new("Done").field("Output", "report.json"));
let caution = cli.warning(&Warning::new("Overwrite", "The old file will be replaced"));
```

`urushi-cli` owns this visual language. It does not define logging levels or
delivery, live progress, prompts, or a full-screen runtime.

### The same Theme in ratatui

Depend on the adapter when the application also uses ratatui:

```toml
[dependencies]
urushi = "0.1.0"
urushi-tui = "0.1.0"
```

Resolve a component from the same `Theme` used by plain output, then pass its
widget adapter to a ratatui frame. Colors and modifiers stay logical in the
Theme; Ratatui performs its own backend conversion.

```rust
use urushi_tui::ratatui::RatatuiStyleExt as _;

let panel = theme.block_style(PanelRole::PanelFocused);
frame.render_widget(panel.widget("保存しました"), frame.area());
```

`RatatuiStyle::from(&style)` converts a `TextStyle` when only foreground,
background, and text modifiers are needed; that conversion carries no geometry,
because a `TextStyle` has none.

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

The example passes one `Theme` to a form containing `Input`, `Select`, and
`Confirm`. The prompt inspects stderr and resolves semantic roles from that
Theme; it does not define a separate palette. A form starts on a new
line and uses the remaining terminal width by default. To place it after text
on the current line and cap its width, run:

```sh
nix develop --command cargo run -p urushi-prompt --example current_position
```

That example combines `PromptStart::CurrentPosition` with
`FormBuilder::width`; the prefix to the left of the prompt remains untouched.

Inline prompts cannot locate their old rows after the primary buffer reflows
them on resize. The default therefore restores the terminal session and returns
`RunError::Resized` without guessing which rows to erase. A caller that accepts
losing every other visible cell may opt into a destructive primary-buffer
redraw:

```rust
use urushi_prompt::{Form, InlineResizePolicy};

let builder = Form::builder()
    .inline_resize_policy(InlineResizePolicy::ClearViewportAndRedraw);
// Add groups to `builder`, then build the form.
```

This policy clears only the visible viewport, not scrollback, and never enters
the alternate screen.

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

Resize the terminal while editing to observe the default `RunError::Resized`
path. Configure `ClearViewportAndRedraw` to smoke-test continued editing at the
new size; expect all other visible primary-buffer content to be erased.

## Roadmap

- [x] `TextStyle` / `BlockStyle` builders: colors, modifiers, padding, margin, border, width, align
- [x] ANSI-aware width measurement and CJK-aware word wrap
- [x] View composition before layout and rendering
- [x] Workspace-independent terminal contracts, target-specific detection, and feature-granular rendering settings
- [ ] Adaptive colors (light/dark terminal backgrounds)
- [x] Nested styles as a view tree (`View::text` / `block` / `row` / `column`) resolved in one layout pass, rather than re-styling already-rendered text
- [x] Theme layers: reusable core components plus CLI-specific presentations derived from shared semantic tokens
- [x] `urushi-tui`: box-model Widget and loss-aware Ratatui style conversion
- [ ] `urushi-tui`: TEA-style full-screen runtime owning event delivery, frame scheduling, and terminal lifecycle
- [x] `urushi-prompt`: themed `Form` / `Group` with `Input`, `Select`, `Confirm`, and synchronous validation

## Acknowledgments

The API design takes [lipgloss](https://github.com/charmbracelet/lipgloss) as
its primary reference; `urushi-prompt` does the same with
[huh](https://github.com/charmbracelet/huh). Thanks to the
[Charm](https://charm.sh) team for showing what a coherent terminal UI stack
looks like.

## License

MIT or Apache-2.0, at your option.
