# Releasing Urushi

Urushi publishes all nine workspace crates at one version. Internal registry
dependencies use that exact version, so releases are coordinated with
`cargo-release` rather than publishing crates individually. GitHub Actions is
the only production release executor; local use stops at the preview.

The release configuration is defined by [`release.toml`](../release.toml).

## Before releasing

- Use the reviewed tip of `main` with a clean working tree that is up to date
  with `origin/main`.
- Confirm that CI passed for that commit, including the host matrix, MSRV,
  formatting, Clippy, rustdoc, and package checks.
- Prepare a reviewed change on `main` that changes `Unreleased` in
  [`CHANGELOG.md`](../CHANGELOG.md) into the dated release section, adds a new
  empty `Unreleased` section and comparison link, and updates every README
  version example to the target version.
- Leave workspace package and internal dependency versions unchanged in that
  change; `cargo-release` owns those changes.
- Confirm that the release preparation is pushed to `origin/main`.

Enter the development shell so the flake-locked Rust and `cargo-release`
versions are used:

```sh
nix develop
```

Use the `cargo-release` binary directly. Do not use `cargo release`, because
Cargo plugin dispatch can select a different executable from `CARGO_HOME`.

## Preview locally

Preview the release without changing Git or publishing anything:

```sh
cargo-release release <version> --workspace
```

Confirm that the preview:

- moves all nine packages and exact internal dependencies to the requested
  version;
- selects all publishable crates in dependency order; and
- plans one release commit, one `v<version>` tag, and a push to `origin`.

Do not pass `--execute` locally. The production credentials are short-lived
and available only to the release workflow.

## Execute through GitHub Actions

Each crate must trust the `choplin/urushi` repository and
`.github/workflows/release.yml` in its crates.io Trusted Publishing settings.
No GitHub environment is part of that identity.

After explicit human approval of the preview, dispatch **Release crates** from
the `main` branch and enter the exact version. The workflow repeats the preview,
exchanges its GitHub OIDC identity for a short-lived crates.io token, and then
runs:

```sh
cargo-release release <version> --workspace --execute --no-confirm
```

The execute path updates package versions, internal dependencies, and
`Cargo.lock`. Cargo verifies each package before publishing it. On success,
`cargo-release` creates one release commit, publishes crates in dependency
order, creates the tag, and pushes the commit and tag.

## Resume an interrupted release

Published crate versions cannot be replaced. Keep the generated release commit
unchanged. After identifying and correcting the failure, dispatch **Release
crates** again from `main` with the same version.

The repeated workflow preview shows the remaining publications. During
execution, `cargo-release` skips versions that already exist and continues in
dependency order. Once all packages resolve from crates.io, it creates the
missing tag and pushes the release commit and tag.

For diagnosis only, the individual local previews are:

```sh
cargo-release release publish --workspace
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
