# Coordinated crate publication

Urushi publishes its nine workspace packages at one version. The packages have
independent Cargo identities, but they describe one tested release surface and
use exact versions for every registry dependency within that surface. A release
therefore advances as dependency layers rather than as one unordered workspace
upload.

## Release surface

The 0.1 release contains:

1. `urushi-derive` and `urushi-terminal`, which have no registry dependency on
   another Urushi package;
2. `urushi`, which consumes both foundation packages;
3. `urushi-adapter-ratatui`, `urushi-cli`, `urushi-graphics`,
   `urushi-prompt`, and `urushi-tui`, which consume `urushi` and, where
   applicable, `urushi-terminal`; and
4. `urushi-tui-app`, which consumes the core, terminal, TUI, and optional
   graphics packages.

This order is part of the package contract. A layer is published only after
crates.io can resolve the preceding layer at the exact release version.

## Supported build floor and hosts

Every package declares Rust 1.90 as its minimum supported Rust version. This is
the highest minimum among the dependencies selected by the complete workspace
feature graph, so one value describes both ordinary and all-feature release
checks without giving optional features a hidden higher requirement.

The release checks exercise the complete workspace on Linux, macOS, and
Windows. Unix hosts use the native terminal implementation where selected;
portable entry points use the Crossterm feature. A platform is not claimed from
cross-compilation alone: the CI host for that platform must compile and run the
workspace tests.

## Pre-publication checks

Run these checks from a clean release commit:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
cargo package --workspace --all-features --locked --no-verify
cargo publish --dry-run --package urushi-derive --locked
cargo publish --dry-run --package urushi-terminal --locked
```

The `msrv` CI job runs both the default and all-feature workspace checks with
Rust 1.90. The three-host `test` matrix must also pass before publication; a
local cross-compile does not substitute for running tests on each host.
The publish dry-runs belong to this release-time checklist rather than ordinary
push and pull-request CI.

The workspace package command deliberately uses `--no-verify`. Cargo can build
all archives together, but verification rewrites path dependencies as registry
dependencies and cannot resolve an Urushi package that has not yet been
published. This does not replace build verification: tests, Clippy, rustdoc,
and MSRV checks build the workspace before packaging, while the two foundation
packages receive a complete publish dry-run.

Inspect every generated archive before upload. Each must contain its package
README, the MIT license text, source and declared examples/tests, and any
package-specific notices. It must not contain editor state, dependency caches,
build output, or website dependencies.

## Publication gate

Real publication, tags, and GitHub Releases require a separate explicit human
approval. After that approval, publish one layer at a time. Wait until the new
version resolves from crates.io, run `cargo publish --dry-run` for the next
layer, and only then upload that layer. After the last layer, build a clean
consumer using registry dependencies only before creating the final release
record.
