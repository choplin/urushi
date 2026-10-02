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

The key types determine the types returned after submission:

| Key | Submitted type |
|---|---|
| `name_key` | `String` |
| `style_key` | `String` |
| `proceed_key` | `ConfirmAnswer` |

An empty key name returns `FieldConfigError::EmptyName`; duplicate names return
`FormBuildError::DuplicateFieldName` when the form is built.

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

This group renders all three fields in insertion order:

<pre class="terminal-preview" aria-label="Greeting group with input, select, and confirmation fields"><code><span class="ansi-bold">Greeting setup</span>
<span class="ansi-dim">Choose how the greeting should be generated.</span>

<span class="ansi-cyan">┃</span> <span class="ansi-bold">What is your name?</span>
<span class="ansi-cyan">┃ ›</span> <span class="ansi-dim">e.g. Alex</span>

  Choose a greeting style
  › Friendly
    Formal

  Generate the greeting?
    Yes     No</code></pre>

Builder validation happens before terminal interaction: an empty group returns
`GroupBuildError::EmptyGroup`, an empty select option list returns
`FieldConfigError::EmptyOptions`, and duplicate field names are rejected when
the form is built.

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
active field is marked by the accent rail; muted help and placeholders retain
their theme roles.

### 1. Enter the name

<pre class="terminal-preview" aria-label="Alex typed into the focused name field"><code><span class="ansi-bold">Greeting setup</span>
<span class="ansi-dim">Choose how the greeting should be generated.</span>

<span class="ansi-cyan">┃</span> <span class="ansi-bold">What is your name?</span>
<span class="ansi-cyan">┃ ›</span> Alex<span class="ansi-reverse"> </span>

  Choose a greeting style
  › Friendly
    Formal

  Generate the greeting?
    Yes     No

  <span class="ansi-dim">enter continue • shift+tab back • esc cancel</span></code></pre>

### 2. Advance and choose a style

After `Enter`, the input is accepted and focus moves to the select. `Down`
makes `Formal` the typed selection:

<pre class="terminal-preview" aria-label="Formal selected in the focused greeting style field"><code><span class="ansi-bold">Greeting setup</span>
<span class="ansi-dim">Choose how the greeting should be generated.</span>

  What is your name?
  › Alex

<span class="ansi-cyan">┃</span> <span class="ansi-bold">Choose a greeting style</span>
<span class="ansi-cyan">┃</span>   Friendly
<span class="ansi-cyan">┃ ›</span> <span class="ansi-cyan ansi-bold">Formal</span>

  Generate the greeting?
    Yes     No

  <span class="ansi-dim">↑/↓ select • enter continue • shift+tab back • esc cancel</span></code></pre>

### 3. Accept the confirmation default

`Enter` advances to confirmation. Because this field has `Some(true)`, `Yes`
is selected as the default and submitting it records `ConfirmSource::Default`:

<pre class="terminal-preview" aria-label="Yes default focused in confirmation field"><code>  What is your name?
  › Alex

  Choose a greeting style
    Friendly
  › Formal

<span class="ansi-cyan">┃</span> <span class="ansi-bold">Generate the greeting?</span>
<span class="ansi-cyan">┃</span>   <span class="ansi-cyan ansi-bold">Yes</span>     No

  <span class="ansi-dim">←/→ choose • y yes • n no • enter submit • shift+tab back • esc cancel</span></code></pre>

### 4. Resume the command

Submitting produces `FormOutcome::Submitted`; the code above prints:

```text title="Program output"
formal greeting for Alex
```

Pressing `Esc` at any active field instead produces `FormOutcome::Cancelled`
and the code prints `Cancelled.` to stderr. No partial `FormValues` is exposed.

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

If OSC 11 reports a light background, `themes.select` supplies the light theme;
if the query has no usable reply, this example supplies the dark fallback. The
prompt still has the same structure, but the selected semantic palette changes:

<pre class="terminal-preview" aria-label="Prompt roles after selecting a terminal-aware theme"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">What is your name?</span>
<span class="ansi-cyan">┃ ›</span> <span class="ansi-dim">e.g. Alex</span>
  <span class="ansi-dim">enter continue • shift+tab back • esc cancel</span></code></pre>

`ThemeMode::Auto` classifies a successful OSC 11 observation and uses the
explicit fallback when the terminal provides no usable reply. The form then
queries size and rendering capabilities, draws the inline interaction, and
restores the caller-owned connection without querying its background again.

Next, configure [fields and validation](/docs/prompts/fields/) or control
[placement and resize behavior](/docs/prompts/placement/). The
[prompt reference](/docs/prompts/reference/) lists every default and error.
