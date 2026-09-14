# CLI Presentation

`urushi-cli` provides an opinionated visual language for human-facing,
non-interactive command output. It owns semantic values such as a completed
operation summary or a warning, and presentations that turn those values into
the renderer-neutral `View` model supplied by `urushi`.

The crate exists because this vocabulary is useful across command-line
applications but is not a universal rendering primitive. Rails, status glyphs,
title hierarchy, and the choice of semantic roles express a CLI product
language. Keeping them outside core lets `urushi` remain the reusable style,
layout, and rendering substrate.

## Place in the architecture

`urushi-cli` sits between application meaning and the core `View` pipeline:

```text
Theme
  |
  | CliTheme::from_theme
  v
CliTheme + Summary / Warning
  |
  | summary / warning
  v
View
```

The result is an ordinary `View`. The application passes it to the same
`resolve`, `render`, or standard-stream convenience path as a view built
directly from core primitives. `urushi-cli` does not inspect the terminal,
select an output width, emit ANSI, or write to a stream.

## Semantic data and presentation

`Summary` and `Warning` hold the meaning an application wants to communicate:

- `Summary` has a title and an ordered collection of labelled fields.
- `Warning` has a title and a message.

They do not contain colors, terminal widths, or pre-rendered strings.
`SummaryPresentation` and `WarningPresentation` own the structural and styling
choices that turn those values into a `View`. Their `compose` operations do not
receive `Available`; width-dependent decisions remain in core layout.

Both canonical presentations use ordinary `View` primitives. Summary expresses
its labelled fields as a Grid inside a left-bordered Block, so shared columns,
wrapping, and row height follow core layout. Warning uses a Row inside the same
kind of bordered Block, so its rail grows with the resolved body height. The
general presentation contract lives in
[`design/component-presentation.md`](design/component-presentation.md).

## Theme derivation

`CliTheme` derives its initial styles from a core `Theme`. Body, muted, and
accent roles reuse the corresponding core component roles; the warning role
uses the core warning semantic token. This keeps one application color system
without making CLI-specific roles or presentations part of `ComponentTheme`.

The ordinary path is deliberately short:

```rust
use urushi_cli::{CliTheme, Summary, Warning};

let cli = CliTheme::from_theme(&theme);
let result = cli.summary(
    &Summary::new("Done").field("Output", "report.json"),
);
let caution = cli.warning(
    &Warning::new("Overwrite", "The old file will be replaced"),
);
```

An application may replace one `CliRole` with `with_style`, or replace a whole
canonical presentation with `with_summary` or `with_warning`. It may also hold
a presentation directly and call `compose`. These paths change presentation
policy; they do not change the semantic data or bypass the core `View` model.

## Ownership boundary

Responsibility is divided as follows:

| Owner | Responsibility |
| --- | --- |
| Application | Command workflow, domain meaning, when output is produced, and where it is sent |
| `urushi-cli` | Summary and Warning data, CLI roles, canonical presentation, and composition into `View` |
| `urushi` | Logical styles, theme tokens, text measurement, layout, rendering, and static output helpers |

Logging levels, structured logging, message delivery, live progress, prompts,
and full-screen application behavior remain outside `urushi-cli`. Those
concerns have different state and lifecycle requirements; sharing colors or
text measurement does not make them part of this crate.

The general semantic-data-to-presentation model is described in
[`component-model.md`](component-model.md). The complete workspace and output
flow starts in [`architecture.md`](architecture.md).
