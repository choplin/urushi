---
title: Build a form
description: Build and run an Urushi prompt form, then retrieve its typed submitted values.
---

This guide builds a form that asks for a name, a greeting style, and final
confirmation.

## Create typed keys

Keep each key so the submitted value can be retrieved with its correct type.

```rust
use urushi_prompt::{ConfirmAnswer, FieldKey};

let name_key = FieldKey::<String>::new("name");
let style_key = FieldKey::<String>::new("style");
let proceed_key = FieldKey::<ConfirmAnswer>::new("proceed");
```

Field names must be non-empty and unique within the form.

## Build the fields and group

```rust
use urushi_prompt::{Confirm, Group, Input, Select, SelectOption};

let group = Group::builder()
    .title("Greeting setup")
    .description("Choose how the greeting should be generated.")
    .field(
        Input::new(name_key.clone(), "What is your name?", "")?
            .placeholder("e.g. Alex")
            .required(),
    )
    .field(Select::new(
        style_key.clone(),
        "Choose a greeting style",
        vec![
            SelectOption::new("Friendly", "friendly".to_owned()),
            SelectOption::new("Formal", "formal".to_owned()),
        ],
    )?)
    .field(Confirm::new(
        proceed_key.clone(),
        "Generate the greeting?",
        Some(true),
    )?)
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Builder validation rejects empty groups, empty field names, and an empty select
option list before terminal interaction begins.

## Build and run the form

```rust
use urushi_prompt::{Form, FormOutcome};

let form = Form::builder().group(group).build()?;

match form.run(&theme)? {
    FormOutcome::Submitted(values) => {
        let name = values.get(&name_key).expect("submitted name");
        let style = values.get(&style_key).expect("submitted style");
        let proceed = values
            .get(&proceed_key)
            .expect("submitted confirmation");

        if proceed.value {
            println!("{style} greeting for {name}");
        }
    }
    FormOutcome::Cancelled => eprintln!("Cancelled."),
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

When `Form::run` opens this form, all three fields share one inline region. The
active field is marked by the rail; colors and text attributes come from the
supplied theme:

```text title="Initial prompt"
Greeting setup
Choose how the greeting should be generated.

┃ What is your name?
┃ › e.g. Alex

  Choose a greeting style
  › Friendly
    Formal

  Generate the greeting?

    Yes     No

  enter continue • shift+tab back • esc cancel
```

`FormValues::get` returns `None` if the key name or type does not match. A form
that reaches `Submitted` contains every accepted field value; cancellation
exposes no partial values.

## Use the application theme

Pass the same `Theme` used for static output or a Ratatui adapter. Prompt roles
are derived from that supplied theme when the form starts. Terminal
capabilities determine which parts of the resulting logical styles can be
rendered; they do not select or replace the theme.

`Form::run(&theme)` opens the default terminal and uses the theme unchanged. It
does not query the terminal background. When automatic light/dark selection is
needed, query a caller-owned connection first and run the form on that same
connection:

Add `urushi-terminal = "0.1.0"` when the application opens the connection
itself. The native connection shown here is available on Unix.

```rust
use urushi_prompt::{
    FormOutcome,
    urushi::{ColorScheme, ThemeMode},
};
use urushi_terminal::{TerminalQuery as _, backend::native::NativeTerminal};

let mode = ThemeMode::Auto {
    fallback: ColorScheme::Dark,
};
let mut terminal = NativeTerminal::open()?;
let background = terminal.terminal_background()?;
let theme = themes.select(mode.resolve(background));

match form.run_with_terminal(&mut terminal, theme)? {
    FormOutcome::Submitted(values) => {
        // Read the same typed keys used to build the form.
    }
    FormOutcome::Cancelled => eprintln!("Cancelled."),
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

`ThemeMode::Auto` classifies a successful OSC 11 observation and uses the
explicit fallback when the terminal provides no usable reply. The form then
queries size and rendering capabilities, draws the inline interaction, and
restores the caller-owned connection without querying its background again.

Next, configure [fields and validation](/docs/prompts/fields/) or control
[placement and resize behavior](/docs/prompts/placement/).
