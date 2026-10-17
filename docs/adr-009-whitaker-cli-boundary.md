# Architectural decision record (ADR) 009: root Whitaker CLI boundary

## Status

Accepted (2026-10-17).

## Date

2026-10-17.

## Context and problem statement

Whitaker's installer currently creates a `whitaker` wrapper that invokes
`cargo dylint`, alongside a `whitaker-ls` wrapper. A Rust binary named
`whitaker` is needed to provide one command surface, but installing it beside
the existing wrapper creates a `PATH` collision. The root package also held
compiler-private Dylint driver code, preventing it from being packaged and
tested as an ordinary application.

The command must retain existing `whitaker --all` workflows while the later
`whitaker check` command is still under development. It must reuse the
installer's established implementation without making command policy depend on
filesystem scanning, subprocess details, or installer internals.

## Decision Drivers

- Make the root `whitaker` package publishable and testable.
- Preserve existing installer behaviour and `cargo dylint` invocations.
- Prevent root installation from generating a script over the new executable.
- Keep the compiler-private driver out of the published dependency graph.
- Give future lazy repair in `whitaker check` a clear ownership boundary.

## Requirements

### Functional requirements

- Provide `whitaker install`, `whitaker ls`, root help/version, and legacy
  Dylint forwarding in roadmap item 3.5.1.
- Preserve the `whitaker-installer` executable and its current wrapper policy.
- Return the forwarded `cargo dylint` exit status to the caller.
- Keep `whitaker ls` output explicitly transitional until bundle manifests are
  introduced.

### Technical requirements

- Keep command parsing, routing, and exit policy in the root CLI library.
- Access installation, inventory, and subprocess behaviour through ports and
  system adapters.
- Keep installation orchestration in the `whitaker-installer` library.
- Ensure a root `whitaker install` does not generate a `whitaker` wrapper.

## Options considered

### Option A: retain the wrapper-only `whitaker` command

Keep `whitaker` as a generated shell script and make no root binary. This
avoids a new executable but leaves the package boundary and requested command
surface unresolved.

### Option B: give the new binary a different name

Publish a separate name such as `whitaker-cli`. This avoids the `PATH`
collision but does not provide the requested `whitaker` command and adds a
second public product name.

### Option C: add the root binary and make wrapper policy caller-specific

Move compiler-private code into an internal crate, expose installer
orchestration from the installer library, and add the root binary behind
application ports. Keep the legacy installer unchanged; its root-CLI entry
point selects a policy that creates only `whitaker-ls`. Route existing Dylint
arguments to `cargo dylint` until `whitaker check` replaces that workflow.

| Topic                                  | Option A       | Option B | Option C                |
| -------------------------------------- | -------------- | -------- | ----------------------- |
| Requested `whitaker` binary            | No             | No       | Yes                     |
| Existing installer compatibility       | Yes            | Yes      | Yes                     |
| Avoids overwriting the root executable | Not applicable | Yes      | Yes                     |
| Preserves `whitaker --all`             | Yes            | Unclear  | Yes, through forwarding |
| Supports the approved package boundary | No             | Partly   | Yes                     |

_Table 1: Comparison of executable-boundary options._

## Decision outcome / proposed direction

Adopt Option C. The root package owns `whitaker`; its `src/cli/` modules own
parsing, routing, output policy, and process exit codes. `src/ports.rs`
declares installation, inventory, and Dylint-runner ports, and `src/adapters/`
supplies the production implementations. The CLI chooses whether and when an
operation runs. The installer library owns how installation and staging are
performed.

The installer orchestration boundary is `whitaker_installer::orchestration`.
The root CLI calls `run_install_for_whitaker`, which uses the same installation
flow but does not write a `whitaker` wrapper. Inventory discovery is shared
through `whitaker_installer::list::collect_installed`; the root CLI and legacy
installer retain their own output and toolchain-display decisions. The
installer's existing public helpers remain source-compatible.

For any invocation whose first non-global token is not `install`, `ls`, root
help, or root version, the root binary forwards all arguments after its own
executable name, in order, to `cargo dylint`. This includes `--` and the
arguments after it. The child exit status is preserved. This requirement is
`CLI-REQ-FWD`; it is derived from current user and Makefile workflows rather
than an upstream specification. It is deferred for removal until roadmap item
3.9.3, after `whitaker check` supersedes it.

The root package depends on the installer library and root-only CLI
dependencies. The compiler-private driver remains in the non-publishable
`whitaker_lint_core` crate, so `ortho_config` and CLI dependencies do not
propagate into the cross-compiled lint crates. This boundary also makes the
future `whitaker check` lazy-repair decision belong to the CLI, while the
installer remains responsible for executing a requested repair.

The installer records one canonical staging directory per toolchain in
`<whitaker_data>/installed_lint_paths.json` after a successful installation.
The root CLI supplies all extant recorded paths to Dylint when forwarding a
legacy invocation, preserving custom `--target-dir` installs and allowing
Dylint to choose the matching toolchain. A non-empty user-provided
`DYLINT_LIBRARY_PATH` remains authoritative. The installer owns this record's
format and updates it at the common completion point for source builds,
prebuilt downloads, and staged-suite installs.

The root package carries cargo-binstall metadata. Publishing the crate reserves
the name `whitaker`, but GitHub release archives for that executable are not
published until roadmap item 3.5.3. Until those archives exist,
`cargo binstall` falls back to a source build. This decision does not claim
that the GitHub release workflow already packages the root executable.

## Goals and non-goals

- Goals:
  - Provide one root executable while preserving the legacy installer.
  - Keep command policy separate from installer operations.
  - Preserve current Dylint invocations during the migration.
- Non-goals:
  - Implement `whitaker check` or `whitaker doctor` in item 3.5.1.
  - Make the current `ls` rendering a stable output contract.
  - Publish GitHub binary release assets before item 3.5.3.

## Migration plan

1. Move compiler-private driver code to `whitaker_lint_core` and make the root
   package an ordinary publishable package.
2. Move installer orchestration behind `whitaker-installer`'s library API and
   preserve its existing CLI behaviour.
3. Add the root `whitaker` binary with `install`, `ls`, and legacy Dylint
   forwarding. Use a root-specific wrapper policy to avoid replacing the
   executable.
4. Add cargo-binstall metadata and reserve the crates.io name by publishing
   the package after the local release gates pass.
5. Document the transitional command surface. Add lazy repair and the later
   stable bundle and diagnostics contracts in their roadmap phases.

## Known risks and limitations

- An older generated `whitaker` script can still take precedence if its
  directory appears earlier on `PATH` than Cargo's binary directory. The root
  installer does not create or overwrite that script. Removing the root binary
  with `cargo uninstall whitaker` leaves the legacy installer script intact.
- The `ls` output reflects the installer's current scanner and is not the
  future bundle-manifest or effective-configuration view.
- There are no `whitaker` GitHub release archives until item 3.5.3, so
  cargo-binstall uses its source-build fallback in the interim.
- The unified parser is wired through `ortho_config`'s localized parsing
  interface with `NoOpLocalizer`; translations are not part of this milestone.

## Outstanding decisions

- No additional architecture decision is required for item 3.5.1. The public
  `check`, bundle-manifest, and `doctor` contracts remain with their later
  roadmap phases.
