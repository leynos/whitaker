# Add a root `whitaker` binary behind an internal library boundary

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Today a user who wants Whitaker installs and runs a binary called
`whitaker-installer`, and reaches the lint inventory through a generated
wrapper script called `whitaker-ls`. The product is named Whitaker but there
is no program called `whitaker`.

After this change there is. A user runs:

```console
whitaker --help
whitaker install
whitaker ls
whitaker ls --json
```

and gets exactly the behaviour `whitaker-installer` and `whitaker-installer
list` give today, from a real Rust binary named `whitaker`, installable with
`cargo install whitaker` or `cargo binstall whitaker`.

That is the entire user-visible outcome. It is deliberately narrow. The
commands `whitaker check` and `whitaker doctor` described in the CLI design
document are **not** part of this plan; they are separate roadmap items that
depend on this one. This plan builds the foundation they land on: a real
root binary, and an internal library boundary that separates decision-making
policy from the input/output work that carries it out.

The second half of the outcome is invisible to users but is the reason the
work is worth doing. The installer's orchestration logic currently lives
inside a binary target (`installer/src/main.rs`, plus two binary-private
modules) where no other program can reach it and no integration test can call
it directly. This plan moves that orchestration into a library crate with
explicit ports, so that the four subcommands still to come can be built by
composing that library rather than by copying the binary.

## Definitions

Terms used throughout, defined here so no prior knowledge is assumed.

**Dylint.** A tool that runs custom Rust lints compiled as dynamic libraries.
Whitaker's lints are Dylint lints. `cargo-dylint` and `dylint-link` are the
two helper binaries Dylint needs.

**Lint bundle / staged library.** A compiled Dylint lint library copied into a
known directory with a filename encoding the toolchain it was built for.
"Staging" is the act of copying it there.

**Prebuilt artefact.** A `.tar.zst` archive of already-compiled lint libraries
published on GitHub Releases, so users do not have to compile lints locally.

**`cargo-binstall`.** A tool that installs a Rust binary by downloading a
prebuilt release archive instead of compiling. It reads a
`[package.metadata.binstall]` table from `Cargo.toml` to learn the archive URL
pattern.

**Port (hexagonal architecture).** A Rust trait, owned by the domain layer,
describing something the domain needs from the outside world (for example
"install the Dylint tools") without saying how it is done.

**Adapter.** A concrete implementation of a port that does the real work, for
example by spawning a process or writing a file.

**Composition root.** The single place — here, `src/main.rs` — where concrete
adapters are constructed and handed to the domain. Nothing else in the program
chooses implementations.

**Driving vs driven.** A _driving_ adapter calls into the domain (the CLI
parser). A _driven_ adapter is called by the domain (the installer).

**ExecPlan plateau.** A milestone that leaves the repository correct,
coherent, and safe to stop at.

## Context and orientation

You have only this repository and this document. Here is what exists.

### The workspace

The Cargo workspace root is the repository root. `Cargo.toml` line 2 declares
members `["common", "crates/*", "installer", "suite"]`. The root directory is
itself a package:

```toml
[package]
name = "whitaker"
version = "0.2.7"
edition = "2024"
```

That root package **has no binary today**. It has only a library,
`src/lib.rs`, which is the shared support library for Whitaker's Dylint lint
crates. Its first two lines matter a great deal to this plan:

```rust
//! Core Whitaker library surfaces shared configuration and helpers for lint crates.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]
```

Under the `dylint-driver` feature this library links `rustc_driver` and the
private compiler crates. A comment in that file records the consequence:

```rust
// Unit tests of this crate should not pull the compiler driver to avoid the
// duplicated `std`/`core` link errors seen during all-features test runs.
```

This is why `Makefile` line 24 excludes the root package from the test run:
`TEST_EXCLUDES` contains `--exclude whitaker`. Read that line before starting
work; it is the single most important constraint on where new code may live.

### The installer

`installer/` is the package `whitaker-installer`. It already has a library
(`installer/src/lib.rs`, 84 lines) exposing about 25 public modules, and four
binaries declared with `autobins = false` in `installer/Cargo.toml` lines
11-27. The one users install is `whitaker-installer`, built from
`installer/src/main.rs`. The other three (`whitaker-package-lints`,
`whitaker-package-installer`, `whitaker-package-dependency-binary`) are
release-packaging utilities and are out of scope here.

`installer/src/main.rs` is 402 lines. It parses `whitaker_installer::cli::Cli`
with clap and dispatches:

```rust
fn run(cli: &Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<()> {
    match &cli.command {
        Some(Command::List(args)) => run_list(args, stdout),
        Some(Command::Install(args)) => run_install(args, stderr),
        None => run_install(cli.install_args(), stderr),
    }
}
```

The important fact is what else is in that file and in two modules declared
only by it (`mod install_flow;` and `mod staged_suite;`, lines 7-8). These are
**binary-private**: they are not part of the `whitaker_installer` library and
no other crate can call them. They contain real orchestration:

- `run_install`, `run_dry`, `try_fast_path_installation`, `finish_install`,
  `finish_install_and_record_metrics`, `resolve_requested_crates`,
  `generate_and_report_wrapper`, `ensure_whitaker_workspace`,
  `resolve_toolchain`, `ensure_toolchain_installed`, `exit_code_for_run_result`
  (all in `installer/src/main.rs`);
- `install_flow::try_prebuilt_installation`, `install_flow::detect_host_target`
  and the `PrebuiltInstallationHooks` struct
  (`installer/src/install_flow/mod.rs`);
- `staged_suite::try_test_staged_suite_installation`
  (`installer/src/staged_suite.rs`, a debug-only test hook).

Moving those behind a library boundary is the "internal library boundary" half
of this task.

### Existing seams

The installer is not a monolith. It already has dependency-injection seams,
but they are inconsistent — three different styles for the same concern:

| Trait / seam | Defined at | Style |
| --- | --- | --- |
| `deps::CommandExecutor` | `installer/src/deps/mod.rs:36` | public trait object |
| `toolchain::CommandRunner` | `installer/src/toolchain/mod.rs:44` | **private** trait object, same shape |
| `dirs::BaseDirs` | `installer/src/dirs.rs:41` | public trait object, `mockall` |
| `builder::CrateBuilder` | `installer/src/builder.rs:58` | public trait object, `mockall` |
| `artefact::download::ArtefactDownloader` | `installer/src/artefact/download.rs:29` | public trait object, `mockall` |
| `install_flow::PrebuiltInstallationHooks` | `installer/src/install_flow/mod.rs:135` | **bare `fn` pointers** |

_Table 1: Existing dependency-injection seams in the installer._

And three places spawn processes with no seam at all:
`installer/src/git.rs` (`Command::new("git")`),
`installer/src/builder.rs:83` (`Command::new("cargo")` inside
`Builder::build_crate`), and `install_flow::detect_host_target`
(`Command::new("rustc")`).

This plan does **not** unify all of those. Doing so would be a large,
independently valuable refactor with its own risk profile. This plan defines
the ports the new CLI needs and implements them over the installer library as
it stands, leaving the installer's internal seam inconsistency for a later
item. That choice is recorded in `Decision log`.

### Where tests live

- Unit tests: colocated, either `#[cfg(test)] mod tests` inline or a sibling
  `_tests.rs` file declared with `#[cfg(test)] mod foo_tests;`.
- Behavioural tests: `<crate>/tests/behaviour_*.rs` integration binaries,
  paired with Gherkin files in `<crate>/tests/features/*.feature`, bound with
  `#[scenario(path = "...", index = N)]`. **The bindings are index-based;
  reordering scenarios in a feature file silently rebinds them.** See the
  warning comment at `installer/tests/behaviour_cli/scenarios.rs:7`.
- Shared behavioural state uses a "World" struct fixture, for example
  `CliWorld` in `installer/tests/behaviour_cli/support.rs`.
- End-to-end CLI tests spawn the binary via the Cargo-provided
  `env!("CARGO_BIN_EXE_<binname>")`. There is no `assert_cmd` in this
  workspace.
- Snapshots live in a `snapshots/` directory beside the test that writes them,
  with `insta`'s default `<crate>__<module>__<name>.snap` naming. Two exist
  today, for example
  `crates/whitaker_clones_core/src/ast/snapshots/whitaker_clones_core__ast__lowering__tests__ast_feature_vector_add_function.snap`.
- Kani harnesses are `#[cfg(kani)]` submodules beside the code, run by
  `scripts/run-kani.sh` via `make kani`.
- Verus proofs are standalone files in `verus/` at the repository root, run by
  `scripts/run-verus.sh` via `make verus`. They are _models_ of the
  implementation, not proofs of the literal Rust source; the trust boundary is
  documented in `docs/developers-guide.md`.

### The gates

Run from the repository root. All four must pass before any commit.

```console
$ make check-fmt   # cargo fmt --all -- --check
$ make typecheck   # cargo check --workspace --all-targets --all-features
$ make lint        # cargo doc, cargo clippy -D warnings, and the Whitaker suite
$ make test        # cargo nextest run over the workspace minus TEST_EXCLUDES,
                   # then cargo test --workspace --doc --all-features
```

Capture output for review, because long output is truncated in agent
transcripts:

```console
make test 2>&1 | tee /tmp/test-whitaker-3-5-1-root-whitaker-binary.out
```

Markdown changes additionally need `make markdownlint`. Do **not** run
`make fmt` for a targeted documentation edit: it runs `mdformat-all` and
reflows every Markdown file in the repository.

## Conformance basis

There is no Terms of Reference document in this repository. The upstream
artefacts are:

- **Design:** `docs/whitaker-cli-design.md`, at the revision present in the
  working tree, specifically §Public CLI surface and §Compatibility and
  migration. Referred to below as `CLI-DESIGN`.
- **Roadmap:** `docs/roadmap.md` item 3.5.1 (line 145). Its stated
  prerequisite, item 3.2.1, is marked done (line 108).
- **ADRs:** `docs/adr-001-prebuilt-dylint-libraries.md` constrains the
  prebuilt-artefact path this plan must not disturb. No existing ADR covers
  the CLI boundary; this plan creates one (see `EP-M4`).
- **Standards:** `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/scripting-standards.md`.

Requirement identifiers used in this plan, each quoting or paraphrasing
`CLI-DESIGN`:

| ID | Statement | Source |
| --- | --- | --- |
| `CLI-REQ-BIN` | "Add a real `whitaker` binary at the root package" | `CLI-DESIGN` §Compatibility and migration, step 1 |
| `CLI-REQ-LIB` | "move the current installer logic behind an internal library boundary" | `CLI-DESIGN` §Compatibility and migration, step 1 |
| `CLI-REQ-BINSTALL` | "copy the working `cargo-binstall` metadata pattern from `whitaker-installer` onto `whitaker`" | `CLI-DESIGN` §Compatibility and migration, step 1 |
| `CLI-REQ-LS` | "`whitaker-ls` disappears in favour of `whitaker ls`"; `ls` "must support `--json`" | `CLI-DESIGN` §Public CLI surface, §Bundle manifests |
| `CLI-REQ-L10N` | "Every human-facing string, including `--help` … should be localizable"; command names and rule codes "are never translated" | `CLI-DESIGN` §Accessibility and localization requirements |
| `CLI-REQ-EXIT` | "install and configuration failures should produce distinct operational errors" | `CLI-DESIGN` §`whitaker check` |
| `CLI-REQ-SHIM` | `whitaker-installer` "survives for one compatibility release as a thin shim" | `CLI-DESIGN` §Public CLI surface |

_Table 2: Upstream requirements traced by this plan._

Trace chain:

```plaintext
CLI-REQ-BIN      -> EP-M2 -> tests::e2e::whitaker_help_lists_install_and_ls
CLI-REQ-LIB      -> EP-M1 -> whitaker_cli::domain unit suite + EP-INV-PARITY
CLI-REQ-BINSTALL -> EP-M3 -> tests::behaviour_binstall::whitaker_package_metadata
                          -> EP-LEM-NAME (verus/whitaker_artefact_naming.rs)
CLI-REQ-LS       -> EP-M2 -> tests::snapshot::ls_json_and_text
CLI-REQ-L10N     -> EP-M2 -> tests::e2e::help_parses_through_localizer
CLI-REQ-EXIT     -> EP-M1 -> EP-INV-EXIT (kani + rstest)
CLI-REQ-SHIM     -> deferred to roadmap 3.9.1, not this plan
```

Requirements explicitly **not** discharged here, with their owning roadmap
item: `whitaker check` (3.5.2); release artefacts and CI packaging (3.5.3);
rule codes and selector precedence (3.6.1, 3.6.2); `whitaker.toml`,
`dylint.toml` bridging and `DYLINT_*` migration (3.6.3); `--locale`/`--colour`
/`--progress` (3.6.4); unified install internals (3.7.x); `doctor`, failure
recording, bundle manifests (3.8.x); the `whitaker-installer` deprecation shim
and `list` alias (3.9.1).

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

1. **`whitaker-installer` keeps working unchanged.** Its command-line surface,
   exit codes, and output must be byte-identical before and after. It is a
   released binary documented in `docs/users-guide.md`, published to GitHub
   Releases, installed by `make install-smoke`, and exercised by
   `installer/tests/behaviour_cli.rs`. `CLI-DESIGN` schedules its deprecation
   for a later release (`CLI-REQ-SHIM`, roadmap 3.9.1), not this one. This is
   not a compatibility shim invented to make a milestone viable: it is the
   currently shipping product, and this plan adds a second entry point beside
   it rather than replacing it.
2. **The public library surface of `whitaker_installer` must not shrink.** The
   integration tests under `installer/tests/` compile against it as an
   external crate and can only see `pub` items. `installer/Cargo.toml` lines
   29-55 gate `StubExecutor` and `InstallerError::StubMismatch` behind the
   `test-support` feature, which roadmap item 3.2.2 declares a supported
   surface for external test suites. Items may be added; existing ones may not
   be removed or narrowed.
3. **The root package must remain named `whitaker` at version `0.2.7`**, and
   the new binary must be named `whitaker`, so that `cargo install whitaker`
   and `cargo binstall whitaker` resolve correctly.
4. **No new lint suppressions.** `Cargo.toml` `[workspace.lints]` sets
   `unsafe_code = "forbid"`, `missing_docs = "deny"`, `allow_attributes =
   "deny"` and clippy `pedantic` at warn with `-D warnings`. Adding
   `#[allow(...)]` to get past a gate is a tolerance breach.
5. **No file may exceed 400 lines** (`AGENTS.md`). Every module needs a `//!`
   doc comment.
6. **Direct environment mutation in tests is forbidden** (`AGENTS.md`). Use
   `temp-env`, or dependency injection through a port.
7. **The prebuilt-artefact download path must not change behaviour.** It is
   governed by `docs/adr-001-prebuilt-dylint-libraries.md` and by the release
   workflow's published asset names.
8. **Caret dependency requirements only** (`AGENTS.md`); no `*` or `>=`.

## Tolerances (exception triggers)

Stop and escalate — do not improvise — when any of these is reached.

- **Scope.** More than 45 files changed, or more than 2,500 net added lines
  across the whole plan. The estimate is roughly 30 files and 1,800 lines.
- **Root-binary feasibility.** If `EP-M0` shows a binary in the root package
  cannot build under `--all-features`, stop at the end of `EP-M0` and
  escalate with the options in `Risk R1`. Do not silently relocate the binary
  to a differently-named package: that would break `cargo install whitaker`
  and violate Constraint 3.
- **Interface.** If discharging `CLI-REQ-LIB` requires removing or narrowing
  any existing `pub` item in `whitaker_installer`, stop (Constraint 2).
- **Dependencies.** Four new dependencies are pre-authorized and listed in
  `Interfaces and dependencies`: `ortho_config`, `googletest`,
  `pretty_assertions`, and `insta` promoted to a root dev-dependency. Any
  fifth new external dependency triggers escalation.
- **Iterations.** If a gate still fails after three fix attempts on the same
  root cause, stop and escalate with the captured log path.
- **Verification.** If a Kani harness exceeds 15 minutes, or a Verus proof
  exceeds 5 minutes, stop and escalate rather than raising the bound or the
  timeout.
- **Behaviour drift.** If any existing `installer/tests/` test needs its
  assertions changed (as opposed to being moved or added to), stop: that is
  evidence of a Constraint 1 violation.
- **Ambiguity.** If `CLI-DESIGN` and `docs/roadmap.md` disagree on whether a
  behaviour belongs to 3.5.1, stop and present both readings.

## Risks

**R1 — A binary in the root package may not build under `--all-features`.**
Severity: high. Likelihood: medium-high.
The root library sets `feature(rustc_private)` and links `rustc_driver` when
`dylint-driver` is enabled. `make typecheck`, `make lint`, and `make test` all
pass `--all-features`, which enables it. `src/lib.rs` records that all-features
test runs produce "duplicated `std`/`core` link errors", and `Makefile` line 24
excludes the package from the test run for that reason. A binary target in the
same package may inherit the same failure.
Mitigation: `EP-M0` is a timeboxed prototyping milestone that answers this
empirically before any design is committed. If it fails, the recommended
remedy is to extract the Dylint driver library out of the root package into
`crates/whitaker_lint_core`, leaving the root package as the CLI package —
which permanently removes the conflict — but that is a scope increase
requiring approval, not an autonomous decision.

**R2 — Index-based BDD scenario bindings break silently.**
Severity: medium. Likelihood: medium.
`#[scenario(path = "...", index = N)]` binds by position. Inserting a scenario
in the middle of an existing `.feature` file rebinds every later scenario to
the wrong step definitions, and the suite may still pass.
Mitigation: put all new scenarios in **new** feature files
(`crates/whitaker_cli/tests/features/*.feature`); never insert into
`installer/tests/features/installer.feature`. `EP-M2` acceptance includes
re-running `installer/tests/behaviour_cli.rs` unchanged.

**R3 — Two packages publishing artefacts with one URL template may collide.**
Severity: high. Likelihood: low-medium.
`installer/Cargo.toml` lines 95-101 template the release URL as
`{name}-{target}-v{version}.{archive-format}`. Giving the `whitaker` package
the same pattern means two packages generate asset names from the same
template. Because `whitaker` is a proper prefix of `whitaker-installer`, and
target triples themselves contain `-`, an ambiguous split is conceivable.
Mitigation: `EP-LEM-NAME`, a Verus proof that the composed name determines its
fields uniquely, plus a `proptest` differential check. See `Verification
plan`.

**R4 — `--help` and `--version` may exit non-zero.**
Severity: medium. Likelihood: medium.
clap reports `--help` as an `Err` variant. Routing every `Err` to exit code 1
would make `whitaker --help` fail. `ortho_config::is_display_request` exists
precisely to distinguish this case, and it is easy to omit.
Mitigation: `EP-INV-EXIT` covers it with both a parameterized test and an
end-to-end assertion on the real process exit status.

**R5 — Promoting binary-private modules widens the public API.**
Severity: medium. Likelihood: medium.
`install_flow` and `staged_suite` are binary-private today. Making them
reachable from a new crate could expose test-only machinery — `staged_suite`
in particular is a debug-only hook driven by the
`WHITAKER_INSTALLER_TEST_STAGE_SUITE` environment variable.
Mitigation: promote to `pub` only what the new ports need; keep
`staged_suite`'s hook behind the existing `#[cfg(debug_assertions)]` and
`test_support` gating; record the resulting surface in the `EP-M4` ADR.

**R6 — `ortho_config` pulls a large dependency subtree.**
Severity: low. Likelihood: high (it is certain; the question is whether it
matters). `ortho_config` 0.9.0 depends on `figment`, `fluent-bundle`,
`fluent-syntax`, `unic-langid`, `clap-dispatch`, `directories`, `xdg`, and
more.
Mitigation: adopt it in this plan for localized parsing only, so the cost is
paid once at the point the roadmap already commits to it (item 3.6.3), not
twice. Confirm `make typecheck` build time does not regress by more than 30%;
report if it does.

**R7 — Snapshot tests of `--help` are brittle across clap versions.**
Severity: low. Likelihood: medium.
`insta` snapshots of help text change whenever clap adjusts its formatting.
Mitigation: snapshot the _structure_ — the subcommand list and the option
names — rather than full rendered help; assert full text only for the
stable `ls --json` output, which is a machine contract.

## Verification plan

This change is mostly a refactor plus a new entry point, so it would be easy
to claim it introduces no invariants. That is not true, and saying so would be
the vacuous option. Three genuine obligations arise, and one lemma.

### Axioms (assumed, not verified here)

- `clap` 4.5 parses an argument vector into the derived struct according to
  its documented derive semantics. Third-party internals are not verified.
- `ortho_config` 0.9.0's `LocalizedParse::try_parse_localized_from` and
  `is_display_request` behave as documented. Repository-owned logic built on
  them **is** verified, against the real interface.
- `cargo-binstall` resolves `pkg-url` by substituting `{name}`, `{target}`,
  `{version}`, and `{archive-format}` literally.
- The GitHub release workflow publishes assets under exactly the names
  produced by `installer/src/artefact/naming.rs`.
- Kani sequentializes concurrency; no obligation below concerns concurrency.

### EP-INV-PARITY — install-argument parity

- **Obligation.** For every argument vector `v` that `whitaker-installer`
  accepts as an install invocation, `whitaker install v` parses to an
  `InstallRequest` equal to the one `whitaker-installer` produces from `v`;
  and for every `v` that `whitaker-installer` rejects, `whitaker install v` is
  rejected too.
- **Method.** Property test (`proptest`), differential.
- **Rationale.** This is the precise formal content of "move the current
  installer behaviour behind a library boundary _without changing it_". The
  flag surface has 14 options with two documented conflict pairs; enumerating
  it by hand would miss combinations, and the space is far too large for
  bounded model checking over strings.
- **Domain.** Generated argument vectors over the 14 flags declared in
  `installer/src/cli.rs:77-181`, including repeated `--lint`, repeated `-v`,
  the `--lint` / `--individual-lints` conflict, the `-v` / `-q` conflict,
  paths containing spaces and non-ASCII characters, and empty values.
- **Artefact.** `crates/whitaker_cli/tests/property_arg_parity.rs`.
- **Evidence.** `cargo nextest run -p whitaker_cli property_arg_parity`. Red
  stage: written before the mapping exists, so it fails to compile, then fails
  on a deliberately incomplete mapping that drops `--jobs`. Discharged when it
  passes with 1,024 generated cases and the regression file is committed.
- **Non-vacuity.** The generator must be _classified_: record via
  `proptest::prop_assume!`-free construction and explicit
  `Strategy::prop_map` that each of the 14 flags appears set in at least 5% of
  cases, that both conflict pairs are generated, and that at least one case
  has zero flags. A run where any flag is never exercised is a **failure**,
  not a pass. Negative control: temporarily drop `--no-update` from the
  `whitaker install` mapping; the test must fail naming that flag. Restore
  afterwards and record the transcript.

### EP-INV-ROUTE — routing totality and conflict rejection

- **Obligation.** The function mapping a parsed CLI to a domain `Request` is
  total (never panics, never returns a "cannot happen" error) over all
  reachable flag combinations, and rejects exactly the two documented conflict
  pairs.
- **Method.** Bounded model check (Kani), complemented by parameterized
  `rstest` cases.
- **Rationale.** Totality over a combinatorial flag space is exactly what
  bounded exhaustive exploration is for, and the space is small enough to
  explore completely once flags are modelled as booleans rather than strings.
  A property test would sample it; Kani covers it.
- **Domain.** The boolean flags modelled as a bitmask, plus a bounded
  `Option<u8>` for `--jobs` and a bounded 0-3 count for `-v`. Ten booleans
  gives 1,024 states; with the two bounded integers the harness explores under
  10^5 states, well inside Kani's practical range for non-heap types.
  `#[kani::unwind(4)]` bounds the single loop over requested lints, capped at
  three entries.
- **Artefact.** `crates/whitaker_cli/src/domain/routing/kani.rs`, gated
  `#[cfg(kani)]`, registered in `scripts/run-kani.sh` alongside the existing
  named harnesses.
- **Evidence.** `make kani 2>&1 | tee /tmp/kani-whitaker-3-5-1.out`. Expect
  `VERIFICATION:- SUCCESSFUL` for
  `verify_route_request_is_total_over_bounded_flags` and
  `verify_route_request_rejects_documented_conflicts`.
- **Non-vacuity.** The harness must drive the **production** routing function,
  not a re-implementation. Assumptions must not collapse the space: assert
  before the main property that at least one satisfying assignment reaches
  each of the three routing outcomes (install, list, conflict-rejected) by
  running three separate `#[kani::proof]` reachability harnesses that assert
  `false` under a constraint selecting that outcome, and confirming each
  reports a counterexample — proving the branch is reachable. Negative
  control: remove the `--lint` / `--individual-lints` conflict check from the
  production function; `verify_route_request_rejects_documented_conflicts`
  must fail with a concrete counterexample. Restore and record.

### EP-INV-EXIT — exit-code policy

- **Obligation.** The process exit code is `0` for success and for a clap
  display request (`--help`, `--version`); `1` for an operational failure. No
  input produces any other code, and no display request produces a non-zero
  code.
- **Method.** Parameterized tests (`rstest` with `googletest` matchers) over
  the finite partition of outcome kinds, plus an end-to-end assertion on the
  real spawned process.
- **Rationale.** The outcome space is a small finite partition — the natural
  fit for parameterized testing. The end-to-end case is what makes it
  non-vacuous, because the unit-level mapping can be right while `main`
  discards it.
- **Domain.** Every variant class of `InstallerError` grouped by kind, plus
  `Ok(())`, plus clap `ErrorKind::DisplayHelp` and `DisplayVersion`, plus a
  genuine parse error.
- **Artefact.** `crates/whitaker_cli/src/domain/exit_tests.rs` and
  `crates/whitaker_cli/tests/e2e_exit_codes.rs`.
- **Evidence.** `cargo nextest run -p whitaker_cli exit`. Red: the e2e test
  asserting `whitaker --help` exits 0 fails against a naive `Err => 1`
  implementation.
- **Non-vacuity.** The e2e test spawns the real binary through
  `env!("CARGO_BIN_EXE_whitaker")` and reads `ExitStatus::code()`, so a
  mapping that is correct in a unit but unwired in `main` is caught. Negative
  control: drop the `is_display_request` branch; `whitaker --help` must then
  exit 1 and the test must fail.

### EP-LEM-NAME — release-asset name unambiguity

- **Obligation.** The composed release-asset name
  `{name}-{target}-v{version}.{ext}` determines `(name, target, version)`
  uniquely. Formally: for well-formed field triples `(n₁,t₁,vs₁)` and
  `(n₂,t₂,vs₂)` drawn from the admissible alphabets, if
  `compose(n₁,t₁,vs₁) = compose(n₂,t₂,vs₂)` then the triples are equal.
- **Method.** Formal proof (Verus), plus a `proptest` differential check
  against the Rust implementation.
- **Rationale.** This is a genuine new obligation created by this change, not
  a restatement. Before this plan only `whitaker-installer` published under
  this template. Adding `whitaker` — a **proper prefix** of
  `whitaker-installer` — into a template whose separator `-` also occurs
  inside every target triple creates a real ambiguity hazard: a wrong split
  means `cargo binstall whitaker` silently fetches the installer's archive.
  The guarantee must hold for all admissible inputs, not a sampled subset, so
  a prover rather than a property test is the right instrument; the property
  test then ties the proven model back to the Rust code.
- **Domain.** Unbounded. `name` over `[a-z0-9_-]+` drawn from the published
  package set; `target` a Rust target triple; `version` a semantic version
  string. The proof proceeds by showing the `-v` delimiter preceding the
  version cannot occur inside a well-formed target triple, which pins the
  version boundary, and that the package-name set is prefix-free **once the
  following separator is included** — the non-obvious step, and the one that
  fails if a future package is named such that the property breaks.
- **Artefact.** `verus/whitaker_artefact_naming.rs`, added to the
  `decomposition`/`clone-detector` group structure in `scripts/run-verus.sh`
  as a new `packaging` group.
- **Evidence.** `make verus 2>&1 | tee /tmp/verus-whitaker-3-5-1.out`. Expect
  `verification results:: N verified, 0 errors`.
- **Non-vacuity.** The proof must not assume its conclusion. Inspect it for
  `assume`: there must be none in the final version, and the well-formedness
  predicates must be shown _inhabited_ by an explicit witness lemma exhibiting
  a concrete satisfying triple (`whitaker`, `x86_64-unknown-linux-gnu`,
  `0.2.7`) before the injectivity theorem is stated — otherwise the theorem is
  vacuously true over an empty domain. Negative control: weaken the
  well-formedness predicate to permit a package name containing `-v` followed
  by digits; the injectivity proof must then fail. Record that failure
  transcript before restoring the predicate.

### Deliberately not verified

- The internals of `clap`, `ortho_config`, `figment`, or `cargo-binstall`.
- The installer's existing behaviour beyond parity. This plan asserts the new
  binary matches the old one; it does not re-verify what the old one does.
  That is already covered by `installer/tests/`.
- Localization catalogue content. `EP-M2` wires `NoOpLocalizer`, so there is
  no translation logic to verify. Fluent catalogues arrive with roadmap 3.6.4.

## Plan of work

### Stage A — prototype and decide (EP-M0, no production code)

Answer `Risk R1` before designing around either outcome. Create a throwaway
`src/main.rs` in the root package containing only:

```rust
//! Feasibility spike: does a root-package binary build under --all-features?
fn main() { println!("spike"); }
```

Then run, capturing output:

```console
cargo check -p whitaker --bins --all-features 2>&1 | tee /tmp/spike-a.out
cargo check --workspace --all-targets --all-features 2>&1 | tee /tmp/spike-b.out
```

Then add `use whitaker::greet;` and a call to it, and repeat, because a bin
that never references the library may not link it — which would make the first
result misleading:

```console
cargo check -p whitaker --bins --all-features 2>&1 | tee /tmp/spike-c.out
```

Go/no-go:

- **Both succeed:** proceed to Stage B with the binary in the root package.
- **Either fails with duplicate `std`/`core` symbols:** delete the spike file,
  record the transcript in `Surprises & discoveries`, set status `BLOCKED`,
  and escalate with the `Risk R1` options. Do not proceed.

Delete the spike file before Stage B regardless of outcome.

### Stage B — red tests and feature specifications

No production behaviour yet. Write the failing specifications first.

Create the crate skeleton `crates/whitaker_cli/` with `src/lib.rs` containing
only module declarations and doc comments, and add it to the workspace. Add
the four dependencies. Then write, in this order:

1. `crates/whitaker_cli/tests/features/whitaker_cli.feature` — the Gherkin
   specification, reproduced in full under `Artefacts and notes`.
2. `crates/whitaker_cli/tests/behaviour_cli.rs` with step definitions and a
   `CliWorld` fixture modelled on `installer/tests/behaviour_cli/support.rs`.
3. `crates/whitaker_cli/tests/e2e_exit_codes.rs` (`EP-INV-EXIT`).
4. `crates/whitaker_cli/tests/property_arg_parity.rs` (`EP-INV-PARITY`).
5. `crates/whitaker_cli/src/domain/routing/kani.rs` (`EP-INV-ROUTE`), plus the
   three reachability harnesses.
6. `verus/whitaker_artefact_naming.rs` (`EP-LEM-NAME`), starting with the
   witness lemma.

Validation for Stage B: every one of the above must **fail**, and the failure
must be the expected one. Record each red transcript. A test that fails
because a module does not exist is acceptable only for the compile-time
skeleton; the behavioural and property tests must reach a genuine assertion
failure once the skeleton compiles.

### Stage C — implementation

Build the library, then the binary, then the packaging metadata. Each step
below names its file and what goes in it; see `Interfaces and dependencies`
for exact signatures.

1. **Promote the binary-private orchestration.** In `installer/src/lib.rs`,
   add `pub mod install_flow;` and move `installer/src/install_flow/` into the
   library. Move the orchestration functions currently in
   `installer/src/main.rs` (`run_install`, `run_dry`,
   `try_fast_path_installation`, `finish_install`,
   `finish_install_and_record_metrics`, `resolve_requested_crates`,
   `generate_and_report_wrapper`, `ensure_whitaker_workspace`,
   `resolve_toolchain`, `ensure_toolchain_installed`) into a new
   `installer/src/orchestration/` module tree, each file under 400 lines.
   Leave `staged_suite` binary-private (`Risk R5`); expose only the single
   entry point the fast path needs, behind its existing gating.
   `installer/src/main.rs` becomes a thin composition root calling the
   library. **No behaviour changes.** Run `make test` here: every existing
   `installer/tests/` test must pass **unmodified**. If any assertion needs
   changing, that is a Constraint 1 breach — stop.

2. **Define the domain and ports** in `crates/whitaker_cli/src/domain/` and
   `crates/whitaker_cli/src/ports/`. The domain owns `Request`, `Outcome`,
   `ExitCode`, the routing function, and the exit-code policy. It imports
   nothing from `std::process`, `std::fs`, or `whitaker_installer`. This is
   the dependency rule, and it is checkable: `crates/whitaker_cli/src/domain/`
   must contain no `use whitaker_installer` and no `use std::{fs, process}`.

3. **Define the driving adapter** in `crates/whitaker_cli/src/cli/`: the clap
   `Parser`/`Subcommand`/`Args` structs for `whitaker`, `whitaker install`,
   and `whitaker ls`, mirroring `installer/src/cli.rs` field for field, plus
   the `ortho_config` localized-parse entry point.

4. **Define the driven adapters** in `crates/whitaker_cli/src/adapters/`,
   implementing the ports over `whitaker_installer`'s now-public
   orchestration.

5. **Add the composition root** at `src/main.rs` in the root package: build
   the adapters, call `whitaker_cli::run`, map the outcome to a process exit
   code. Target under 60 lines. Declare `[[bin]] name = "whitaker"` with
   `autobins = false` in the root `Cargo.toml`, matching the convention at
   `installer/Cargo.toml:11-27`.

6. **Remove `--exclude whitaker` from `TEST_EXCLUDES`** if and only if
   `EP-M0` showed the package tests cleanly; otherwise leave it and note in
   `Surprises & discoveries` that root-package tests remain excluded, with all
   `whitaker_cli` tests living in the non-excluded crate (which is why they
   were put there).

7. **Add binstall metadata.** Parameterize
   `installer/src/binstall_metadata.rs` over the package name rather than
   hardcoding `"whitaker-installer"` (currently at lines 52 and 77), and add
   the `[package.metadata.binstall]` block to the root `Cargo.toml` mirroring
   `installer/Cargo.toml:95-101`, including the
   `overrides.x86_64-pc-windows-msvc` entry with `pkg-fmt = "zip"`.

Validation after each numbered step: `make check-fmt && make typecheck &&
make lint && make test`, captured with `tee`. Commit after each step.

### Stage D — verification, documentation, and wider validation

1. Turn the Verus proof green; run the negative control and record it.
2. Turn the Kani harnesses green; run the negative control and record it.
3. Run the `proptest` non-vacuity classification report and confirm every flag
   is exercised.
4. Write the ADR (`EP-M4`).
5. Update `docs/users-guide.md`, `docs/developers-guide.md`,
   `docs/whitaker-cli-design.md`, `docs/whitaker-dylint-suite-design.md`, and
   `docs/roadmap.md`.
6. Run `make markdownlint` and `make nixie`.

## Milestones and plateaus

### EP-M0 — feasibility established (prototyping)

- **Outcome.** A recorded, evidence-backed answer to whether the `whitaker`
  binary can live in the root package. No production code; the spike file is
  deleted.
- **Requirements.** De-risks `CLI-REQ-BIN`.
- **Acceptance evidence.** `/tmp/spike-a.out`, `/tmp/spike-b.out`,
  `/tmp/spike-c.out`, summarized in `Surprises & discoveries`.
- **Conformance check.** No interface, dependency, or format change.
- **Recovery.** `git checkout -- .` — nothing is committed.
- **Remaining gaps.** Everything.
- **Compatibility decision.** None required.

### EP-M1 — installer orchestration behind a library boundary

- **Outcome.** `installer/src/main.rs` is a thin composition root. All
  orchestration is in the `whitaker_installer` library. `whitaker-installer`
  behaves identically. `crates/whitaker_cli` exists with its domain and ports,
  no adapters yet.
- **Requirements.** `CLI-REQ-LIB`; `EP-INV-ROUTE` and `EP-INV-EXIT` green.
- **Acceptance evidence.** All existing `installer/tests/` pass unmodified;
  `make kani` reports `VERIFICATION:- SUCCESSFUL` for the two routing
  harnesses; `crates/whitaker_cli/src/domain/` contains no `use
  whitaker_installer` (grep-checkable).
- **Conformance check.** Public surface of `whitaker_installer` grew, never
  shrank; no persisted-format change; the prebuilt path is untouched.
- **Recovery.** Revert the milestone's commits; nothing outside the workspace
  changed.
- **Remaining gaps.** No `whitaker` binary yet.
- **Compatibility decision.** None. This is a pre-1.0, application-internal
  boundary; callers are updated in the same change.

### EP-M2 — the `whitaker` binary works

- **Outcome.** `whitaker --help`, `whitaker install`, `whitaker ls`, and
  `whitaker ls --json` all work, with behaviour matching `whitaker-installer`.
- **Requirements.** `CLI-REQ-BIN`, `CLI-REQ-LS`, `CLI-REQ-L10N`,
  `CLI-REQ-EXIT`; `EP-INV-PARITY` green.
- **Acceptance evidence.** The BDD scenarios in `Artefacts and notes` pass;
  `insta` snapshots for `ls` text and JSON are committed; `whitaker --help`
  exits 0.
- **Conformance check.** Command names are untranslated (`CLI-REQ-L10N`);
  `--json` is on `ls` only, not global, as `CLI-DESIGN` requires.
- **Recovery.** The binary is additive; reverting removes it and leaves
  `EP-M1` intact.
- **Remaining gaps.** `check` and `doctor` are absent by design.
- **Compatibility decision.** `whitaker-installer` remains, per Constraint 1
  — named consumer: existing users following `docs/users-guide.md`, and the
  published GitHub release assets. Its removal is roadmap 3.9.1.

### EP-M3 — installable via binstall

- **Outcome.** The root package carries binstall metadata; asset naming is
  proven unambiguous.
- **Requirements.** `CLI-REQ-BINSTALL`; `EP-LEM-NAME` green.
- **Acceptance evidence.** `make verus` verifies
  `verus/whitaker_artefact_naming.rs`; a behavioural test asserts the root
  package's binstall table matches the shared template constants.
- **Conformance check.** `docs/adr-001-prebuilt-dylint-libraries.md` still
  holds; no release-workflow change is made here (that is roadmap 3.5.3), so
  no published asset changes.
- **Recovery.** Metadata-only; revert is safe.
- **Remaining gaps.** CI does not yet _publish_ a `whitaker` artefact — 3.5.3.
  The plan must say so plainly in the ADR rather than implying binstall works
  end-to-end today.
- **Compatibility decision.** None.

### EP-M4 — documented

- **Outcome.** An ADR records the boundary; the user guide, developers' guide,
  CLI design document, and suite design document reflect reality.
- **Requirements.** `AGENTS.md` documentation rules.
- **Acceptance evidence.** `make markdownlint` and `make nixie` pass; the
  roadmap item 3.5.1 checkbox is ticked.
- **Conformance check.** Every discovery from `Surprises & discoveries` is
  reconciled against `CLI-DESIGN`; anything that contradicts it is either
  fixed in the design document or recorded in `Decision log`.
- **Recovery.** Documentation-only.
- **Remaining gaps.** None for 3.5.1.
- **Compatibility decision.** None.

## Interfaces and dependencies

### New dependencies

Add to `[workspace.dependencies]` in the root `Cargo.toml`, caret-pinned:

```toml
ortho_config = "0.9.0"
googletest = "0.14.3"
pretty_assertions = "1.4.1"
```

`insta` is already a workspace dependency (`insta = { version = "1", features
= ["json"] }`); add it as a dev-dependency of `crates/whitaker_cli`.

`ortho_config` is a normal dependency of `crates/whitaker_cli`. The other
three are dev-dependencies only.

**`googletest` ordering rule.** When combining with `rstest`, `#[gtest]` must
come **before** `#[rstest]`, otherwise the test registers twice and runs
twice. Document this in `docs/developers-guide.md`:

```rust
#[gtest]
#[rstest]
#[case::install_with_lint(&["install", "--lint", "module_max_lines"])]
fn routes_to_install(#[case] argv: &[&str]) -> googletest::Result<()> {
    verify_that!(route(parse(argv)?), matches_pattern!(Request::Install(_)))
}
```

### `crates/whitaker_cli` layout

```plaintext
crates/whitaker_cli/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # run(); re-exports; no logic
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── request.rs          # Request, InstallRequest, ListRequest
│   │   ├── outcome.rs          # Outcome, ExitCode
│   │   ├── routing/
│   │   │   ├── mod.rs          # route(): pure
│   │   │   └── kani.rs         # #[cfg(kani)] harnesses
│   │   └── exit.rs             # exit-code policy: pure
│   ├── ports/
│   │   ├── mod.rs
│   │   ├── install.rs          # InstallService
│   │   └── inventory.rs        # LintInventory
│   ├── adapters/
│   │   ├── mod.rs
│   │   ├── installer.rs        # InstallService over whitaker_installer
│   │   └── inventory.rs        # LintInventory over whitaker_installer
│   └── cli/
│       ├── mod.rs              # Cli, Command; localized parse entry point
│       ├── install_args.rs
│       └── list_args.rs
└── tests/
    ├── features/whitaker_cli.feature
    ├── behaviour_cli.rs
    ├── e2e_exit_codes.rs
    └── property_arg_parity.rs
```

### Required signatures

In `crates/whitaker_cli/src/ports/install.rs`:

```rust
/// Performs an installation on behalf of the domain.
pub trait InstallService {
    /// Runs an installation and reports what happened.
    ///
    /// # Errors
    ///
    /// Returns an error when the installation cannot complete.
    fn install(&self, request: &InstallRequest) -> Result<InstallReport, CliError>;
}
```

In `crates/whitaker_cli/src/ports/inventory.rs`:

```rust
/// Reports the lints currently staged on this machine.
pub trait LintInventory {
    /// Lists staged lints in the given staging directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the staging directory cannot be scanned.
    fn list(&self, request: &ListRequest) -> Result<Vec<StagedLint>, CliError>;
}
```

In `crates/whitaker_cli/src/domain/routing/mod.rs` — pure, total, and the
subject of `EP-INV-ROUTE`:

```rust
/// Maps a parsed command line onto a domain request.
///
/// # Errors
///
/// Returns [`RoutingError`] when mutually exclusive flags are combined.
pub fn route(cli: &Cli) -> Result<Request, RoutingError>;
```

In `crates/whitaker_cli/src/domain/exit.rs` — pure, the subject of
`EP-INV-EXIT`:

```rust
/// Maps an outcome onto a process exit code.
#[must_use]
pub const fn exit_code_for(outcome: &Outcome) -> ExitCode;
```

In `crates/whitaker_cli/src/lib.rs`:

```rust
/// Runs the Whitaker command-line interface.
///
/// # Errors
///
/// Returns an error when the command cannot be completed.
pub fn run(
    cli: &Cli,
    installer: &dyn InstallService,
    inventory: &dyn LintInventory,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
) -> Result<Outcome, CliError>;
```

The root `src/main.rs` constructs the two adapters, calls `run`, and converts
the returned `Outcome` with `exit_code_for`. That is all it does.

### Localized parsing

Use `ortho_config`'s `LocalizedParse` with `NoOpLocalizer` at this milestone:

```rust
use ortho_config::{LocalizedParse as _, NoOpLocalizer, is_display_request};

let cli = match Cli::try_parse_localized_from(std::env::args_os(), &NoOpLocalizer) {
    Ok(cli) => cli,
    Err(err) if is_display_request(&err) => { err.print()?; return Ok(ExitCode::SUCCESS); }
    Err(err) => { err.print()?; return Ok(ExitCode::FAILURE); }
};
```

This establishes the localization seam that roadmap 3.6.4 fills with a
`FluentLocalizer` and an `en-GB` catalogue, without adopting a configuration
model this plan does not own.

## Concrete steps

All commands run from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/0c485c79-c21a-486e-b126-29c3ef23084f`.

Confirm the branch first:

```console
$ git branch --show-current
3-5-1-root-whitaker-binary
```

Stage A, the feasibility spike, is given verbatim under `Plan of work`.

Create the crate skeleton:

```console
mkdir -p crates/whitaker_cli/src/{domain/routing,ports,adapters,cli} \
           crates/whitaker_cli/tests/features
```

Add `crates/whitaker_cli` to the workspace — it is already covered by the
`crates/*` glob in `Cargo.toml` line 2, so no edit is needed there; only the
new `crates/whitaker_cli/Cargo.toml` is required.

Run a focused test while iterating:

```console
cargo nextest run -p whitaker_cli 2>&1 \
    | tee /tmp/nextest-whitaker_cli-3-5-1-root-whitaker-binary.out
```

Run the full gate before every commit:

```console
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-3-5-1-root-whitaker-binary.out
make typecheck 2>&1 | tee /tmp/typecheck-whitaker-3-5-1-root-whitaker-binary.out
make lint      2>&1 | tee /tmp/lint-whitaker-3-5-1-root-whitaker-binary.out
make test      2>&1 | tee /tmp/test-whitaker-3-5-1-root-whitaker-binary.out
```

Run them **sequentially**, never in parallel: this environment relies on build
caching and concurrent Cargo jobs contend on the shared package-cache lock.

Verification runs:

```console
make kani  2>&1 | tee /tmp/kani-whitaker-3-5-1-root-whitaker-binary.out
make verus 2>&1 | tee /tmp/verus-whitaker-3-5-1-root-whitaker-binary.out
```

Note that `make test` uses the default nextest profile, which skips
`behaviour_cli` and `behaviour_toolchain`. Before the final commit of `EP-M2`,
run the CI profile once so the new behavioural binary is actually executed:

```console
make test NEXTEST_PROFILE=ci 2>&1 \
    | tee /tmp/test-ci-whitaker-3-5-1-root-whitaker-binary.out
```

Smoke-test the real binary:

```console
cargo run --bin whitaker -- --help
cargo run --bin whitaker -- ls --json
```

Expected shape of the first:

```plaintext
Usage: whitaker <COMMAND>

Commands:
  install  Install or repair Whitaker dependencies and lint bundles
  ls       Show installed lints and bundle metadata
  help     Print this message or the help of the given subcommand(s)
```

## Validation and acceptance

### Red-Green-Refactor evidence to record

**Red.** Before any production code in Stage C:

```console
cargo nextest run -p whitaker_cli 2>&1 | tail -20
```

Expect the BDD scenarios and `e2e_exit_codes` to fail. The e2e failure must
name the missing `whitaker` binary or a wrong exit code — not a compile error
in the test itself.

**Green.** After the minimal implementation of each Stage C step, the focused
command for that step passes.

**Refactor.** After splitting any file approaching 400 lines, re-run the
focused command and then the full gate.

### Behaviour to observe

Acceptance is phrased as things a person can do:

1. Run `whitaker --help`. Observe `install` and `ls` listed as subcommands,
   and `echo $?` printing `0`.
2. Run `whitaker ls --json` in a workspace with staged lints. Observe the same
   JSON that `whitaker-installer list --json` prints, byte for byte.
3. Run `whitaker install --dry-run`. Observe the same output that
   `whitaker-installer --dry-run` prints.
4. Run `whitaker install --lint module_max_lines --individual-lints`. Observe
   a clear rejection and `echo $?` printing `2` (clap's usage-error code) or
   `1` per the routing policy — whichever the implementation settles on must
   be asserted in `EP-INV-EXIT` and documented, not left implicit.
5. Run `whitaker-installer --help`. Observe it is unchanged from before this
   plan.

### Quality criteria

- **Tests.** `make test` and `make test NEXTEST_PROFILE=ci` pass. Every
  existing `installer/tests/` test passes **without assertion changes**.
- **Verification.** `EP-INV-PARITY`, `EP-INV-ROUTE`, `EP-INV-EXIT`, and
  `EP-LEM-NAME` are all discharged, each with its recorded negative-control
  failure transcript. An obligation without a recorded negative control is not
  discharged.
- **Lint/typecheck.** `make check-fmt`, `make typecheck`, `make lint` pass
  with no new suppressions.
- **Documentation.** `make markdownlint` and `make nixie` pass.
- **Performance.** No benchmark threshold. Report if `make typecheck` wall
  time regresses more than 30% (`Risk R6`).
- **Security.** No new network or filesystem capability is introduced; the new
  crate performs I/O only through the two ports, both backed by existing
  installer code.

## Idempotence and recovery

Every step is re-runnable. The spike in `EP-M0` writes one file that is
deleted afterwards. The Stage C steps are additive except for the moves in
step 1, which are pure relocations verifiable by the unchanged test suite.

If a milestone must be abandoned, `git revert` its commits; nothing writes
outside the repository except `/tmp` logs and the normal Cargo target
directory. The `make test` target backs up and restores `~/.local/bin/whitaker`
around its run (`Makefile` lines 92-139) — if a run is interrupted, check that
file was restored before re-running.

Do not create an isolated Cargo cache. Use the shared default cache and let
Cargo's package-cache lock serialize access; if another job holds it, wait.

## Artefacts and notes

### Feature specification (`crates/whitaker_cli/tests/features/whitaker_cli.feature`)

Scenarios must be **appended** to this new file only, never inserted into
existing feature files, because `#[scenario(index = N)]` binds by position
(`Risk R2`).

```gherkin
Feature: The root whitaker command-line interface

  Scenario: The root command lists its subcommands
    Given the whitaker binary is available
    When I run whitaker with "--help"
    Then the command succeeds
    And the output lists the subcommand "install"
    And the output lists the subcommand "ls"

  Scenario: Requesting help exits successfully
    Given the whitaker binary is available
    When I run whitaker with "--help"
    Then the exit code is 0

  Scenario: Listing staged lints as text
    Given a staging directory containing a staged suite library
    When I run whitaker with "ls"
    Then the command succeeds
    And the output names the staged suite

  Scenario: Listing staged lints as JSON
    Given a staging directory containing a staged suite library
    When I run whitaker with "ls --json"
    Then the command succeeds
    And the output is valid JSON

  Scenario: A dry-run install reports its configuration without building
    Given a Whitaker workspace checkout
    When I run whitaker with "install --dry-run"
    Then the command succeeds
    And no lint library is staged

  Scenario: Conflicting lint selection flags are rejected
    Given the whitaker binary is available
    When I run whitaker with "install --lint module_max_lines --individual-lints"
    Then the command fails
    And the error names both conflicting options

  Scenario: The legacy installer binary is unaffected
    Given the whitaker-installer binary is available
    When I run whitaker-installer with "--help"
    Then the command succeeds
    And the output is unchanged from the recorded snapshot
```

### Verus proof skeleton (`verus/whitaker_artefact_naming.rs`)

The witness lemma comes first, so the injectivity theorem is not vacuous:

```rust
use vstd::prelude::*;

verus! {

/// A release-asset name is well formed when its fields are drawn from the
/// admissible alphabets and no field contains the composed delimiter.
pub open spec fn well_formed(name: Seq<char>, target: Seq<char>, version: Seq<char>) -> bool;

/// Exhibits a satisfying triple so `well_formed` is not empty.
proof fn lemma_well_formed_is_inhabited()
    ensures exists|n: Seq<char>, t: Seq<char>, v: Seq<char>| well_formed(n, t, v),
{ /* witness: ("whitaker", "x86_64-unknown-linux-gnu", "0.2.7") */ }

/// Composition determines its fields uniquely.
proof fn lemma_compose_is_injective(
    n1: Seq<char>, t1: Seq<char>, v1: Seq<char>,
    n2: Seq<char>, t2: Seq<char>, v2: Seq<char>,
)
    requires
        well_formed(n1, t1, v1),
        well_formed(n2, t2, v2),
        compose(n1, t1, v1) =~= compose(n2, t2, v2),
    ensures n1 =~= n2, t1 =~= t2, v1 =~= v2,
{ /* by delimiter disjointness, then prefix-freedom of the name set */ }

} // verus!
```

The proof must contain no `assume` in its final form. Per
`docs/developers-guide.md`, Verus proofs here are models of the
implementation, not proofs of the literal Rust source; the `proptest`
differential check in `EP-LEM-NAME` is what ties the model to the code.

## Signposts

Read these before starting.

| Document | Why |
| --- | --- |
| `docs/whitaker-cli-design.md` | The specification. §Public CLI surface and §Compatibility and migration are normative for this plan. |
| `docs/roadmap.md` | Items 3.5.1 through 3.9.3 — what belongs here and what does not. |
| `docs/users-guide.md` | The user-facing surface that must be updated in `EP-M4`. |
| `docs/developers-guide.md` | Installer architecture, Kani harness conventions, the Verus trust boundary. |
| `docs/ortho-config-users-guide.md` | Layering, subcommand merging, and the localization API. |
| `docs/rstest-bdd-users-guide.md` | Writing `#[scenario]` bindings and step functions. |
| `docs/rust-testing-with-rstest-fixtures.md` | Fixture patterns for the `CliWorld` fixture. |
| `docs/rust-doctest-dry-guide.md` | Doctests are gated by `make test`; keep them DRY. |
| `docs/complexity-antipatterns-and-refactoring-strategies.md` | The suite lints this repository against itself; keep routing flat. |
| `docs/whitaker-dylint-suite-design.md` | Workspace layout, updated in `EP-M4`. |
| `docs/whitaker-clone-detector-design.md` | Confirms `whitaker_clones_core`/`whitaker_sarif` naming so the new crate name does not collide. |
| `docs/documentation-style-guide.md` | The ADR template and naming (`docs/adr-NNN-*.md`). |
| `docs/adr-001-prebuilt-dylint-libraries.md` | The prebuilt path this plan must not disturb. |
| `AGENTS.md` | Gates, commit rules, the 400-line limit, the test-environment rules. |

Skills to load: `leta` for symbol navigation instead of grep;
`hexagonal-architecture` for the port and adapter boundaries;
`kani` for `EP-INV-ROUTE`; `verus` for `EP-LEM-NAME`; `proptest` for
`EP-INV-PARITY`; `rust-unit-testing` for `googletest` and `insta` assertion
style; `execplans` for keeping this document current.

## Progress

- [ ] EP-M0 — feasibility spike for a root-package binary.
- [ ] EP-M1 — installer orchestration moved behind the library boundary.
- [ ] EP-M2 — the `whitaker` binary with `install` and `ls`.
- [ ] EP-M3 — binstall metadata and the asset-naming proof.
- [ ] EP-M4 — ADR and documentation updates; roadmap 3.5.1 ticked.

## Surprises & discoveries

- Observation: the root `whitaker` package is excluded from `make test`.
  Evidence: `Makefile` line 24, `TEST_EXCLUDES` contains `--exclude whitaker`;
  `src/lib.rs` records "duplicated `std`/`core` link errors ... during
  all-features test runs".
  Impact: drove the decision to place all testable CLI logic in
  `crates/whitaker_cli` rather than in the root package, and created `EP-M0`.

- Observation: `install_flow` and `staged_suite` are binary-private, so real
  orchestration is unreachable from any other crate today.
  Evidence: `installer/src/main.rs:7-8` declares `mod install_flow;` and
  `mod staged_suite;`, neither appears in `installer/src/lib.rs`.
  Impact: `CLI-REQ-LIB` is a genuine code move, not a re-export.

- Observation: three distinct dependency-injection styles coexist in the
  installer for the same concern.
  Evidence: Table 1 above.
  Impact: unifying them is deliberately out of scope; see `Decision log`.

- Observation: `#[gtest]` must precede `#[rstest]` or the test runs twice.
  Evidence: the `googletest` 0.14.3 crate documentation.
  Impact: recorded as a convention for `docs/developers-guide.md`.

## Decision log

- **Decision:** Place the CLI domain, ports, and adapters in a new crate
  `crates/whitaker_cli`, and make the root `src/main.rs` a thin composition
  root.
  **Rationale:** The root package cannot be tested by `make test` and its
  library requires `feature(rustc_private)`. Putting logic there would make it
  untestable under the repository's own gates. `CLI-DESIGN` requires the
  _binary_ at the root package; it says nothing about where the library lives,
  and "an internal library boundary" is precisely what a separate crate gives.
  **Date/Author:** 2026-08-21, planning agent.

- **Decision:** Adopt `ortho_config` in this plan for localized argument
  parsing only (`LocalizedParse`, `NoOpLocalizer`, `is_display_request`), not
  for configuration layering.
  **Rationale:** The task brief asks for `ortho_config` with localized help.
  `CLI-DESIGN` §Compatibility and migration sequences the configuration switch
  as step 3, and `docs/roadmap.md` gives it its own item, 3.6.3, which
  _requires_ 3.5.1. Wiring `whitaker.toml` discovery and the `dylint.toml`
  bridge here would take work from 3.6.3 and half-activate a configuration
  model this plan cannot finish. Taking the localization seam now satisfies
  `CLI-REQ-L10N`, pays the dependency cost once, and shapes the argument types
  so 3.6.3 adds `#[derive(OrthoConfig)]` and `load_and_merge()` without
  restructuring. **This narrowing should be confirmed before implementation
  begins.**
  **Date/Author:** 2026-08-21, planning agent.

- **Decision:** Do not unify the installer's three dependency-injection styles.
  **Rationale:** It is a large refactor with its own risk profile and no
  requirement in `CLI-DESIGN` driving it. This plan defines the ports the CLI
  needs and implements them over the installer as it stands. Folding the
  refactor in would breach the scope tolerance and blur the parity evidence
  that `EP-INV-PARITY` depends on.
  **Date/Author:** 2026-08-21, planning agent.

- **Decision:** Keep `whitaker-installer` fully functional and unchanged.
  **Rationale:** Not compatibility theatre. It is the currently shipping,
  documented, published binary; `CLI-DESIGN` schedules its deprecation for a
  named later release and `docs/roadmap.md` item 3.9.1 owns that work. The
  named consumers are existing users following `docs/users-guide.md` and the
  GitHub release assets.
  **Date/Author:** 2026-08-21, planning agent.

- **Decision:** Verify asset-name unambiguity with Verus rather than tests
  alone.
  **Rationale:** Adding a second package to a shared URL template, where the
  new name is a proper prefix of the old one and the separator occurs inside
  every target triple, creates a real ambiguity hazard whose guarantee must
  hold for all admissible inputs. A sampled property test cannot establish
  that; a prover can, and the property test then ties the proven model to the
  Rust implementation.
  **Date/Author:** 2026-08-21, planning agent.

- **Decision:** The plan file is named `3-5-1-root-whitaker-binary.md`, not the
  filename given in the task brief.
  **Rationale:** The brief's filename referenced roadmap item 6.5.1, a
  different item (a SARIF emitter for brain-trust diagnostics). The task body,
  branch name, and required pull-request title all identify 3.5.1. Confirmed
  with the requester before drafting.
  **Date/Author:** 2026-08-21, planning agent.

## Outcomes & retrospective

To be completed at each milestone boundary and at completion. Before setting
this plan to `COMPLETE`, reconcile every entry in `Surprises & discoveries`
against `docs/whitaker-cli-design.md`: update the design document where a
discovery contradicts it, raise an ADR where the architecture changed, and
record a purely mechanical difference here. Do not mark the plan `COMPLETE`
while any upstream change or deviation is unrecorded.

## Revision note

Initial draft, 2026-08-21. Covers roadmap item 3.5.1 only. Two points need
explicit confirmation before implementation begins: the `ortho_config`
narrowing recorded in `Decision log`, and the `EP-M0` go/no-go on placing the
binary in the root package.
