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

```rust
# use urushi_prompt::{Form, PromptStart};
let form = Form::builder()
    .start(PromptStart::NewLine)
    .group(group)
    .build()?;
# let _ = form;
# Ok::<(), urushi_prompt::FormBuildError>(())
```

The prefix remains untouched and the prompt begins on the following line:

<pre class="terminal-preview" aria-label="Prompt starting on a new line"><code>Configuration:
<span class="ansi-cyan">┃</span> <span class="ansi-bold">What is your name?</span>
<span class="ansi-cyan">┃ ›</span> <span class="ansi-dim">e.g. Alex</span></code></pre>

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

<pre class="terminal-preview" aria-label="Prompt beginning after an existing ASCII prefix"><code>Configuration: <span class="ansi-cyan">┃</span> <span class="ansi-bold">What is your name?</span>
               <span class="ansi-cyan">┃ ›</span> <span class="ansi-dim">e.g. Alex</span></code></pre>

The byte length is a valid column only because this prefix is ASCII. For other
text, compute terminal display width rather than using `str::len`.

Use `CurrentLine` when the complete current line belongs to the application and
may be overwritten:

```rust
# use urushi_prompt::{Form, PromptStart};
let form = Form::builder()
    .start(PromptStart::CurrentLine)
    .group(group)
    .build()?;
# let _ = form;
# Ok::<(), urushi_prompt::FormBuildError>(())
```

<pre class="terminal-preview" aria-label="Current line replaced by a prompt at column zero"><code><span class="ansi-dim">before: Configuration: waiting…</span>
<span class="ansi-cyan">after:  ┃</span> <span class="ansi-bold">What is your name?</span></code></pre>

`width` caps the prompt region; the actual width is also limited by the columns
remaining after its left edge. At a width of 20 cells, long content wraps:

<pre class="terminal-preview" aria-label="Prompt wrapped to a twenty-cell region"><code><span class="ansi-cyan">┃</span> <span class="ansi-bold">What is your</span>
<span class="ansi-cyan">┃</span> <span class="ansi-bold">name?</span>
<span class="ansi-cyan">┃ ›</span> <span class="ansi-dim">e.g. Alexandra</span></code></pre>

## Handle resize explicitly

Inline content can reflow in the primary buffer, so a prompt may no longer know
where its previously drawn rows moved after a resize. The default policy is
`InlineResizePolicy::ReturnError`, which restores the session and returns
`RunError::Resized` without erasing an uncertain region:

```rust
use urushi_prompt::{Form, InlineResizePolicy, RunError};

let form = Form::builder()
    .inline_resize_policy(InlineResizePolicy::ReturnError)
    .group(group)
    .build()?;

match form.run(&theme) {
    Err(RunError::Resized { cleanup: None }) => {
        eprintln!("Prompt stopped after terminal resize.");
    }
    Err(error) => return Err(error.into()),
    Ok(outcome) => { /* handle submission or cancellation */ }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

```text title="Program output after resize"
Prompt stopped after terminal resize.
```

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

The transition is deliberately destructive inside the visible viewport:

<pre class="terminal-preview" aria-label="Viewport before and after clear and redraw resize policy"><code><span class="ansi-bold">before resize</span>
build log line 1
build log line 2
<span class="ansi-cyan">┃</span> Name

<span class="ansi-bold">after resize</span>
<span class="ansi-dim">(all previous visible cells cleared)</span>
<span class="ansi-cyan">┃</span> <span class="ansi-bold">Name</span>
<span class="ansi-cyan">┃ ›</span></code></pre>

The prompt remains running and redraws its current field state. Scrollback is
not cleared. Compare every placement and resize default in the
[prompt reference](/docs/prompts/reference/).
