# Publishing Whitaker crates

This guide explains how to publish Whitaker packages to
[crates.io](https://crates.io). The dependency order is `whitaker-common`,
`whitaker-installer`, then the root `whitaker` CLI. The root CLI uses the
installer library; the installer in turn uses the shared support crate.

## Preconditions

- A [crates.io](https://crates.io) token is available, and `cargo login` has
  been run.
- The working tree is clean, and the release version is agreed.
- The release notes and changelog (if maintained) are up-to-date.

## Version and metadata

1. Bump the versions in `common/Cargo.toml`, `installer/Cargo.toml`, and the
   root `Cargo.toml` together for a coordinated release.
2. Update the workspace dependency versions in `Cargo.toml` to match the
   published dependencies.
3. Regenerate the lockfile if needed.

## Pre-publish validation

Run the project publish gate to ensure production-like builds and packaging
succeed:

```sh
make publish-check PUBLISH_PACKAGES="whitaker-common whitaker-installer whitaker"
```

This target builds the workspace, runs tests with the pinned toolchain, and
packages the crates named in `PUBLISH_PACKAGES` for inspection, which here means
`whitaker-common`, `whitaker-installer`, and `whitaker`. The target runs under
`set -eu`, so any failed step aborts the gate immediately rather than
continuing with a partially built or stale toolchain.

Before building the lint libraries, `publish-check` provisions the pinned
Dylint tools by delegating to `scripts/install-dylint-tools.sh`. Host-tool
installs run under the toolchain named by `DYLINT_TOOLS_TOOLCHAIN` (default
`stable`), because the dylint 6.0.1 lockfile requires a newer rustc than the
repository's pinned nightly provides. The script compares any installed
`cargo-dylint` against `CARGO_DYLINT_VERSION`, and checks `dylint-link` via
`cargo install --list` (`dylint-link` is a linker shim whose `--version` is
forwarded to `cc`, so it cannot be probed directly). Tools that are missing or
mismatched are installed into an isolated, per-run temporary root; the Makefile
prepends that root's `bin/` directory to `PATH` only when it exists, so the
pinned versions take precedence without touching any system-wide install. If
either install fails, the script exits non-zero and the gate fails fast rather
than proceeding with stale or absent tools. This behaviour is covered by
`tests/workflows/test_install_dylint_tools.py`.

To validate the installer archive path used by the release workflow on the
current host platform, run:

```sh
make release-installer-dry-run
```

This target builds the installer, invokes `whitaker-package-installer`, and
generates checksums for the resulting archive, so release packaging issues are
caught before tagging or publishing. It is a POSIX-shell target and checks for
the required `awk`, `jq`, `mktemp`, `python`, and `rustc` commands before doing
build work. On Windows, run it from an environment that provides those tools,
such as the Bash shell used by CI.

## Dry run

Perform a dry run to see the exact artefacts that would be uploaded:

```sh
cargo publish -p whitaker-common --dry-run
cargo publish -p whitaker-installer --dry-run
cargo publish -p whitaker --dry-run
```

Review the package contents in the output. If files need to be excluded or
included, adjust `common/Cargo.toml` for `whitaker-common`, or the appropriate
package manifest for the other crates, then repeat the relevant dry run.

Publishing the root `whitaker` package reserves that crates.io name. Perform
that first publication only after the release gates and package contents have
been reviewed. It does not publish GitHub release archives for the executable;
those are a separate delivery in roadmap item 3.5.3. Until then,
`cargo binstall whitaker` falls back to building the crate from source.

## Publish

When ready, publish from the repository root:

```sh
cargo publish -p whitaker-common
cargo publish -p whitaker-installer
cargo publish -p whitaker
```

## After publishing

- Confirm the new releases appear on crates.io for the
  [whitaker-common](https://crates.io/crates/whitaker-common), [whitaker-installer](https://crates.io/crates/whitaker-installer),
  and [whitaker](https://crates.io/crates/whitaker) crates.
- Tag the release if Git tags are maintained for published versions.
- Announce the release through the agreed channels (team chat, mailing list,
  or social updates).
- Verify documentation links for the installer still resolve (for example, the
  `documentation` URL in `Cargo.toml`).
- Update related documentation that references the published version (for
  example, Whitaker suite integration guidance) if applicable.
- Update any release notes and changelog entries.
