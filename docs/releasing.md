# Releasing Urushi

Urushi publishes all nine workspace crates at one version. Internal registry
dependencies use that exact version, so releases are coordinated with
`cargo-release` rather than publishing crates individually.

The release configuration is defined by [`release.toml`](../release.toml).

## Before releasing

- Use the reviewed tip of `main` with a clean working tree that is up to date
  with `origin/main`.
- Confirm that CI passed for that commit, including the host matrix, MSRV,
  formatting, Clippy, rustdoc, and package checks.
- Prepare and merge a release PR that changes `Unreleased` in
  [`CHANGELOG.md`](../CHANGELOG.md) into the dated release section, adds a new
  empty `Unreleased` section and comparison link, and updates every README
  version example to the target version.
- Leave workspace package and internal dependency versions unchanged in that
  PR; `cargo-release` owns those changes.
- Make crates.io credentials available through Cargo's credential provider or
  the release environment.

Enter the development shell so the flake-locked Rust and `cargo-release`
versions are used:

```sh
nix develop
```

Use the `cargo-release` binary directly. Do not use `cargo release`, because
Cargo plugin dispatch can select a different executable from `CARGO_HOME`.

## Preview and execute

Preview the release without changing Git or publishing anything:

```sh
cargo-release release <version> --workspace
```

Confirm that the preview:

- moves all nine packages and exact internal dependencies to the requested
  version;
- selects all publishable crates in dependency order; and
- plans one release commit, one `v<version>` tag, and a push to `origin`.

After explicit human approval, execute the same release:

```sh
cargo-release release <version> --workspace --execute
```

The execute path updates package versions, internal dependencies, and
`Cargo.lock`. Cargo verifies each package before publishing it. On success,
`cargo-release` creates one release commit, publishes crates in dependency
order, creates the tag, and pushes the commit and tag.

## Resume an interrupted release

Published crate versions cannot be replaced. Keep the generated release commit
unchanged and preview the remaining publications:

```sh
cargo-release release publish --workspace
```

After checking the remaining set, continue it:

```sh
cargo-release release publish --workspace --execute
```

Already-published versions are skipped. Once all packages resolve from
crates.io, create a missing tag or push through the corresponding preview and
execute steps:

```sh
cargo-release release tag --workspace
cargo-release release tag --workspace --execute
cargo-release release push --workspace
cargo-release release push --workspace --execute
```

Do not edit the release contents, reuse the version for a different commit, or
try to upload an existing crate version again.

## Finish the release

Build a clean consumer using registry dependencies only, covering the static
output, Prompt, Ratatui adapter, and TUI application paths. After that check,
create the GitHub Release for `v<version>` and record the commit, CI result,
published packages, consumer verification, tag, and GitHub Release URLs.

Urushi does not use `dist`; it publishes libraries rather than application
binaries or installers.
