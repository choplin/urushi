---
title: CLI presentations
description: Apply an opinionated visual language to human-facing, non-interactive command output.
---

`urushi-cli` provides an opinionated visual language for non-interactive,
human-facing command output. It owns CLI-specific structure such as rails,
status glyphs, title hierarchy, and semantic roles while composing everything
into ordinary Urushi `View` values.

The 0.1.0 release starts with `Summary` and `Warning`. They are the currently
available presentations, not the complete scope of the crate. `urushi-cli`
does not define logging levels, prompts, live progress, or a full-screen
runtime.

## Install the CLI surface

```sh
cargo add urushi urushi-cli
```

## Derive a CLI theme

`CliTheme` derives its body, muted, accent, and warning roles from a core
`Theme`. The resulting components therefore match output from other Urushi
surfaces.

```rust
use urushi_cli::CliTheme;

let cli = CliTheme::from_theme(&theme);
```

## Presentations available in 0.1.0

### Print a result summary

```rust
use urushi_cli::{CliTheme, Summary};

let cli = CliTheme::from_theme(&theme);
let view = cli.summary(
    &Summary::new("Build complete")
        .field("Target", "aarch64-apple-darwin")
        .field("Profile", "release")
        .field("Output", "target/release/app"),
);

urushi::println_view(&view)?;
# Ok::<(), std::io::Error>(())
```

The canonical summary uses a vertical rail, a status glyph, and aligned field
labels:

```text title="Rendered output"
│
◇  Build complete
│  Target   aarch64-apple-darwin
│  Profile  release
│  Output   target/release/app
```

The component returns an ordinary `View`. It reflows labels and values when the
available terminal width changes, including CJK labels and values.

### Print a warning

```rust
use urushi_cli::Warning;

let view = cli.warning(&Warning::new(
    "Existing file",
    "The previous report will be replaced.",
));

urushi::eprintln_view(&view)?;
# Ok::<(), std::io::Error>(())
```

```text title="Rendered output"
│
▲  Existing file
│  The previous report will be replaced.
```

Warnings are presentation, not delivery policy. Decide in the application
whether a message goes to stdout, stderr, a logger, or nowhere.

## Override one CLI role

Use `CliTheme::style` when the canonical mapping needs one surface-specific
adjustment.

```rust
use urushi::{Color, TextStyle};
use urushi_cli::CliRole;

let cli = CliTheme::from_theme(&theme).style(
    CliRole::Warning,
    TextStyle::new().foreground(Color::BRIGHT_YELLOW).bold(),
);
```

Keep the core `Theme` as the default source of truth. Override a CLI role only
when that semantic role genuinely differs on this surface.
