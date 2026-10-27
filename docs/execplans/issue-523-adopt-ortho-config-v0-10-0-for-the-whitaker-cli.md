# Adopt OrthoConfig v0.10.0 for the Whitaker CLI

This ExecPlan is a living document. Keep `Constraints`, `Tolerances`, `Risks`,
`Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` current as work proceeds.

Status: implementation in progress; draft PR 525 is open for design review.

Issue: [leynos/whitaker#523](https://github.com/leynos/whitaker/issues/523)

## Purpose / big picture

Whitaker's unified CLI remains planned; the shipped command-line programs are
the clap-based `whitaker-installer` binaries. This change will deliver roadmap
item 3.6.3 by adding an OrthoConfig-backed configuration spine to a separate
`whitaker-cli` application package. It will not add a `whitaker` executable,
start command execution workflows, or alter installer behaviour.

The configuration spine will make the CLI's future precedence and migration
contract executable: defaults, discovered TOML, environment, and explicit CLI
values compose in that order; `whitaker.toml` is canonical; `dylint.toml` and
`DYLINT_LOCALE` remain lower-priority compatibility sources for one release;
and deprecation notices are returned to the future application boundary. Domain
settings remain typed and validated before leaving the application package.

Observable success is a tested config package which resolves settings from
injected sources at every precedence tier, demonstrates the compatibility
bridge and its notices, and builds with the release's Rust 1.89.0 floor while
Whitaker's rustc-private lint crates still use the pinned nightly. The existing
installer CLI test suite must pass without edits to its tests or snapshots.

## Constraints

- Preserve the supplied baseline commit
  `9214b274c062a11c3d8a72de38cac11d8f6e3eff` as the starting point; reconcile
  it with the live base only before delivery.
- Use the imported, byte-preserved v0.10.0 guides and exact source commit
  `38812c563e3953a490e3efc202609f37e21a0517`. The migration guide is nested
  under `docs/ortho-config/` to keep its upstream relative links distinct from
  Whitaker's existing `docs/users-guide.md`. Do not use unreleased APIs.
- Add `ortho_config` only to the application package and keep its default
  features enabled. Do not add it to reusable lint, common, installer, or other
  library crates. Add `ortho_l10n` only if a library needs localization without
  the facade.
- Keep the root `nightly-2026-05-28` toolchain available for Dylint and rustc
  private crates. Isolate the CLI package on stable Rust 1.89.0; do not change
  CI provisioning.
- Preserve the shipped installer's flags, defaults, output, exit statuses,
  streams, and environment behaviour. The new package has no binary entrypoint.
- Use injected `EnvSource` and `ScanEnvSource` values, `MapEnv`, and explicit
  file roots in tests. Do not mutate the process environment in tests.
- Keep parse/help/version handling and errors at the application boundary.
  Configuration libraries return typed results and notices; they do not print
  or terminate the process.
- Do not merge or publish a release. Push the completed candidate and create a
  draft PR after all required checks and requested CodeRabbit reviews pass.

## Configuration decision

This section records the design for implementation. Before adding code, record
the decision in `docs/whitaker-cli-design.md` and a new ADR, then update
roadmap item 3.6.3 if its wording or status needs to be made more precise.

- **Application boundary:** add a workspace package named `whitaker-cli`
  under `cli/`. It owns parser/configuration DTOs, source-aware loading,
  compatibility notices, and conversion into validated settings. It contains no
  executable or command dispatch in this issue. Existing domain and reusable
  crates stay independent of OrthoConfig.
- **Configurable settings:** carry the existing design's `lint.select`,
  `lint.ignore`, and `lint.experimental`; `install.lazy`,
  `install.prefer_prebuilt`, and `install.allow_build_from_source`;
  `diagnostics.locale`; `output.colour` and `output.progress`; and the
  documented per-lint tuning fields. Validate enum-like values when converting
  into domain settings. Preserve the specified `WHITAKER_NO_SOURCE_FALLBACK`
  environment control as the inverse of `install.allow_build_from_source`.
- **CLI-only values:** help, version, config path selection, quiet/verbose,
  JSON output mode, offline operation, forwarded Cargo arguments, and other
  one-run controls that the design does not list as persistent settings. Where
  an existing Clap default represents a value that a file or environment can
  override, use v0.10.0's `cli_default_as_absent` marker.
- **Files and formats:** use `whitaker.toml` as the canonical project and
  platform config name, with `.whitaker.toml` for the hidden project form.
  Retain OrthoConfig v0.10.0's system, user, and project discovery locations;
  TOML is the only enabled format. `--config` and `WHITAKER_CONFIG` select an
  explicit file and take precedence over automatic discovery.
- **Legacy bridge:** when no canonical file is found, probe the corresponding
  automatic locations for `dylint.toml`; never merge it with a canonical file.
  Map the documented legacy locale key `DYLINT_LOCALE` to `diagnostics.locale`
  below `WHITAKER_DIAGNOSTICS_LOCALE`, and return an actionable deprecation
  notice naming the replacement. A malformed selected file is an error and does
  not silently fall through to the other filename.
- **Precedence:** defaults < file layers < legacy environment aliases < new
  `WHITAKER_*` environment < explicitly supplied CLI values. An explicit file
  selector chooses the file source but does not outrank environment or CLI
  settings. Errors from a selected missing/unreadable file are distinct from
  optional automatic-file absence.
- **Subcommands:** reserve per-command file and environment namespaces using
  the OrthoConfig v0.10.0 command convention (`cmds.<command>` and
  `WHITAKER_CMDS_<COMMAND>_*`). Define only schema needed by this roadmap item;
  do not implement command workflows.
- **Localization:** retain the design's `en-GB` fallback and localize help and
  parse failures together through the v0.10.0 facade API. Keep config parsing
  errors typed and suitable for localized presentation. Do not depend on
  localized field defaults, which v0.10.0 documents as unavailable.
- **Profiles and Cargo invocation:** do not offer named profiles or a
  `--profile` flag in this slice. The CLI is not a Cargo subcommand, so no
  external-subcommand wrapper is needed.
- **Compatibility period:** one release. Keep the old file and locale paths
  readable during this window and return notices. Do not remove or change the
  existing suite-level `DYLINT_LOCALE` behaviour in reusable crates.

## Baseline command inventory

The repository has no `whitaker` CLI binary yet. The design document specifies
the planned `check`, `install`, `ls`, and `doctor` commands, with `check` as
the default. Planned shared options include `--config`, `--locale` (default
`en-GB`), `--colour` (default `auto`), `--progress` (default `plain`),
`-q/--quiet`, and repeatable `-v/--verbose`. `check` additionally specifies
`--select`, `--ignore`, `--extend-select`, `--experimental` (default false),
`--no-install`, `--offline`, `--build-from-source`, and forwarded Cargo
arguments after `--`. `ls` and `doctor` specify machine-readable `--json`.
These are design contracts, not currently executable commands.

The shipped `whitaker-installer` accepts no subcommand as install, plus
`install` and `list` subcommands. Install flags are `-t/--target-dir` (platform
default when absent), repeatable `-l/--lint`, `--individual-lints` (false),
`--experimental` (false), `-j/--jobs` (auto when absent), `--toolchain` (auto
when absent), `--cranelift` (false), `--dry-run` (false), repeatable
`-v/--verbose` (zero; alias `--verbosity`), `-q/--quiet` (false), `--skip-deps`
(false), `--skip-wrapper` (false), `--no-update` (false), `--suite-version`
(absent; alias `--suite-ref`), `--build-only` (false), and
`--no-source-fallback` (false). List accepts `--json` (false) and
`-t/--target-dir` (platform scan roots when absent). It has no configuration
file; the existing policy variable is `WHITAKER_NO_SOURCE_FALLBACK`.

Clap help/version requests exit successfully and write to stdout; parse errors
retain Clap's failure status and stderr output. Runtime success returns 0;
runtime errors write to stderr and return 1. `list` writes human-readable or
JSON output to stdout. Install progress and diagnostics use stderr, while the
stable `whitaker-installer: suite-source=...` machine marker uses stdout.
Installer tests also cover the generated `DYLINT_LIBRARY_PATH` shell setup;
that is not a CLI configuration input. Direct baseline probes with `WHITAKER_*`
and `DYLINT_*` variables removed confirmed that `--help` and `--version` return
0 with stdout only, while `--invalid-option` returns 2 with stderr only.

Callers checked include the repository's CI, release, and rolling-release
workflows, `Makefile`, and external workspace callers in Combobulate and
Wildside. Additional repositories contain test and contract fixtures for the
installer. This issue does not change their provisioning or invocation.

The CLI-specific baseline inventory is `installer/src/cli_tests.rs` (24 test
functions/parameterized declarations covering defaults, flags, subcommands,
suite refs, prebuilt policy, verbosity, conflicts, and argument projections),
`installer/tests/behaviour_cli.rs` (11 scenarios), and
`installer/tests/tracing_cli.rs` (one process-level stream/verbosity test). The
full all-target baseline also exercises package utility binaries, installation
behaviour, and UI compile contracts.

## Tolerances (exception triggers)

- Scope: if a real executable, installer changes, CI provisioning changes, or
  a reusable library dependency on OrthoConfig becomes necessary, stop and
  revise the design before proceeding.
- Compatibility: if v0.10.0 cannot express the proposed source order through
  released APIs, stop and record the smallest supported adapter before coding
  around it. Do not use unreleased APIs or disable default features.
- File behaviour: if a selected canonical file and a legacy file need to be
  merged rather than canonical-first fallback, stop and revise the design.
- Validation: if a deterministic gate fails, fix it before the next gate or
  CodeRabbit review. If the same gate failure remains after three attempts,
  stop with the evidence and next action.
- Scope size: keep implementation in the new application package and the
  explicitly named design, user, developer, roadmap, provenance, and plan
  documents. Escalate before expanding beyond that boundary.

## Risks

- **File fallback order may be easy to misstate.** `ConfigDiscovery` chooses a
  first usable candidate, while the compatibility contract requires canonical
  filename preference across locations. Mitigation: test canonical-only,
  legacy-only, both-present, malformed-canonical, and malformed-legacy cases
  with explicit project roots before wiring the loader.
- **Legacy environment aliases may collide with canonical values.** Both
  should occupy a well-defined environment tier, with `WHITAKER_*` winning.
  Mitigation: use named legacy lookup and the injected `MapEnv` to test both
  values together and record the deprecation notice exactly once per key.
- **Stable and nightly toolchains may be confused by workspace commands.**
  Mitigation: run the CLI package from its nested stable-toolchain directory
  and the workspace gates from the root nightly toolchain; verify both compiler
  identities.
- **Imported guides may not satisfy local formatter rules.** Their bytes are
  an explicit requirement. Mitigation: keep them untouched, verify SHA-256
  after all docs gates, and report any conflicting formatter check rather than
  rewriting upstream text.
- **The baseline `whitaker-installer` all-target test run is slow under shared
  compiler admission.** It is active now; let it complete and do not start a
  second build or work around the shared Cargo cache.

## Interfaces and dependencies

- Add `ortho_config = "0.10.0"` with default features enabled, plus only the
  existing workspace dependencies needed for serde, typed errors, and tests.
- Add a `whitaker-cli` package with `rust-version = "1.89.0"` and a nested
  `cli/rust-toolchain.toml` selecting stable `1.89.0`.
- Keep `whitaker`, `whitaker-common`, installer, and all rustc-private lint
  crates free of OrthoConfig and at their current MSRV/toolchain.
- The package's public loader accepts parsed CLI values and injected
  `SharedEnvSource` / `SharedScanEnvSource`; the process-backed wrapper is for
  future application use. It returns validated settings plus deprecation
  notices, with no output side effects.

## Plan of work

1. **Finish baseline and lock the design.** Complete the baseline all-target
   installer test run, record its count and results, and complete caller and
   source inventory. Confirm the source tag and hashes, then record the design
   decision before implementation edits.
2. **Record the decision.** Update the CLI design and roadmap, add an ADR and
   provenance record, and index the imported guides. Run documentation gates,
   verify imported-file checksums, commit this design milestone, and request
   CodeRabbit review before proceeding.
3. **Build the configuration spine.** Add the application package, isolated
   stable toolchain, OrthoConfig dependency, schema, typed settings conversion,
   injected-source loader, and legacy file/environment bridge. Add tests first
   for precedence, subcommand namespaces, compatibility notices, malformed and
   unreadable files, invalid settings, unknown keys, and default-only
   behaviour. Run the package's Rust 1.89 gates and deterministic workspace
   code gates, then request CodeRabbit review and clear all concerns.
4. **Document the user and developer contracts.** Explain file discovery,
   environment mapping, precedence, compatibility duration, error boundary, and
   the package boundary in the users' and developers' guides. Run all
   documentation gates, recheck imported bytes, commit, and request CodeRabbit
   review.
5. **Reconcile and deliver.** Run the canonical gates sequentially on the
   final candidate: formatting, lint, unit/behavioural/UI tests, documentation
   tests, Markdown lint, spelling, and Mermaid validation. Check exact-head
   review and issue/PR state, push over SSH to the requested branch, set its
   tracking upstream, and create a draft PR with `Closes #523` and the Lody
   session link under a terminal `## References` heading. Do not merge.

## Verification

- Baseline: `cargo test -p whitaker-installer --all-targets` on the supplied
  baseline; preserve the test sources and snapshots.
- CLI package: run format, Clippy, unit/BDD tests, and docs with Rust 1.89.0
  from `cli/`; additionally confirm the toolchain identity.
- Full repository: run `make check-fmt`, `make lint`, `make test`,
  `make test-doc`, `make markdownlint`, `make spelling`, and `make nixie`
  sequentially on the final candidate. Use the documented Makefile targets and
  captured logs.
- Behaviour: injected-source tests cover defaults < file < legacy env < new
  env < CLI; subcommand namespaces; explicit source isolation; all file/env
  compatibility cases; and parse/help/config failure boundaries.
- Binary compatibility: the existing installer CLI tests and snapshots pass
  unchanged; no binary or CI workflow has been added or edited.
- Delivery: check GitHub identity using the standard `gh` and SSH clients,
  prove the remote branch head matches local HEAD after push, and read back the
  created PR's title, draft state, base/head, summary, and References section.

## Progress

- [x] 2026-10-26: Read repository instructions, docs index/layout/style, CLI
  design, roadmap, manifests, installer CLI tests, and local caller references.
- [x] 2026-10-26: Confirmed target branch already exists in a separate clean
  worktree at the supplied baseline; exact target PR search returned none.
- [x] 2026-10-26: Live issue #523 is open, no PR uses the target head, and
  GitHub's branch API returned 404 for the remote target branch. The local
  candidate currently tracks `origin/main` and is one commit behind it; set
  `origin/issue-523-adopt-ortho-config-v0-10-0-for-the-whitaker-cli` after the
  eventual SSH push.
- [x] 2026-10-26: Fetched v0.10.0 tag; tag object and peeled commit match the
  supplied values. Imported guides match both upstream Git blobs and recorded
  SHA-256 checksums.
- [x] 2026-10-26: Renamed the Lody session to
  `Adopt ortho_config v0.10.0 for the whitaker CLI (#523)`; its ID is
  `9a515cd8-a3a4-43d3-9fe0-1974854c450c`.
- [x] 2026-10-26: Captured the baseline nightly compiler identity and confirmed
  stable Rust 1.89.0 is installed.
- [x] 2026-10-26: `cargo test -p whitaker-installer --all-targets` passed on
  the supplied baseline: 711 tests passed across 21 test targets, including 11
  CLI behaviour tests, one tracing CLI test, and both UI contracts. The UI
  target took 1,502 seconds under shared compiler admission.
- [x] 2026-10-26: `make fmt`, `make check-fmt`, `make markdownlint`,
  `make spelling`, and `make nixie` passed for the imported guides, provenance,
  index, and plan. Markdown lint reported zero errors; imported guide hashes
  remained unchanged.
- [x] 2026-10-26: The formal CodeRabbit review completed after the requested
  51-minute cooldown. It found a missing plan index and relative-link conflicts
  with Whitaker's existing `docs/users-guide.md`; compatibility entries and a
  nested migration-guide location now preserve upstream bytes while resolving
  those paths. Rerun documentation gates and review these corrections before
  continuing.
- [x] 2026-10-26: The second CodeRabbit review found three more missing upstream
  link targets and five minor grammar issues in the imported migration guide.
  Repository-owned pointers now resolve the three paths. The upstream grammar
  remains byte-preserved as explicitly required; no local source edit is
  appropriate for those five findings. Rerun documentation gates and review
  these corrections before continuing.
- [x] 2026-10-26: The third CodeRabbit review confirmed the compatibility links
  and found that the ADR pointer also needed a contents-index entry. That index
  entry is now added. Its remaining grammar suggestions repeat findings against
  the byte-preserved upstream migration guide and remain intentionally
  unmodified. Rerun documentation gates and review this index correction before
  continuing.
- [x] 2026-10-26: The fourth CodeRabbit review found that the nested migration
  guide's unchanged `../ortho_l10n/README.md` link needed a pointer under
  `docs/ortho_l10n/`. That symlink now resolves to the existing compatibility
  README. The repeated upstream grammar findings remain intentionally
  unmodified. Rerun documentation gates and review this link correction before
  continuing.
- [x] 2026-10-26: The fifth CodeRabbit review found that the legacy migration
  compatibility entry point was not indexed. It is now listed in
  `docs/contents.md`; rerun documentation gates and review the index correction
  before continuing.
- [x] 2026-10-26: Committed the import and planning checkpoint as
  `4a4246166a40ea4c6b27f37bb57979d3f80d8ff9`.
- [x] 2026-10-26: Full baseline gates passed on `e22093e0`: format and lint
  passed, followed by 1,852 nextest passes (five skipped) and 11 Dylint test
  passes. The corrected design review completed with zero findings.
- [x] 2026-10-26: Rebased onto upstream `1fadb66b`, retaining its rewrite
  command design and renumbering the configuration decision as ADR 008.
- [x] The issue request explicitly authorizes implementation and draft PR
  delivery. Continue with the recorded design and sequential milestones.

- [x] 2026-10-27: Opened draft PR 525 for ADR review. A separate agent owns
  the root CLI binary; this branch supplies the configuration foundation.
- [ ] 2026-10-27: Implementing source discovery, compatibility layers, typed
  settings, and injected tests in `cli/`. Targeted nightly tests and Clippy
  passed during iteration; Rust 1.89.0 tests passed before the latest changes.
  Full gates and CodeRabbit remain pending for the implementation candidate.

## Surprises & discoveries

- The requested branch already exists locally and is checked out in another
  worktree at the exact supplied baseline. The original session branch could
  not take the target name, so it was preserved under
  `issue-523-ortho-config-adoption-session`; this work uses the existing target
  branch rather than overwriting it.
- The current workspace has no `whitaker-cli` package or unified CLI binary.
  Existing clap behaviour belongs to the installer package and will remain
  isolated from the future CLI config spine.
- The v0.10.0 facade supports injected discovery and scan sources, ordered
  explicit config selectors, localizable parsing, and TOML by default. Its
  package floor is Rust 1.89.0; its default features include `serde_json` and
  `toml`. Keep those defaults enabled.
- `DYLINT_LOCALE` is the only legacy `DYLINT_*` variable documented as CLI
  configuration. `DYLINT_LIBRARY_PATH` is a lint-library loading variable and
  is outside this CLI compatibility bridge.
- The managed sccache server is not serving compilations, but the build-limit
  supervisor is healthy and builds continue uncached through the shared queue.

## Decision Log

- Decision: keep the existing installer interface untouched and add a
  config-only `whitaker-cli` application package, with no executable. This
  delivers the named roadmap item without starting the planned command
  workflows. Date: 2026-10-26.
- Decision: isolate the application package on stable Rust 1.89.0 while leaving
  the root nightly pin and rustc-private crates unchanged. Date: 2026-10-26.
  Status: implementation authorized by the issue request.
- Decision: canonical-first `whitaker.toml` discovery, one-release
  `dylint.toml` fallback, legacy locale alias below `WHITAKER_*`, and explicit
  returned deprecation notices. Date: 2026-10-26.
- Process: request CodeRabbit only after deterministic gates pass for each
  major milestone; treat a rate limit as a workflow wait and use the requested
  `vsleep` cooldown if it occurs.

## Outcomes & Retrospective

Implementation and delivery are pending.
