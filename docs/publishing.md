# Publishing the Whitaker installer

This guide explains how to publish the Whitaker installer to
[crates.io](https://crates.io). The installer is published under the crate name
[whitaker-installer](https://crates.io/crates/whitaker-installer)
and depends on the shared support crate
[whitaker-common](https://crates.io/crates/whitaker-common).

## Preconditions

- A [crates.io](https://crates.io) token is available, and `cargo login` has
  been run.
- The working tree is clean, and the release version is agreed.
- The release notes and changelog (if maintained) are up-to-date.

## Version and metadata

1. Bump the version in `installer/Cargo.toml`.
2. Bump the version in `common/Cargo.toml`.
3. Update the workspace dependency versions in `Cargo.toml` so the workspace
   points to the same release for both published crates.
4. Regenerate the lockfile if needed.

## Pre-publish validation

Run the project publish gate to ensure production-like builds and packaging
succeed:

```sh
make publish-check PUBLISH_PACKAGES="whitaker-common whitaker-installer"
```

This target builds the workspace, builds each lint library, and packages the
crates named in `PUBLISH_PACKAGES` for inspection, which here means both
`whitaker-common` and `whitaker-installer`. The target runs under `set -eu`, so
any failed step aborts the gate immediately rather than continuing with a
partially built or stale toolchain.

It runs no tests. The coverage job is the single execution of the suite per
pull request, so re-running it here would bill twice for one result; see "One
execution of the test suite per pull request" in the developers' guide.

Before building the lint libraries, `publish-check` provisions the pinned
Dylint tools by delegating to `scripts/install-dylint-tools.sh`. Nothing is
built from source: the script downloads the upstream `trailofbits/dylint`
prebuilt Linux release archives for `cargo-dylint` and `dylint-link` and
verifies each against a SHA-256 digest pinned in the script, so a gate never
spends minutes recompiling a host tool that upstream already publishes as a
trusted binary. Only a version with a pinned digest can be installed; a request
for any other version is a hard error rather than an unverified download or a
silent fall back to a source build.

The script compares any installed `cargo-dylint` against
`CARGO_DYLINT_VERSION`, probing it as `cargo-dylint dylint --version` because
since 6.x the binary rejects a bare `--version`. `dylint-link` cannot report
its own version at all: it is a linker shim that forwards `--version` to `cc`,
so its presence is instead detected from an executable at
`DYLINT_TOOLS_DIR/bin/dylint-link` or a system copy on `PATH`, replacing the
former `cargo install --list` probe. Tools that are missing or mismatched are
installed into `DYLINT_TOOLS_DIR` (default `~/.cache/whitaker-dylint-tools`), a
durable directory rather than a per-run temporary root, so CI can cache the
tools under one owner and a warm run observes a hit instead of repeating the
download. The Makefile prepends that directory's `bin/` to `PATH` before
invoking the script, so the pinned versions take precedence without touching
any system-wide install. The script still accepts the trailing `CARGO` and
`TOOLCHAIN` arguments, but only so the caller's contract is unchanged now that
no Cargo build occurs; neither participates in the download-and-verify install
path. If either install fails, the script exits non-zero and the gate fails
fast rather than proceeding with stale or unverified tools. This behaviour is
covered by `tests/workflows/test_install_dylint_tools.py`.

The installer declares Rust 1.85 as its minimum supported Rust version. Before
publishing, run the real locked packaged-crate install check as well as the
publish gate:

```sh
make installer-msrv-check
```

The installer keeps `zip` at the 7.2-compatible line because `zip` 8 requires
Rust 1.88. Its dependency configuration enables only the Deflate feature needed
by the installer, so do not broaden that constraint without rechecking the Rust
1.85 build.

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

## Linux compatibility baseline

Published `x86_64-unknown-linux-gnu` installer, dependency-tool, and lint
artefacts support Ubuntu 22.04 and its glibc 2.35 runtime. Their release matrix
entries therefore use the explicit `ubuntu-22.04` runner rather than the moving
`ubuntu-latest` label.

Before upload, release jobs run `scripts/check_glibc_baseline.py` over every
published ELF executable or shared library and reject a required symbol newer
than `GLIBC_2.35`. The tagged workflow then downloads and extracts the packaged
x86_64 installer and dependency archives on Ubuntu 22.04 and executes the
installer, `cargo-dylint`, and `dylint-link`. Do not change the runner baseline
or maximum symbol version independently; update the ADR, checker calls, and
packaged smoke contract together.

## Dry run

Perform a dry run to see the exact artefacts that would be uploaded:

```sh
cargo publish -p whitaker-common --dry-run
cargo publish -p whitaker-installer --dry-run
```

Review the package contents in the output. If files need to be excluded or
included, adjust `common/Cargo.toml` for `whitaker-common` and
`installer/Cargo.toml` for `whitaker-installer` with `include` or `exclude`
settings, then repeat the relevant dry run.

## Publish

When ready, publish from the repository root:

```sh
cargo publish -p whitaker-common
cargo publish -p whitaker-installer
```

## After publishing

- Confirm the new releases appear on crates.io for the
  [whitaker-common](https://crates.io/crates/whitaker-common) and
  [whitaker-installer](https://crates.io/crates/whitaker-installer) crates.
- Tag the release if Git tags are maintained for published versions.
- Announce the release through the agreed channels (team chat, mailing list,
  or social updates).
- Verify documentation links for the installer still resolve (for example, the
  `documentation` URL in `Cargo.toml`).
- Update related documentation that references the published version (for
  example, Whitaker suite integration guidance) if applicable.
- Update any release notes and changelog entries.
