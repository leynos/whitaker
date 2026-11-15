# Architectural decision record (ADR) 0014: per-lint opt-in for experimental lints

## Status

Accepted (2026-11-15): Compile every experimental lint into the published
suite, register an experimental lint's pass only when the consuming repository's
`dylint.toml` names it, validate that list in `whitaker-common` so the
installer and the forthcoming `whitaker` command share one parser, and keep
`whitaker-installer --experimental` as a local-build shortcut that enables all
of them.

## Date

2026-11-15.

## Context and problem statement

Until `whitaker-installer` 0.2.12, an experimental lint was a compile-time
choice. Three suite features (`experimental-macro-aware-cognitive-complexity`,
`experimental-rstest-helper-should-be-fixture` and
`experimental-string-continuation-style`) pulled each lint into
`whitaker_suite`, and `whitaker-installer --experimental` switched all of them
on together and forced a source build. Three consequences followed:

- A repository that wanted one experimental lint in CI had to build every
  experimental lint from source, because the prebuilt suite is published
  without them. `install-whitaker` in shared-actions has no input for either.
- `--lint X --experimental` built that one lint alone, not the suite plus it.
- The choice was all or nothing. Rolling an experimental lint out to one
  repository at a time was impossible, and a crash in any one of them was a
  crash for every consumer of the experimental build.

Many experimental lints are in the pipeline, so the project needs an
incremental rollout path. The forthcoming `whitaker` command
([ADR 0012](adr-0012-whitaker-cli-boundary.md) and the
[CLI design](../whitaker-cli-design.md)) must be able to take that path over
without redoing it.

## Decision drivers

- Continuous integration (CI) must never source-build for an experiment.
- A lint that a repository has not chosen must not run, and must not be able to
  crash the compiler for that repository.
- A misspelt lint name must be a clear error rather than a silent no-op, the
  failure mode recorded in issue #332 for `no_std_fs_operations`.
- The decision must live in data a repository owns, not in an installer flag.
- Parsing and validation must be reusable by the installer, the suite, and the
  future command, none of which should depend on the others' internals.
- Repositories build with `-D warnings`, so an opted-out repository must not
  gain a new warning merely because a lint exists.

## Options considered

### Option A: per-lint installer flags

Extend `--experimental` to `--experimental=name,…` and add an
`install-whitaker` input. This keeps the choice outside the repository, forces
a different suite binary per choice, and cannot be taken over by the command
without reproducing the flag matrix. Rejected.

### Option B: an environment variable read by the suite

Cheap, but invisible in review, absent from the repository, and awkward in
editors and CI matrices. Rejected as the primary mechanism. The suite already
honours `DYLINT_TOML`, so the command can still supply the same table through
that route (see the CLI inheritance below).

### Option C: ship everything, opt in per lint in `dylint.toml` (chosen)

The suite always contains every experimental lint. The repository's
`dylint.toml` lists the ones it wants.

## Decision outcome

### Configuration

```toml
[whitaker]
experimental = ["macro_aware_cognitive_complexity"]

[macro_aware_cognitive_complexity]
max_complexity = 9
```

`dylint.toml` is the driver contract: the suite runs inside `rustc` and can
read only what Dylint hands it. The per-lint threshold keeps its existing key
(`max_complexity` for the macro-aware lint).

### Enforcement

`register_suite_entry` in the suite reads the `[whitaker]` table with
`dylint_linting::config` and passes the names to
`whitaker_common::experimental::resolve`. The result is an
`ExperimentalSelection`, and `register_suite_lints` takes it:

- Every suite lint, experimental or not, is *declared* (`register_lints`).
- Only an experimental lint named in the selection gets a pass
  (`register_late_pass` or `register_early_pass`). An unlisted lint is declared
  but passless, so it cannot run and cannot crash the compiler.
- The always-on late lints stay in one combined pass. Experimental late lints
  are registered as separate passes, which removes the per-combination
  `define_suite_pass!` call sites that each new experimental lint would
  otherwise have doubled.

### Why declare an unlisted lint

Registering nothing would make a leftover `#[allow(lint)]` or `#[expect(lint)]`
trip rustc's `unknown_lints`, which `-D warnings` turns into an error for a
repository that opted out. A declared, passless lint keeps `#[allow(…)]` silent
and keeps the list `cargo dylint list` prints coherent.

One residual difference is observed and accepted. On a passless lint,
`#[expect(lint)]` is reported as `unfulfilled_lint_expectations`, exactly as
for any lint that never fires; under `-D warnings` that is an error. A
repository that does not opt in should therefore use `#[allow]`, or remove the
`#[expect]`. The nested `cargo dylint` smoke test in the suite pins that
`#[allow]` stays silent on an unlisted lint.

### Errors

- A name that is neither experimental nor promoted ends compilation with a
  fatal diagnostic naming every unknown entry and the valid choices. Nothing is
  silently ignored.
- An unknown key in the `[whitaker]` table, a wrongly typed list, or an
  unreadable table is an error too, for the same reason.
- A *promoted* lint (one that left the experimental stage) is still accepted,
  with a note that the entry is redundant, so promotion never breaks a
  consumer's configuration.

### Shared validation

`whitaker_common::experimental` holds the stage vocabulary
(`EXPERIMENTAL_LINTS`, `PROMOTED_LINTS`) and the pure `resolve` function. It
has no `rustc`, serde, file, or environment dependency. The installer pins its
`EXPERIMENTAL_LINT_CRATES` to the same list with a test, and the suite derives
`LintDescriptor::is_experimental` from it.

### The installer

- Every published and locally built suite contains the experimental lints. The
  suite's per-lint `experimental-*` features remain for one release as empty
  aliases of `dylint-driver`, and the installer no longer passes them.
- `--experimental` builds the suite from source with the
  `experimental-enable-all` feature, which starts every experimental lint
  enabled. A repository's `dylint.toml` still adds to that. It remains a source
  build because that feature is not part of the published suite. It needs a
  suite from this release onward, so `--experimental` together with an older
  `--suite-version` fails at the feature lookup.
- `--lint X` for an experimental crate no longer requires `--experimental`:
  naming the crate is the opt-in for that one lint. The
  `InstallerError::ExperimentalLintRequiresFlag` variant and its message are
  removed, which supersedes issue #234 (its rejection metric has nothing left
  to count). A named experimental crate still builds from source, because no
  standalone prebuilt exists for it.

### Inheritance by the `whitaker` command

The command design makes `whitaker.toml` the canonical configuration and
`dylint.toml` a one-release bridge. The suite cannot read `whitaker.toml`, so
the table above remains the driver contract and the command is the user-facing
layer over it:

1. `whitaker.toml` gains `[lint] experimental_lints = ["…"]` beside the
   existing `[lint] experimental` boolean, which keeps its meaning of "all
   experimental lints".
2. The command validates the list with `whitaker_common::experimental::resolve`
   (the same function the suite calls), so errors and promotion notes read the
   same everywhere. `whitaker-common` takes no `ortho_config` dependency,
   consistent with [ADR 0008](adr-0008-ortho-config-cli-boundary.md).
3. To run the driver the command hands over the resolved list as the
   `[whitaker] experimental` table, through a generated `DYLINT_TOML` (or by
   leaving a repository's own `dylint.toml` alone when it already carries the
   table). The boolean maps to the `experimental-enable-all` behaviour.
4. A repository using only `dylint.toml` keeps working through the existing
   bridge with no change.

## Lint lifecycle

A lint moves through three stages, documented for contributors in the
developers' guide:

1. **Experimental.** Compiled into the suite, listed in
   `EXPERIMENTAL_LINTS`, runs only for repositories that list it.
2. **Opt-in rollout.** Repositories list the lint one at a time, thresholds are
   tuned, and findings are triaged.
3. **Promoted.** The name moves from `EXPERIMENTAL_LINTS` to `PROMOTED_LINTS`,
   joins the always-on set, and runs by default. Existing opt-in entries stay
   valid and are reported as redundant.

## Consequences

- One prebuilt suite serves every repository. CI never source-builds for an
  experiment, and `install-whitaker` needs no input.
- The prebuilt suite grows by the three experimental lints' code.
- An opted-in experimental late lint runs as its own pass rather than inside
  the fused pass, which costs an extra HIR walk for that lint only.
- Experimental lints now ship to everyone, so a defect that would have been
  confined to an opt-in build can now ship in the prebuilt library. The
  mitigation is that an unlisted lint has no pass and cannot execute.
- Removing `ExperimentalLintRequiresFlag` and changing `--lint X` is a breaking
  change to the installer's public API, so the release is a minor version bump
  (0.3.0) of `whitaker-common`, `whitaker-installer`, and `whitaker`.
- `whitaker-installer list` reports the default lints for an installed suite,
  because the scanner cannot tell which suite generation it is looking at.
  Experimental lints appear in `cargo dylint list` and in the installer's
  staging summary.

## Outstanding decisions

- Whether the command also accepts the per-lint names on the command line
  (`whitaker check --experimental-lint NAME`). Out of scope here; the
  configuration route above is sufficient for the first release.
- When the `experimental-*` alias features are removed. Planned for the release
  after 0.3.0.
