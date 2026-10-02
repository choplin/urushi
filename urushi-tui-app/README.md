# urushi-tui-app

A TEA-style application framework for full-screen Urushi interfaces.

```toml
[dependencies]
urushi = "0.1.0"
urushi-tui-app = "0.1.0"
```

Applications describe model initialization, pure updates, views, effects, and subscriptions. The runtime owns ordered delivery, scheduling, terminal input, frame presentation, and session restoration. Enable `graphics` to integrate `urushi-graphics` with the same terminal owner.

Applications that return image-bearing views also depend on `urushi-graphics`
directly and enable the runtime integration:

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-tui-app = { version = "0.1.0", features = ["graphics"] }
```

On Unix this configuration makes the default runtime use Urushi's native
bidirectional terminal connection so it can positively query Kitty and Sixel
support. On other platforms automatic selection keeps the text fallback unless
the application supplies a capability-aware backend.

The 0.1 API is in early development. See [Choose a terminal surface](https://github.com/choplin/urushi#choose-a-terminal-surface) and [docs.rs](https://docs.rs/urushi-tui-app).

Licensed under the MIT License.
