# Terminal Background and Theme Selection

How an application may adapt an Urushi theme to the terminal background without
making theme values, prompts, or the TUI runtime own hidden terminal detection.

## Decision

Terminal background observation and theme selection are separate operations:

1. a caller-owned bidirectional terminal connection observes an optional RGB
   background;
2. core classifies that RGB value as light or dark and resolves the caller's
   explicit theme policy; and
3. the caller constructs its application or form with the selected `Theme` and
   passes the same terminal connection to the interactive surface.

Neither `Theme` nor `ThemeSet` performs terminal I/O. Standard-stream output,
which does not own the response side of a terminal connection, never queries a
background implicitly.

## Terminal observation

`urushi-terminal` owns the observed value and query:

```rust
pub struct TerminalBackground {
    red: u8,
    green: u8,
    blue: u8,
}

impl TerminalBackground {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self;
    pub const fn red(self) -> u8;
    pub const fn green(self) -> u8;
    pub const fn blue(self) -> u8;
}

pub trait TerminalQuery {
    fn terminal_background(&mut self)
        -> std::io::Result<Option<TerminalBackground>> {
        Ok(None)
    }
}
```

The query is a synchronous, one-shot observation of the connection on which it
is invoked. The native backend sends the OSC 11 background-color query and
parses an OSC 11 `rgb:` reply. Protocol components from one through four
hexadecimal digits are scaled to the nearest eight-bit value. Ordinary input
consumed while waiting for the reply remains pending for the event reader, as
it does for other terminal queries.

An unsupported query, no reply, a malformed reply, or expiry of the query
timeout produces `Ok(None)`. Failure to read from or write to the owned
terminal connection, or failure to establish or restore a process mode needed
for the query, remains an I/O error. An implementation that cannot perform the
query returns `Ok(None)`.

The result is not added to `TerminalInfo`: passive inspection of an output
handle has no response channel. It is also not cached. Calling
`terminal_background` again is an explicit request for a new observation.

## Classification and policy

Core `urushi` owns the policy because it already owns `ColorScheme` and
`ThemeSet`:

```rust
pub enum ThemeMode {
    Light,
    Dark,
    Auto { fallback: ColorScheme },
}

impl ThemeMode {
    pub fn resolve(
        self,
        background: Option<TerminalBackground>,
    ) -> ColorScheme;
}
```

`Light` and `Dark` are explicit overrides. Callers using either variant skip
terminal observation. `Auto` classifies a present background and uses its
explicit `fallback` when observation returns `None`; there is no implicit
fallback and `ThemeMode` has no default.

Classification uses relative luminance over sRGB. For each eight-bit channel,
first normalize `c = channel / 255`. Linearize it as:

```text
c_linear = c / 12.92                         when c <= 0.04045
           ((c + 0.055) / 1.055) ^ 2.4      otherwise
```

Then compute:

```text
L = 0.2126 * red_linear + 0.7152 * green_linear + 0.0722 * blue_linear
```

`L < 0.5` resolves to `ColorScheme::Dark`; `L >= 0.5` resolves to
`ColorScheme::Light`. `ThemeSet::select` then selects the corresponding stable
theme value. The same input always produces the same classification; terminal
families and environment variables do not participate.

## Caller-owned composition

The ordinary adaptive flow is performed once before constructing the object
whose `view` uses the theme:

```rust
let mut terminal = NativeTerminal::open()?;
let background = match &mode {
    ThemeMode::Auto { .. } => terminal.terminal_background()?,
    ThemeMode::Light | ThemeMode::Dark => None,
};
let theme = themes.select(mode.resolve(background));
```

The caller then stores `theme` in its application or supplies it to its form.
It also passes `terminal`, the same physical connection that answered the
query, into the selected interactive surface. This prevents observation of one
terminal from choosing presentation for another.

The prompt exposes the caller-owned path directly:

```rust
pub fn run_with_terminal(
    self,
    terminal: &mut impl TerminalBackend,
    theme: &Theme,
) -> Result<FormOutcome, RunError>;
```

`Form::run(&Theme)` remains the convenience path that opens the default
terminal and uses the already selected theme. It does not infer a theme.

For a TUI, the selected theme belongs to the `Application` value and may be
used by `Application::view`. The same connection is then installed on the
runtime through its terminal entry point. Background does not appear in
`Surface`, is not delivered through `Subscription`, and does not cause a
`Sync` delivery. It cannot change during that application run unless the
application itself owns an explicit re-query and replacement policy.

Static CLI helpers likewise do not query. A program that wants adaptive static
output must own a bidirectional connection to the actual destination, query it,
and choose the theme before rendering.

## Update behavior

Urushi performs no polling, focus- or resize-triggered re-query, background
change notification, or automatic theme swap. One call performs one
observation. An application may call again when it owns the connection and the
ordering consequences, but the library does not present the value as live
surface state.

## Rejected alternatives

- Putting the result in TUI `Surface` would repeatedly deliver a value that is
  stable for the ordinary run and would incorrectly make initial theme
  construction depend on application messages.
- Adding a background subscription would model a one-shot query as a live
  source and imply update behavior the terminal protocol does not provide.
- Having the TUI or prompt invoke an application factory after opening its own
  terminal would invert ownership and make application construction a runtime
  lifecycle callback.
- Returning only light/dark from `urushi-terminal` would put a presentation
  threshold in the terminal foundation and prevent callers from using the
  observed color for another policy.

## Noctui parity

Noctui must use the equivalent `TerminalBackground`, `ThemeMode`, classification
formula, explicit fallback, and caller-owned one-shot flow. Its current
`src/theme/theme.mbt` and `src/theme/theme_test.mbt` define explicit
`ColorScheme` selection and correctly keep terminal detection out of Theme,
but they do not yet define terminal background observation or automatic policy.
That is an implementation gap, not a reason for the two libraries to have
different models or results.
