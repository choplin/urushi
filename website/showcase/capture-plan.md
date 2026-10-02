# Documentation overview capture plan

## Brief

- Audience: Rust developers evaluating Urushi's interactive surfaces.
- Destination and format: Prompt and TUI overview pages; short silent MP4 plus
  a PNG poster for each surface.
- Core user benefit: See one real interaction complete before reading the
  concepts and implementation steps behind it.
- Difference from static output: The capture proves focus, state transition,
  effects, submission, and terminal restoration that a single text fence
  cannot show.
- Assumptions: The website-only fixture uses the same public form and runtime
  APIs as the checked-in examples, with terminal capabilities fixed by the
  known VHS environment.
- Target duration: 4–7 seconds per video, long enough to read every
  proof-bearing state without turning the overview into a tutorial video.

## Video script

| Beat | Duration | Benefit / claim | On-screen action | Visible evidence | Surface |
|---|---:|---|---|---|---|
| Prompt 1 | 2 s | A form owns a sequence of typed fields | Start `wizard`, type `Aki`, submit | Colored question, cursor, entered value | terminal |
| Prompt 2 | 3 s | Selection and confirmation are part of the same form | Move to Formal, submit, accept Yes | Focus and selected-option styling move between fields | terminal |
| Prompt 3 | 2 s | Submission returns typed values and restores the terminal | Submit confirmation | `Good day, Aki.` appears in ordinary output | terminal |
| TUI 1 | 2 s | A runtime draws a full-screen View from its Model | Start `runtime_counter` | Initial counter and surface size | terminal |
| TUI 2 | 4 s | Input updates the Model and Effects return later as Messages | Press two keys, wait | `key updates` changes before `effect completions` catches up | terminal |
| TUI 3 | 2 s | Shutdown restores the session and returns the final Model | Press `q` | Shell output shows `final count: 2` | terminal |

## Still-image shot list

| Shot | Benefit / claim | Scene and visible proof | Dimensions / format | Surface |
|---|---|---|---|---|
| Prompt poster | One form coordinates several field types | Formal selected and confirmation visible | 1100 × 620 PNG | terminal |
| TUI poster | Model and Effect progress are distinct | Two key updates and two completions | 840 × 300 PNG | terminal |

## Capture readiness

- Representative fixture: `website/showcase/terminal-fixtures`, which runs the
  real Prompt and TUI runtimes through a caller-owned Crossterm backend.
- Real execution path: `cargo run --quiet --manifest-path
  terminal-fixtures/Cargo.toml --bin <fixture>`.
- Input gesture fidelity: key input is evidence and must reach the real program.
- Required lifecycle end state: prompt submission and TUI shutdown must restore
  the terminal before the final ordinary output appears.
- Window: 1100 × 620 at 18 px for Prompt and 840 × 300 at 20 px for TUI,
  Menlo with 24 px padding.
- Visible environment: bash, minimal prompt, Catppuccin Mocha, UTF-8,
  `TERM=xterm-256color`, `COLORTERM=truecolor`, and `NO_COLOR` removed.
- Completion signals: `Good day, Aki.` and `final count: 2`.
- Reset point: prebuilt workspace, fresh example process, empty VHS terminal.
- Privacy: no hostname, absolute path, token, notification, or personal fixture.
- Proof-bearing states: prompt field focus and final output; TUI initial frame,
  pending/completed effect frame, and restored shell output.
- Outputs: `website/public/docs/media/prompt-wizard.{mp4,png}` and
  `website/public/docs/media/tui-runtime-counter.{mp4,png}`.

## Reproduction bundle

- Capture source: `website/showcase/prompt-wizard.tape` and
  `website/showcase/tui-runtime-counter.tape`.
- Fixture: the checked-in website capture package; no generated data.
- Fixed display settings: declared in each tape.
- Final outputs: the MP4 and PNG files under `website/public/docs/media/`.
