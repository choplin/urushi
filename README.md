# Urushi

Urushi is a UI library for Rust terminal applications. It gives command output,
interactive prompts, and full-screen TUIs one shared presentation foundation
while each surface keeps its own interaction and terminal lifecycle.

**Status:** Urushi 0.1.0 is in early development. Public APIs may change before
1.0.

## Why Urushi?

- **Keep one visual language.** Semantic themes, styles, components, and views
  can be reused across static output, prompts, and full-screen interfaces.
- **Use only the surface you need.** Write once to stdout, run a blocking form,
  let Urushi own a TEA-style application loop, or adapt a view to an existing
  Ratatui application.
- **Compose before rendering.** Renderer-neutral `View` values preserve layout
  and styling decisions until the target terminal surface is known.
- **Treat terminal width as UI geometry.** Layout, wrapping, borders, and
  alignment account for grapheme clusters and East Asian wide characters.

## Quickstart

Urushi 0.1.0 requires Rust 1.90 or newer. Add the core crate:

```sh
cargo add urushi
```

Build a bordered status view and write it to stdout:

```rust
use urushi::{BlockStyle, Border, Color, TextStyle, View};

fn status_view() -> View {
    let text = TextStyle::new()
        .foreground(Color::GREEN)
        .bold();

    let panel = BlockStyle::new()
        .border(Border::ROUNDED)
        .border_foreground(Color::BRIGHT_BLACK)
        .padding((0, 1));

    View::block(panel, View::text("Build complete", text))
}

fn main() -> std::io::Result<()> {
    urushi::println_view(&status_view())
}
```

Run the program:

```sh
cargo run
```

The result is a terminal-aware panel:

```text
╭────────────────╮
│ Build complete │
╰────────────────╯
```

On a capable terminal, the message is green and bold and the border is bright
black. When stdout is redirected, Urushi keeps the layout but omits ANSI
styling. `NO_COLOR=1` disables color without discarding other supported text
attributes.

For a guided explanation of this example, follow the
[quickstart](website/src/content/docs/docs/quickstart.md).

## Choose a terminal surface

Every surface uses the same presentation foundation, but each keeps the
interaction and terminal lifecycle appropriate to its job.

| What you are building | Start with | Who owns control flow |
| --- | --- | --- |
| Styled command output and reusable layouts | `urushi` | Your program writes a view and continues. |
| Opinionated summaries and warnings | `urushi-cli` | Your program writes semantic CLI presentations. |
| Input, selection, and confirmation forms | `urushi-prompt` | The prompt temporarily owns a blocking terminal session. |
| A TEA-style full-screen application | `urushi-tui-app` | Urushi owns delivery, effects, drawing, and terminal restoration. |
| A caller-driven synchronous frame loop | `urushi-tui` | Your application owns input, timing, and the terminal session. |
| UI inside an existing Ratatui application | `urushi-adapter-ratatui` | Your application keeps its Ratatui terminal and event loop. |
| Kitty or Sixel images with text fallback | `urushi-graphics` | Your CLI, prompt, or TUI host owns when images are presented. |

See [Choose by use case](website/src/content/docs/docs/use-cases.md) for the
dependency set and first task for each surface.

## How the shared model works

Urushi shares presentation, not control flow:

```text
Theme → component presentation → View → layout → terminal surface
```

- A `Theme` gives semantic roles a consistent visual meaning.
- Components turn application data into renderer-neutral `View` values.
- One layout pass resolves the box model and terminal-cell geometry.
- The selected surface writes static output, runs a prompt, presents frames, or
  adapts cells to Ratatui.

This boundary lets a CLI summary, prompt, and TUI use the same theme without
pretending that they have the same interaction model. Read the
[presentation foundation](website/src/content/docs/docs/concepts/presentation-foundation.md)
for the complete model.

## Run the examples

From a checkout of this repository, the smallest visual catalog is:

```sh
cargo run -p urushi --example showcase
```

The CJK catalog exercises East Asian width and alignment:

```sh
cargo run -p urushi --example cjk_showcase
```

Interactive and full-screen examples require a real terminal:

```sh
cargo run -p urushi-prompt --example wizard
cargo run -p urushi-tui-app --example runtime_counter
```

## Workspace crates

| Crate | Responsibility |
| --- | --- |
| [`urushi`](urushi/) | Styles, themes, components, renderer-neutral views, layout, ANSI rendering, and standard-stream output. |
| [`urushi-cli`](urushi-cli/) | Opinionated presentations for human-facing, non-interactive CLI output. |
| [`urushi-prompt`](urushi-prompt/) | Typed input, selection, and confirmation forms with validation. |
| [`urushi-tui`](urushi-tui/) | Synchronous frames, cell buffers, diffing, and transactional terminal output. |
| [`urushi-tui-app`](urushi-tui-app/) | TEA-style applications, effects, subscriptions, scheduling, input, and terminal lifecycle. |
| [`urushi-adapter-ratatui`](urushi-adapter-ratatui/) | Stateless Urushi view and style adapters for caller-owned Ratatui buffers. |
| [`urushi-graphics`](urushi-graphics/) | Image components and Kitty/Sixel terminal graphics with text fallback. |
| [`urushi-terminal`](urushi-terminal/) | Shared terminal commands, events, geometry, capabilities, and session restoration. |
| [`urushi-derive`](urushi-derive/) | Derive macros used by typed Urushi components. |

All workspace crates in one release use the same version. Add only the crates
that correspond to the surfaces your application presents.

## Documentation

- [Changelog](CHANGELOG.md)
- [Quickstart](website/src/content/docs/docs/quickstart.md)
- [Choose by use case](website/src/content/docs/docs/use-cases.md)
- [Crates and Cargo features](website/src/content/docs/docs/reference/crates.md)
- [Current limitations](website/src/content/docs/docs/reference/limitations.md)
- [Developer architecture](docs/architecture.md)

The 0.1.0 release gate checks default and all-feature builds on Linux, macOS,
and Windows, and checks the complete feature graph with Rust 1.90. Decoration
still depends on detected terminal capabilities, so applications must not use
color, attributes, hyperlinks, or graphics as the only carrier of meaning.

## Acknowledgments

The API design takes [Lip Gloss](https://github.com/charmbracelet/lipgloss) as
its primary reference; `urushi-prompt` does the same with
[Huh](https://github.com/charmbracelet/huh). Thanks to the
[Charm](https://charm.sh) team for demonstrating how a coherent terminal UI
stack can feel.

## License

Urushi is licensed under the [MIT License](LICENSE-MIT).
