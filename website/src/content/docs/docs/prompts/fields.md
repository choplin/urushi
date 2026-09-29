---
title: Fields and validation
description: Configure input, select, and confirmation fields and validate submitted prompt values.
---

All prompt content is plain text. Use the form `Theme` for styling rather than
embedding ANSI sequences in questions, descriptions, labels, or messages.

## Text input

```rust
use urushi_prompt::{FieldKey, Input, ValidationError};

let project_key = FieldKey::new("project");
let project = Input::new(project_key, "Project name", "")?
    .description("Used as the generated directory name.")
    .placeholder("my-project")
    .required()
    .required_message("Enter a project name.")
    .validate(Box::new(|value| {
        if value.contains(' ') {
            Err(ValidationError::new("Use a name without spaces."))
        } else {
            Ok(())
        }
    }));
# Ok::<(), urushi_prompt::FieldConfigError>(())
```

The configured field appears as one focused region. The placeholder uses the
theme's muted role:

```text title="Input field"
┃ Project name
┃ Used as the generated directory name.
┃ › my-project
```

Validators are synchronous and run against the `String` value. Multiple
validators run in insertion order.

If the user enters `my project`, the validator adds the error below the
control without replacing the entered value:

```text title="Validation result"
┃ Project name
┃ Used as the generated directory name.
┃ › my project
┃ ! Use a name without spaces.
```

## Typed selection

`Select<T>` stores values independently from their visible labels.

```rust
use urushi_prompt::{FieldKey, Select, SelectOption};

#[derive(Debug)]
enum Profile {
    Debug,
    Release,
}

let profile_key = FieldKey::new("profile");
let profile = Select::new(
    profile_key,
    "Build profile",
    vec![
        SelectOption::new("Debug", Profile::Debug),
        SelectOption::new("Release", Profile::Release),
    ],
)?.visible_rows(5);
# Ok::<(), urushi_prompt::FieldConfigError>(())
```

The first option starts selected:

```text title="Select field"
┃ Build profile
┃ › Debug
┃   Release
```

The user can filter options; `visible_rows` controls when the list starts
scrolling.

## Confirmation

```rust
use urushi_prompt::{
    Confirm, ConfirmAnswer, FieldKey,
    urushi::Align,
};

let overwrite_key = FieldKey::<ConfirmAnswer>::new("overwrite");
let overwrite = Confirm::new(
    overwrite_key,
    "Overwrite the existing file?",
    None,
)?.labels("Overwrite", "Keep")
  .button_alignment(Align::Left)
  .unanswered_message("Choose an action.");
# Ok::<(), urushi_prompt::FieldConfigError>(())
```

The two choices share one left-aligned control row. The theme distinguishes
the focused answer; the plain geometry is:

```text title="Confirmation field"
┃ Overwrite the existing file?
┃
┃   Overwrite     Keep
```

`ConfirmAnswer::value` is the yes-or-no result. `ConfirmAnswer::source` tells
whether the user made an explicit choice or submitted the configured default.
When no default exists, the field remains active until the user chooses.

## Help and descriptions

Every field supports a description and configurable navigation help. Keep help
short enough to remain useful at narrow terminal widths. Select fields also
provide separate help for editing and applying a filter.
