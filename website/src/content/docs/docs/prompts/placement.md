---
title: Placement and resize
description: Choose where an inline prompt starts, limit its width, and handle terminal resize safely.
---

The 0.1.0 prompt runtime draws inline into the primary terminal buffer. The form
must know the left edge of the region it owns. These placement and resize
options are specific to inline presentation; they are not general form-state
settings.

## Start on a new line

`PromptStart::NewLine` is the default. It starts at column zero after emitting
a carriage return and line feed. Prefer it when no surrounding line content
must be preserved.

## Start at the current cursor position

The application supplies the current column because the prompt does not query
cursor position.

```rust
use std::io::{self, Write};
use urushi_prompt::{Form, PromptStart};

const PREFIX: &str = "Configuration: ";

let form = Form::builder()
    .start(PromptStart::CurrentPosition {
        column: PREFIX.len() as u16,
    })
    .width(32)
    .group(group)
    .build()?;

let stderr = io::stderr();
let mut output = stderr.lock();
write!(output, "{PREFIX}")?;
output.flush()?;
drop(output);

let outcome = form.run(&theme)?;
# let _ = outcome;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The prompt owns only the suffix beginning after `Configuration: `:

```text title="Inline placement"
Configuration: ┃ What is your name?
               ┃ › e.g. Alex
```

The byte length is a valid column only because this prefix is ASCII. For other
text, compute terminal display width rather than using `str::len`.

`PromptStart::CurrentLine` starts at column zero and overwrites the current
line. `width` caps the prompt region; the actual width is also limited by the
remaining terminal columns.

## Handle resize explicitly

Inline content can reflow in the primary buffer, so a prompt may no longer know
where its previously drawn rows moved after a resize. The default policy is
`InlineResizePolicy::ReturnError`, which restores the session and returns
`RunError::Resized` without erasing an uncertain region.

```rust
use urushi_prompt::InlineResizePolicy;

let form = Form::builder()
    .inline_resize_policy(InlineResizePolicy::ClearViewportAndRedraw)
    .group(group)
    .build()?;
# let _ = form;
# Ok::<(), urushi_prompt::FormBuildError>(())
```

`ClearViewportAndRedraw` keeps the prompt running, but it clears every visible
cell in the primary-buffer viewport, including content outside the prompt. Use
it only when the application accepts that behavior.
