# Publishing the Whitaker installer

This guide explains how to publish the Whitaker installer to
[crates.io](https://crates.io). The installer is published under the crate name
[whitaker-installer](https://crates.io/crates/whitaker-installer).

## Preconditions

- A [crates.io](https://crates.io) token is available, and `cargo login` has
  been run.
- The working tree is clean, and the release version is agreed.
- The release notes and changelog (if maintained) are up-to-date.

## Version and metadata

1. Bump the version in `installer/Cargo.toml`.
2. Update the workspace dependency version in `Cargo.toml` so the workspace
   points to the same installer version.
3. Regenerate the lockfile if needed.

## Pre-publish validation

Run the project publish gate to ensure production-like builds and packaging
succeed:

```sh
make publish-check PUBLISH_PACKAGES=whitaker-installer
```

This target builds the workspace, runs tests with the pinned toolchain, and
packages the installer crate for inspection.

## Dry run

Perform a dry run to see the exact artefacts that would be uploaded:

```sh
cargo publish -p whitaker-installer --dry-run
```

Review the package contents in the output. If files need to be excluded or
included, adjust `installer/Cargo.toml` with `include` or `exclude` settings
and repeat the dry run.

## Publish

When ready, publish from the repository root:

```sh
cargo publish -p whitaker-installer
```

## After publishing

- Confirm the new release appears on
  [crates.io](https://crates.io/crates/whitaker-installer).
- Tag the release if Git tags are maintained for published versions.
- Announce the release through the agreed channels (team chat, mailing list,
  or social updates).
- Verify documentation links for the installer still resolve (for example, the
  `documentation` URL in `Cargo.toml`).
- Update related documentation that references the published version (for
  example, Whitaker suite integration guidance) if applicable.
- Update any release notes and changelog entries.
