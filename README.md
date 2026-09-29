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
- **Ride the ratatui ecosystem via an adapter.** The
  `urushi-adapter-ratatui` crate draws
  urushi styles and view trees into a ratatui `Buffer`, mapping the
  foreground, background, and attribute subset onto `ratatui::style::Style` and
  keeping the box model
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
| [`urushi`](urushi/) | Style definitions: colors, attributes, padding, margin, borders, alignment, wrapping | Core rendering works |
| [`urushi-cli`](urushi-cli/) | Opinionated `Summary` and `Warning` presentation for human-facing, non-interactive CLI output | Core presentations work |
| [`urushi-graphics`](urushi-graphics/) | Image components and Kitty/Sixel terminal graphics adapters | Stateless Kitty/Sixel output works |
| [`urushi-terminal`](urushi-terminal/) | Generic terminal commands, events, session restoration, geometry, capability inspection, and physical backends | Shared primitives, native Unix backend, and optional Crossterm adapter work |
| [`urushi-prompt`](urushi-prompt/) | Theme-aware `Input`, `Select`, and `Confirm` fields with synchronous validation | Core prompt flow works |
| [`urushi-tui`](urushi-tui/) | Synchronous `Screen` and draw-scoped `Frame` with Urushi-owned cell buffers, diffing, and transactional output | Frame engine works |
| [`urushi-tui-app`](urushi-tui-app/) | TEA-style application runtime owning delivery, effects, subscriptions, drawing, and terminal lifecycle | Runtime and examples work |
| [`urushi-adapter-ratatui`](urushi-adapter-ratatui/) | Stateless style, view, and anchor adapters for caller-owned Ratatui buffers | Adapter and example work |

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
use urushi_adapter_ratatui::{RatatuiStyleExt as _, ViewWidget};

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
use urushi_adapter_ratatui::{available, draw_resolved};

let resolved = resolve(&view, available(area))?;
draw_resolved(&resolved, area, frame.buffer_mut());
```

Two things this approach does not carry yet: a resolved view reports no cursor
position, so an application placing a caret computes it itself, and text inside
a ratatui `Buffer` is plain — ANSI escape sequences are not interpreted there.

### urushi owns the loop, the application describes state

Available today. The TEA-style runtime in `urushi-tui-app` takes an `Application`
whose model is changed only by `update` and whose `view` returns an ordinary
Urushi view tree. `urushi_tui_app::run(app)` supplies the production terminal,
executor, and clock; `Runtime::new(app)` exposes the same entry point as a
builder when a program needs to replace those boundaries or change session
options. The runtime owns ordered delivery, effects and subscriptions, frame
scheduling, terminal input and presentation, and restoration on every normal
or unwinding exit. [`docs/tui-architecture.md`](docs/tui-architecture.md)
defines the complete ownership and ordering model.

Run the small runtime example in a real terminal:

```sh
cargo run -p urushi-tui-app --example runtime_counter
```

Any non-release key updates the model and starts one blocking effect; the
effect completion returns as another message. Resize updates the subscribed
surface, and `q`, Escape, or Ctrl-C shuts down and restores the session.

Enable the `graphics` feature to let the runtime present `Image` components
through the same terminal owner as its cell frame. Automatic selection uses
positively confirmed Kitty support first, then Sixel when uniform cell-pixel
geometry is available, and otherwise leaves the Image text fallback visible.
An explicit override is available for compatibility and diagnostics:

```rust
use urushi_tui_app::{GraphicsPreference, Runtime};

let model = Runtime::new(application)
    .graphics(GraphicsPreference::Sixel)
    .run()?;
```

An unavailable explicit protocol returns a diagnostic error. If selected
graphics later fails, the runtime attempts protocol cleanup, redraws the View's
text fallback as a complete cell frame, and reports the original terminal error
through the runtime's existing terminal-error path.

## Example

Run the English showcase to see one labeled example per styling feature. Each
entry changes one subject at a time, including colors, attributes, spacing,
alignment, joins, and individual border sides:

```sh
cargo run --example showcase
```

Run the Japanese variant for the same one-feature-at-a-time catalog with
CJK-aware width and alignment:

```sh
cargo run --example cjk_showcase
```

### A terminal image

Image support is isolated in `urushi-graphics`; core `urushi` remains unaware
of image data and terminal graphics protocols:

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-terminal = "0.1.0"
```

`ImagePresentation` puts an owned image snapshot, its generic anchored region,
and fallback text into the returned `View`. `render_view` resolves and writes
the complete View, locates its images internally, and queries the terminal for
positive capability evidence. It prefers Kitty, otherwise uses Sixel when
cell-pixel geometry is also available, and otherwise leaves the fallback cells
visible:

```rust
use std::io;

use urushi::{Align, TextStyle, View};
use urushi_graphics::{CellSize, Image, ImagePresentation, PixelSize, render_view};
use urushi_terminal::{
    CommandWriter, TerminalGraphicsProtocol, TerminalQuery,
};

fn logo_view() -> Result<View, Box<dyn std::error::Error>> {
    let image = Image::rgba(
        "logo-placement",
        "logo-rgba",
        PixelSize::new(1, 1),
        [255, 0, 0, 255],
    )?
    .fallback("[logo]");
    let image = ImagePresentation::new().compose(&image, CellSize::new(8, 3));
    Ok(View::column(
        Align::Left,
        [View::text("Build result", TextStyle::new().bold()), image],
    ))
}

fn draw<T: CommandWriter + TerminalQuery>(
    terminal: &mut T,
) -> io::Result<Option<TerminalGraphicsProtocol>> {
    let view = logo_view().map_err(io::Error::other)?;
    render_view(terminal, &view)
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

`TerminalCapabilities::supports_graphics` reports Kitty and Sixel independently,
so a terminal may support both; `render_view` deliberately chooses Kitty in
that case. The one-shot operation is stateless and treats the terminal's
top-left cell as the View origin. It writes backend-independent terminal
commands rather than accessing a physical backend or raw writer. Sixel output
resamples the prepared RGBA raster to the resolved cell rectangle, so it
additionally needs cell-pixel geometry. A cell-only renderer keeps the fallback.
`render_resolved_images` is the lower-level overlay API for a host that already
owns resolution and cell output. Retained uploads, scrolling slices, deletion,
protocol-specific repaint, and draw-failure recovery belong to the TUI or prompt
host that opts into graphics state.

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
urushi-adapter-ratatui = "0.1.0"
```

Resolve a component from the same `Theme` used by plain output, then pass its
widget adapter to a ratatui frame. Colors and attributes stay logical in the
Theme; Ratatui performs its own backend conversion.

```rust
use urushi_adapter_ratatui::RatatuiStyleExt as _;

let panel = theme.block_style(PanelRole::PanelFocused);
frame.render_widget(panel.widget("保存しました"), frame.area());
```

`RatatuiStyle::from(&style)` converts a `TextStyle` when only foreground,
background, and text attributes are needed; that conversion carries no geometry,
because a `TextStyle` has none.

Run the complete Theme → plain CLI / ratatui example with:

```sh
cargo run -p urushi-adapter-ratatui --example themed_ratatui
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

- [x] `TextStyle` / `BlockStyle` builders: colors, attributes, padding, margin, border, width, align
- [x] ANSI-aware width measurement and CJK-aware word wrap
- [x] View composition before layout and rendering
- [x] Workspace-independent terminal contracts, target-specific detection, and feature-granular rendering settings
- [ ] Adaptive colors (light/dark terminal backgrounds)
- [x] Nested styles as a view tree (`View::text` / `block` / `row` / `column`) resolved in one layout pass, rather than re-styling already-rendered text
- [x] Theme layers: reusable core components plus CLI-specific presentations derived from shared semantic tokens
- [x] `urushi-tui`: native synchronous frame engine with transactional cell diffing
- [x] `urushi-tui-app`: TEA-style full-screen runtime owning event delivery, frame scheduling, and terminal lifecycle
- [x] `urushi-adapter-ratatui`: box-model widgets and loss-aware Ratatui style conversion
- [x] `urushi-prompt`: themed `Form` / `Group` with `Input`, `Select`, `Confirm`, and synchronous validation

## Acknowledgments

The API design takes [lipgloss](https://github.com/charmbracelet/lipgloss) as
its primary reference; `urushi-prompt` does the same with
[huh](https://github.com/charmbracelet/huh). Thanks to the
[Charm](https://charm.sh) team for showing what a coherent terminal UI stack
looks like.

## License

MIT or Apache-2.0, at your option.
