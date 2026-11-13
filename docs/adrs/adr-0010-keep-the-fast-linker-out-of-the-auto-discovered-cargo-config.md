# ADR 010: Keep the fast linker out of the auto-discovered Cargo configuration

## Status

Accepted.

## Context

The estate build standard (concordat `rust-build-defaults`) asks every
repository to make the parallel frontend and the `mold` linker defaults in
`.cargo/config.toml`. Cargo discovers that file from the build directory
upwards. `whitaker-installer` builds Dylint lint libraries from a checkout of
this repository, passing the caller's environment through, so a Linux user
without `mold` would fail to link from source if the file named it.

## Decision

`.cargo/config.toml` carries no `rustflags`. The fast flags live in
`tools/dev-fast/config.toml`, which a developer selects with
`cargo --config tools/dev-fast/config.toml`, and the Makefile's `test`,
`test-doc` and `typecheck` recipes restate them (`DEV_STANDARD_FLAGS`) because
they assign `RUSTFLAGS`. The `.concordat` file records the exception for
BD-001, BD-002, BD-004 and BD-008 with its route and review trigger. The
release workflows build on the pinned nightly from `rust-toolchain.toml`, as an
application does, and CI installs `mold` only in the lanes that run the
Makefile recipes.

## Consequences

- A bare `cargo build` takes no fast flag, and neither does anyone building
  Whitaker from source.
- The flags are stated twice, in the fragment and in the Makefile.
- The exception stays valid while the fragment meets the standard and the
  review trigger holds.
