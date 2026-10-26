# Architectural decision record (ADR) 007: OrthoConfig CLI boundary

## Status

Accepted for roadmap item 3.6.3.

## Context and problem statement

The planned Whitaker CLI needs a single configuration model across TOML,
environment variables and explicit command-line values. Existing Dylint users
have `dylint.toml` and `DYLINT_LOCALE`; they need a one-release migration path.
The shipped installer has its own CLI contract and remains unchanged. Dylint
also requires a dated nightly toolchain, while OrthoConfig v0.10.0 requires at
least Rust 1.89.0.

## Decision

Place OrthoConfig v0.10.0 with default features enabled in a configuration-only
`whitaker-cli` application package. Its parser and configuration DTOs are
separate from validated settings passed to the domain. Reusable libraries do
not depend on OrthoConfig. The package has a Rust 1.89.0 toolchain boundary;
the lint workspace retains its Dylint nightly. This item does not introduce a
CLI executable or command dispatch.

The configurable fields are lint selection, ignore and experimental settings;
installation policy; diagnostic locale; output colour and progress; and the
per-lint settings in the CLI design. Help, version, configuration selection,
verbosity, JSON, offline mode, forwarded Cargo arguments and one-run controls
remain CLI-only. Per-command sources use `[cmds.<name>]` and
`WHITAKER_CMDS_<NAME>_*`. Named profiles and Cargo external-subcommand entry
points are not part of this CLI.

Use TOML and discover `whitaker.toml` or `.whitaker.toml` in the configured
system, user and project scopes. `--config` and `WHITAKER_CONFIG` select an
explicit file. Merge defaults, files, legacy environment aliases, `WHITAKER_*`
environment values and explicit CLI values in that order. Where clap supplies a
default for a configurable field, use `cli_default_as_absent` so the default
does not mask file or environment values.

For one release, fall back to `dylint.toml` only when no canonical file is
selected or found. Map `DYLINT_LOCALE` below `WHITAKER_DIAGNOSTICS_LOCALE`.
Return deprecation notices that identify the replacement. A selected missing,
unreadable, or malformed file is an error, not optional absence. Help and
version are successful display requests; parse and configuration failures
remain typed at the application boundary. Localize help and parse errors
together with an `en-GB` fallback.

## Consequences

Injected sources and explicit file roots permit precedence and compatibility
tests without mutating the process environment. The future CLI presents notices
and diagnostics, while libraries neither print nor exit. The existing installer
and CI provisioning retain their current contracts. Later roadmap items
implement selector precedence, shared presentation flags and execution.
