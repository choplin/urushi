---
title: Prompt reference
description: Look up prompt fields, builders, defaults, outcomes, placement, resize policies, and errors.
---

Use this page after the [prompt overview](/docs/prompts/). Prompt content is
plain text; the `Theme` supplied to `run` or `run_with_terminal` provides its
styles.

## Typed keys and outcomes

`FieldKey<T>::new(name)` identifies a submitted value by both its name and Rust
type. Names must be non-empty and unique within a form. Clone a key before
passing it into a field when the caller will later retrieve its value.

`FormOutcome` has two variants:

| Variant | Meaning |
|---|---|
| `Submitted(FormValues)` | Every field was accepted. `get(&key)` returns `Some(&T)` only when both name and type match. |
| `Cancelled` | The user cancelled. No partial values are exposed. |

## Input

`Input::new(key, question, initial_value)` creates a one-line `String` field.

| Builder/accessor | Effect or default |
|---|---|
| `placeholder(text)` | Text shown only while the value is empty. Default: none. |
| `description(text)` | Supporting text below the question. Default: none. |
| `help(text)` | Navigation hint. Default: `enter continue • shift+tab back • esc cancel`. |
| `required()` | Reject an empty value. Default: false. |
| `required_message(text)` | Required error. Default: `This field is required.` |
| `validate(validator)` | Append a synchronous `String` validator. Runs after the required check, in insertion order. |
| `question()` | Borrow the question. |
| `value()` | Borrow the current value. |

Validators return `Result<(), ValidationError>`. Rejection retains the entered
value and displays the error under the control.

Editing is grapheme-aware. Left/Right or Ctrl-B/Ctrl-F move one grapheme;
Home/End or Ctrl-A/Ctrl-E move to an edge; Backspace/Delete remove the previous
or current grapheme; Ctrl-U/Ctrl-K erase to the start or end. Ordinary typing
and paste insert at the cursor. A paste changes CR, LF, and tab boundaries to
spaces and drops other control characters so the value remains one line.
Enter or Tab validates and advances, Shift-Tab moves to the previous field, and
Esc cancels the form. Any value change clears the currently displayed
validation error; moving the cursor alone does not. Submission runs the
required check before validators.

The form runtime admits key press and repeat events, paste, and resize. It
ignores key release, mouse, and terminal-focus events, including when a
caller-owned backend reports them.

## Select

`Select<T>::new(key, question, options)` requires at least one
`SelectOption::new(label, value)`. The first option is selected initially;
labels and typed values are stored separately.

| Builder | Effect or default |
|---|---|
| `description(text)` | Supporting text. Default: none. |
| `help(text)` | Unfiltered navigation hint. Default: `↑/↓ select • enter continue • shift+tab back • esc cancel`. |
| `filter_help(editing, applied)` | Hints while editing and after applying a non-empty filter. Defaults: `type to filter • ↑/↓ select • enter apply • esc close`, then `↑/↓ select • enter continue • / edit filter • esc clear`. |
| `no_matches_message(text)` | Empty-result message. Default: `No matches`. |
| `visible_rows(rows)` | Maximum option rows before scrolling. Default: 7; zero normalizes to 1. |

`/` opens filter editing. Filtering is case-insensitive substring matching.
`Enter` or `Tab` applies an edited filter, then accepts the current option when
not editing. `Esc` closes editing, then clears a non-empty applied filter, then
cancels. Arrow keys and `j`/`k` move; Home/`g` and End/`G` jump; Ctrl-U/Ctrl-D
page through the filtered list.

## Confirm

`Confirm::new(key, question, default)` submits a `ConfirmAnswer`.

| Builder | Effect or default |
|---|---|
| `labels(yes, no)` | Choice labels. Defaults: `Yes`, `No`. |
| `button_alignment(align)` | Alignment within the natural question/description width. Default: `Align::Left`. |
| `description(text)` | Supporting text. Default: none. |
| `help(text)` | Navigation hint. Default: `←/→ choose • y yes • n no • enter submit • shift+tab back • esc cancel`. |
| `unanswered_message(text)` | Error when there is no default or explicit choice. Default: `Choose yes or no.` |

`ConfirmAnswer::value` is the Boolean choice. `source` is
`ConfirmSource::Default` when `Enter` accepts a configured default and
`ConfirmSource::Explicit` after an arrow, `h`/`l`, or `y`/`n` selection.

## Groups and forms

| Builder | Members and defaults |
|---|---|
| `Group::builder()` | `title`, `description`, repeated `field`, then `build`; at least one field is required. |
| `Form::builder()` | repeated `group`, `start`, `width`, `inline_resize_policy`, then `build`; at least one group is required. |

Groups and fields execute in insertion order. `width(n)` caps the owned region
at `n` cells and at the terminal columns remaining after the start column; zero
normalizes to the renderer's one-cell minimum.

`Form::run(&theme)` opens the default terminal. On Unix this is the process's
controlling `/dev/tty`, independent of redirected stdin/stdout; failure to open
it is `RunError::Io { operation: EnterTerminal, .. }`. Other backends may reject
a non-interactive process stream. `run_with_terminal(&mut backend, &theme)` uses
a caller-owned `TerminalBackend` and returns `RunError::NotInteractive` when
that backend reports `is_interactive() == false`.

Both paths are blocking and attempt to restore terminal state before returning.
Restoration itself can fail; `RunError::Io` and `RunError::Resized` retain that
failure in `cleanup` rather than claiming the terminal was restored.

## Placement and resize

| Setting | Meaning |
|---|---|
| `PromptStart::NewLine` | Default. Emit CRLF and begin at column zero. |
| `PromptStart::CurrentLine` | Begin at column zero and overwrite the current line. |
| `PromptStart::CurrentPosition { column }` | Begin at the caller-supplied column; the runtime does not query cursor position. |
| `InlineResizePolicy::ReturnError` | Default. Restore and return `RunError::Resized` without erasing an uncertain region. |
| `InlineResizePolicy::ClearViewportAndRedraw` | Clear every visible primary-buffer cell and redraw; scrollback is retained. |

## Construction and runtime errors

| Error | Cause |
|---|---|
| `FieldConfigError::EmptyName` | A field key name is empty. |
| `FieldConfigError::EmptyOptions` | A select has no options. |
| `GroupBuildError::EmptyGroup` | A group has no fields. |
| `FormBuildError::EmptyForm` | A form has no groups. |
| `FormBuildError::DuplicateFieldName(name)` | Two fields anywhere in the form share a name. |
| `RunError::NotInteractive` | The selected or caller-owned backend reports that it is not interactive. Unix `Form::run` instead reports failure to open `/dev/tty` as `Io { operation: EnterTerminal, .. }`. |
| `RunError::Resized { cleanup }` | A resize occurred under `ReturnError`. |
| `RunError::Io { operation, source, cleanup }` | Entering, reading, rendering, or cleanup failed. |

`IoOperation` identifies `EnterTerminal`, `ReadEvent`, `Render`, or `Cleanup`.
Both runtime error variants retain a cleanup failure separately when one occurs.
