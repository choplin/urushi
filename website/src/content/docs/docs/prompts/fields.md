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

The configured field appears as one focused region. The placeholder and
description use muted theme roles:

<pre class="terminal-preview" aria-label="Focused input with muted description and placeholder"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Project name</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">Used as the generated directory name.</span>
<span class="ansi-cyan">┃ ›</span> <span class="ansi-dim">my-project</span></code></pre>

Submitting the empty value runs the required check before custom validators:

<pre class="terminal-preview" aria-label="Required input validation result"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Project name</span>
<span class="ansi-cyan">┃ ›</span>
<span class="ansi-cyan">┃</span> <span class="ansi-red ansi-bold">! Enter a project name.</span></code></pre>

Validators are synchronous and run against the `String` value. Multiple
validators run in insertion order.

If the user enters `my project`, the validator adds the error below the
control without replacing the entered value:

<pre class="terminal-preview" aria-label="Custom validator result below the retained input"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Project name</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">Used as the generated directory name.</span>
<span class="ansi-cyan">┃ ›</span> my project
<span class="ansi-cyan">┃</span> <span class="ansi-red ansi-bold">! Use a name without spaces.</span></code></pre>

### Edit the one-line value

Input editing follows terminal grapheme boundaries, so a combined emoji or a
base character plus combining mark moves and deletes as one unit.

| Action | Keys |
|---|---|
| Move one grapheme | Left / Right, Ctrl-B / Ctrl-F |
| Move to an edge | Home / End, Ctrl-A / Ctrl-E |
| Delete one grapheme | Backspace / Delete |
| Delete to an edge | Ctrl-U / Ctrl-K |
| Validate and continue | Enter / Tab |
| Return to the previous field | Shift-Tab |
| Cancel the form | Esc |

Typing or pasting inserts at the cursor and clears a displayed validation
error. Because `Input` is one line, paste converts CR, LF, and tab boundaries
to spaces and removes other control characters. For example, pasting
`api\tserver\r\nrelease` at the cursor produces this observable state:

<pre class="terminal-preview" aria-label="One-line input after tab and newline paste normalization"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Project name</span>
<span class="ansi-cyan">┃ ›</span> api server release<span class="ansi-reverse"> </span></code></pre>

Moving left twice and pressing Delete removes the grapheme under the cursor;
Ctrl-U then removes everything before it. Validation runs only when Enter or
Tab attempts to advance.

## Typed selection

`Select<T>` stores values independently from their visible labels.

```rust
use urushi_prompt::{FieldKey, Select, SelectOption};

#[derive(Debug)]
enum Profile {
    Debug,
    Release,
    Test,
    Bench,
    Minimal,
    Production,
}

let profile_key = FieldKey::new("profile");
let profile = Select::new(
    profile_key,
    "Build profile",
    vec![
        SelectOption::new("Debug", Profile::Debug),
        SelectOption::new("Release", Profile::Release),
        SelectOption::new("Test", Profile::Test),
        SelectOption::new("Bench", Profile::Bench),
        SelectOption::new("Minimal", Profile::Minimal),
        SelectOption::new("Production", Profile::Production),
    ],
)?.visible_rows(3)
  .filter_help("type filter • enter apply", "enter choose • / edit")
  .no_matches_message("No build profiles match");
# Ok::<(), urushi_prompt::FieldConfigError>(())
```

The first option starts selected. Three rows are visible; moving farther scrolls
the window instead of increasing the field height:

<pre class="terminal-preview" aria-label="Select window after scrolling to Bench"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Build profile</span>
<span class="ansi-cyan">┃</span>   Test
<span class="ansi-cyan">┃ ›</span> <span class="ansi-cyan ansi-bold">Bench</span>
<span class="ansi-cyan">┃</span>   Minimal
<span class="ansi-cyan">┃</span> <span class="ansi-dim">  ↑ 2 • ↓ 1</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">↑/↓ select • enter continue • shift+tab back • esc cancel</span></code></pre>

Press `/` to edit a case-insensitive substring filter. Typing `pro` narrows the
list while showing the editing help configured above:

<pre class="terminal-preview" aria-label="Select filtered to Production"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Build profile</span>  / pro<span class="ansi-reverse"> </span>
<span class="ansi-cyan">┃ ›</span> <span class="ansi-cyan ansi-bold">Production</span>
<span class="ansi-cyan">┃</span>
<span class="ansi-cyan">┃</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">  = 1</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">type filter • enter apply</span></code></pre>

When no label contains the filter, the configured message replaces the option
window without inventing a submitted value:

<pre class="terminal-preview" aria-label="Select filter with no matching options"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Build profile</span>  / xyz<span class="ansi-reverse"> </span>
<span class="ansi-cyan">┃</span> <span class="ansi-red ansi-bold">No build profiles match</span>
<span class="ansi-cyan">┃</span>
<span class="ansi-cyan">┃</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">  = 0</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">type filter • enter apply</span></code></pre>

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

The two choices share one left-aligned control row. With no default, neither is
selected initially:

<pre class="terminal-preview" aria-label="Unanswered confirmation"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Overwrite the existing file?</span>
<span class="ansi-cyan">┃</span>
<span class="ansi-cyan">┃</span>   Overwrite     Keep</code></pre>

Pressing `Enter` before choosing keeps the field active and displays the
configured error:

<pre class="terminal-preview" aria-label="Unanswered confirmation validation message"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Overwrite the existing file?</span>
<span class="ansi-cyan">┃</span>   Overwrite     Keep
<span class="ansi-cyan">┃</span> <span class="ansi-red ansi-bold">! Choose an action.</span></code></pre>

Pressing `Right` or `n` explicitly chooses `Keep`; submission then returns
`ConfirmAnswer { value: false, source: ConfirmSource::Explicit }`:

<pre class="terminal-preview" aria-label="Keep explicitly selected"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Overwrite the existing file?</span>
<span class="ansi-cyan">┃</span>   Overwrite     <span class="ansi-cyan ansi-bold">Keep</span></code></pre>

`ConfirmAnswer::value` is the yes-or-no result. `ConfirmAnswer::source` tells
whether the user made an explicit choice or submitted the configured default.
When no default exists, the field remains active until the user chooses.

## Help and descriptions

Every field supports a description and configurable navigation help. Select
fields provide separate help for editing and applying a filter:

```rust
# use urushi_prompt::{FieldKey, Input, Select, SelectOption};
let input = Input::new(FieldKey::new("name"), "Name", "")?
    .description("Used in the greeting.")
    .help("enter next • esc cancel");

let select = Select::new(
    FieldKey::new("region"),
    "Region",
    vec![SelectOption::new("Tokyo", "ap-northeast-1")],
)?
.help("↑/↓ choose • enter next")
.filter_help("type filter • enter apply", "enter next • / edit");
# let _ = (input, select);
# Ok::<(), urushi_prompt::FieldConfigError>(())
```

<pre class="terminal-preview" aria-label="Custom input description and navigation help"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">Name</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">Used in the greeting.</span>
<span class="ansi-cyan">┃ ›</span>
<span class="ansi-cyan">┃</span> <span class="ansi-dim">enter next • esc cancel</span></code></pre>

Keep help short enough to remain useful at narrow widths. See the
[prompt reference](/docs/prompts/reference/) for all defaults and builders.
