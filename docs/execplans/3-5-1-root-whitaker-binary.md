# Add a root `whitaker` binary behind an internal library boundary

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

## Purpose / big picture

The product is called Whitaker, but there is no program called `whitaker` —
only a binary called `whitaker-installer` and a shell script the installer
generates. After this change there is a real one:

```console
whitaker --help
whitaker install
whitaker ls
whitaker ls --json
```

These do exactly what `whitaker-installer` and `whitaker-installer list` do
today. Crucially, the existing workflow keeps working too:

```console
whitaker --all -- -p whitaker-common --all-targets
```

That form is not a subcommand. It is the contract of the shell script the
installer writes to `~/.local/bin/whitaker`, and it is what this repository's
own `make lint` runs. The new binary forwards it to `cargo dylint`, so a user
whose `PATH` resolves to the new binary sees no regression. See `Decision log`
entry D-2 and requirement `CLI-REQ-FWD`.

The invisible half of the outcome is the reason the work is worth doing, and it
is larger than it first appears. Two structural problems block a root binary
today, and both are fixed here:

1. **The root package cannot be published.** It depends on four
   `publish = false` compiler-shim crates, and its manifest has no
   `description`, `license`, or `repository`. So `cargo install whitaker`
   cannot resolve — the headline outcome is unreachable until this is fixed.
2. **The installer's orchestration is trapped inside a binary target.** Ten
   functions in `installer/src/main.rs` and two binary-private modules cannot
   be called by any other program or tested directly.

The commands `whitaker check` and `whitaker doctor` are **not** in this plan;
they are separate roadmap items that depend on this one.

## Definitions

Defined here so no prior knowledge is assumed.

**Dylint.** A tool that runs custom Rust lints compiled as dynamic libraries.
Whitaker's lints are Dylint lints. `cargo-dylint` and `dylint-link` are the
helper binaries it needs.

**Dylint driver.** Code that links the private `rustc_*` compiler crates and
therefore needs `#![feature(rustc_private)]` and a nightly toolchain. It is
contagious: anything depending on it inherits the constraint.

**Lint bundle / staged library.** A compiled lint library copied into a known
directory under a filename encoding the toolchain it was built for.

**Prebuilt artefact.** A `.tar.zst` archive of already-compiled lint libraries
published on GitHub Releases, so users need not compile lints locally.

**`cargo-binstall`.** Installs a Rust binary by downloading a prebuilt release
archive rather than compiling. It reads `[package.metadata.binstall]` from the
package's manifest **as published on crates.io**.

**Wrapper script.** The executable file named `whitaker` that
`installer/src/wrapper.rs` writes into the user's binary directory. Its whole
body sets `DYLINT_LIBRARY_PATH` and runs `exec cargo dylint "$@"`.

**Port / adapter (hexagonal architecture).** A port is a trait, owned by the
policy layer, describing something it needs from the outside world. An adapter
implements it. A _driving_ adapter calls into the policy (the CLI parser); a
_driven_ adapter is called by it (the installer).

**Composition root.** The one place — `src/main.rs` — where concrete adapters
are constructed. Nothing else selects implementations.

**Plateau.** A milestone leaving the repository correct, coherent, and safe to
stop at.

## Progress

- [x] `EP-M0` — extracted the compiler-private Dylint driver into
  `whitaker_lint_core`; the root package now packages and tests independently.
- [x] `EP-M1` — moved installer orchestration behind the installer library API
  and protected the legacy dry-run output with a snapshot.
- [x] `EP-M2a` — root commands, forwarding, Kani proof, and bounded property
  checks are implemented.
- [x] `EP-M2b` — CLI Verus proof/control pass in default `make verus`.
- [x] `EP-M3a` — binstall and platform package metadata implemented.
- [ ] `EP-M3b` — crates.io name reservation remains unauthorized.
- [x] `EP-M4` — architecture and user/developer documentation.

Resumed 2026-10-15; rebased onto `origin/harden-lint-config`
(`afb1e4ac1387aff8e51fa7bbeb62d94f0f6c8970`), still an ancestor. D-11/D-12
accepted the then-current 80-file diff and set a 7,000-line ceiling for this
plan only.

The CLI delivers `install`, transitional `ls`, localized help/version, legacy
forwarding, per-toolchain install records, and binstall metadata. Kani passed
360 checks; the CLI Verus proof and its negative control pass. Name reservation
remains unauthorized, so the checked roadmap item keeps this plan open.

Published `a02ddff` failed three hosted tests because `workspace_root` selected
the manifest's parent. `8444efa` fixes the path and recovers abandoned locks.
Fresh review of `20494294` found three defects; the current tree addresses
signal exit status, library path selection, and the layout description.

Before Verus edits, nine gates passed (1,713 tests, 5 skipped, doctests,
format, typecheck, lint, docs, spelling, Makefile, packageability). Both Verus
groups passed; the instrumented coverage attempt timed out in trybuild.

**Review follow-up, 2026-10-25.** A completeness assessment on `3759ee4` found
mixed short clusters losing common flags and forwarded Windows statuses being
narrowed to one byte. Source review confirmed both; regression fixes now pass
`make check-fmt`, `make test` (1,703 passed, 5 skipped, doctests passed),
`make typecheck`, `make lint`, `make markdownlint`, and `make nixie`.
`cargo package -p whitaker --no-verify --allow-dirty` also passes.

Negative-control evidence is recorded here so it survives local log rotation:

- `EP-INV-PARITY`: dropping `--no-update` from the root-to-installer mapping
  made the 4,096-case property test fail at case 12; the expected parsed value
  was `no_update=true`, while the root mapping returned `false`.
- `EP-INV-EXIT`: mapping `whitaker --help` to a usage error made the process
  exit-code test fail with actual status 2 instead of expected status 0.
- `EP-INV-DISPATCH`: removing the `Ls` classifier arm made the scoped Kani
  proof fail at check 64 of 361. The three reachability controls each produced
  the expected counterexample.
- `EP-LEM-DISPATCH`: widening the Verus disjointness guard to overlap `Help`
  produced 3 verified obligations and 1 expected error.

`cargo package -p whitaker --no-verify --allow-dirty` passed, producing a
15-file archive (133 KiB unpacked, 36.3 KiB compressed). The normal dependency
count fell from 342 to 189; the stripped release binary is 5,040,816 bytes
(6,506,472 bytes unstripped).

The assessment on `41e3f52f` found parser value-boundary and repeated-quiet
parity gaps. The candidate validates install arguments through the installer
parser, adds regression coverage for both legacy forms and a Windows child-exit
test, and adds root-package validation to CI. The published candidate's local
gates pass, and the Windows child-exit regression returned 256 in hosted
Windows CI on `ff2d4f67` (Actions run `40220275632`). A helper split clears
CodeScene's complexity finding, and the short-cluster cases move into
`tests/property_arg_parity/short_clusters.rs` to keep the parity test under the
400-line limit. D-23 moves `ortho_config` to 0.10.0, and an end-to-end
assertion pins bare `whitaker` to `cargo dylint` forwarding. The 130-file diff
is +7,400 net lines, 400 above D-12's ceiling, and exceeds D-11's accepted 80
files. Resolve this scope with the owner before merge. Crates.io reservation
remains unauthorized; the plan stays `IN PROGRESS`.

**Rebase and review repair, 2026-11-05.** The branch was replayed onto
`origin/main` (`15e65ba8`) with `zdiff3`, keeping both sides where each
contributed something and taking the target's lock file before rebuilding it.
Review of the replayed tree produced the repairs below.

- The root package's `[lints] workspace = true` was reverted. Moving the
  strict Clippy restriction set into `[workspace.lints.clippy]` was measured
  and fails: the four packages that inherit workspace lints produce 134 denials
  (`shadow_reuse`, `cognitive_complexity`, `needless_pass_by_value`,
  `indexing_slicing`, `const fn`, float comparisons). The root crate instead
  keeps its own `[lints.clippy]` and mirrors the workspace `rust` and `rustdoc`
  tables, because Cargo forbids `workspace = true` beside package lint tables.
  This closes the root crate's real gap — it had been skipping `missing_docs`,
  `unknown_lints`, `renamed_and_removed_lints`, and every rustdoc denial.
- The root install now passes `WrapperScripts::None`. The generated
  `whitaker-ls` calls `whitaker-installer run`, so writing it from a root
  install would leave a script that fails unless the installer is separately
  installed; `whitaker ls` provides the listing. `whitaker-installer` still
  passes `Both`, and the intermediate `WhitakerLsOnly` variant was removed now
  that nothing constructs it.
- `src/adapters/mod.rs` was split into `src/adapters/tests.rs` to stay under
  the 400-line limit after the rebase.
- Adapter directory lookups now take a `BaseDirs` provider instead of
  constructing `SystemBaseDirs` internally. `LintRunner::forward` resolves it
  once and threads one `Option<&dyn BaseDirs>` through every lookup, so the
  record tests supply a `TempBaseDirs` fixture rather than redirecting
  `XDG_DATA_HOME`. Windows resolves the data directory from the known-folder
  API and ignores that variable, which is why the previous head failed three
  adapter tests on `windows-compat`.
- `tests/e2e_exit_codes.rs` was split into three processes, the `fake_cargo_dir`
  fixture now drops its directory handle instead of claiming to keep it, and
  the recorded-path behaviour fixture canonicalizes its staging directory so
  the macOS `/var` → `/private/var` resolution no longer fails an unrelated
  assertion.
- The nextest workflow contract test now asserts the two-pass split: both
  `trybuild-contracts` passes carry the contract filter and the `ci` profile,
  and only the feature-free `whitaker_lint_core` pass tolerates an empty match.

Documentation was reconciled with those decisions in ADR-011, the CLI design
document, `docs/publishing.md`, and `docs/users-guide.md`.

Gate evidence for this repair commit, run sequentially against the worktree
that becomes the commit:

| Gate                           | Result                       | Log                                                                    |
| ------------------------------ | ---------------------------- | ---------------------------------------------------------------------- |
| `make check-fmt`               | pass                         | `/tmp/check-fmt-whitaker-3-5-1-root-whitaker-binary.out`               |
| `make lint`                    | pass                         | `/tmp/lint-whitaker-3-5-1-root-whitaker-binary.out`                    |
| `make typecheck`               | pass                         | `/tmp/typecheck-whitaker-3-5-1-root-whitaker-binary.out`               |
| `make test`                    | pass, 2203 passed, 6 skipped | `/tmp/test-whitaker-3-5-1-root-whitaker-binary.out`                    |
| `NEXTEST_PROFILE=ci make test` | pass, 2221 passed, 6 skipped | `/tmp/test-ci3-whitaker-3-5-1-root-whitaker-binary.out`                |
| `make markdownlint`            | pass, 114 files              | `/tmp/markdownlint-whitaker-3-5-1-root-whitaker-binary.out`            |
| `make nixie`                   | pass                         | `/tmp/nixie-whitaker-3-5-1-root-whitaker-binary.out`                   |
| `make test-workflow-contracts` | pass, 793 passed             | `/tmp/test-workflow-contracts-whitaker-3-5-1-root-whitaker-binary.out` |

**Base merge, 2026-11-08.** `origin/main` advanced to `9fc35c08` (PR #371)
while the candidate was being gated, which made the PR `DIRTY`. The merge of
`origin/main` into `330cb3dd` resolved eleven conflicts by keeping this
branch's product changes and `main`'s hardened gate configuration (see D-28).
Two semantic conflicts surfaced only under the compiler rather than in a
conflict marker: `crates/string_continuation_style/tests/support/ui.rs` still
named `whitaker::testing::ui::HarnessError` after `main` introduced that
harness into a crate this branch moved, and `bootstrap/run.rs` had two
independent complexity repairs for the same function. Both are resolved.
`Cargo.lock` merged without conflict; the packaging lock file policy (take
`main`'s, then rebuild) needs no change because the manifests' dependency sets
did not disagree.

**Review qualifications, 2026-11-08.** CodeRabbit accepted the two isolation
repairs and raised four residual qualifications, all valid (see D-29). The
record now reports an uninspectable-only record separately from a stale one;
the end-to-end test refuses to spawn a child it cannot isolate; the bin root is
named through a second debug-only seam so the executable assertions inspect the
directory the child resolved on every platform; and the test now follows the
record its own install wrote out to a forwarded lint run started outside the
checkout, with a stub `cargo` in place of Dylint.

**Gate repair, 2026-11-08.** The first gate run found one real defect in the
new fixture: Clippy's `unused_self` on `InstallFixture::external_project`,
which took `&self` and never used it. It is now an associated function, and the
caller in `tests/root_install_e2e.rs` names the type instead. The repaired
fixture changes the `root_install_e2e` target, so the full gate sequence was
re-run on the repaired tree rather than carried over from the earlier green
pass. `make fmt` then reflowed the fixture, so `make lint` and `make typecheck`
were repeated on exactly the tree that was tested. All six gates passed:
`make check-fmt`, `make lint`, `make typecheck`, `make test` (2410 nextest
tests passed with 6 filtered by the default profile, 84 lint-core tests, and 19
`dylint_testing` tests), `make markdownlint`, and `make nixie`.

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

1. **`whitaker-installer` keeps working unchanged.** Its command-line surface,
   exit codes, and output must be identical before and after. It is a released
   binary documented in `docs/users-guide.md`, published to GitHub Releases,
   exercised by `make install-smoke` and `installer/tests/`. Its deprecation is
   roadmap 3.9.1, not this plan. This is not a shim invented to make a
   milestone viable; it is the shipping product.
2. **`whitaker --all` keeps working.** The wrapper script's contract is
   documented at `docs/users-guide.md:25-27` and consumed by `Makefile:185` and
   `.github/workflows/ci.yml:154`. Named consumers: existing users, and this
   repository's own lint gate.
3. **The public library surface of `whitaker_installer` must not shrink.**
   Integration tests under `installer/tests/` compile against it as an external
   crate and see only `pub` items; `installer/Cargo.toml:29-55` gates
   `StubExecutor` and `InstallerError::StubMismatch` behind the `test-support`
   feature, which roadmap 3.2.2 declares supported. Additions are fine;
   removals and signature narrowings are not.
4. **The root package stays named `whitaker`**, and the new binary is named
   `whitaker`. The workspace version has moved with each release since this
   plan was written and is now `0.2.11`; the constraint fixes the package and
   binary name, not the version number.
5. **The Dylint lint crates must keep building and their UI tests passing**
   throughout `EP-M0`. They are the oracle for that milestone.
6. **No new lint suppressions.** `Cargo.toml [workspace.lints]` sets
   `unsafe_code = "forbid"`, `missing_docs = "deny"`,
   `allow_attributes = "deny"`, clippy `pedantic` at warn with `-D warnings`.
   Adding `#[allow(…)]` to pass a gate is a tolerance breach. Adding an
   `excluded_crates` entry to `dylint.toml` counts as a suppression for this
   purpose and requires the same escalation.
7. **No file may exceed 400 lines** (`AGENTS.md`). Every module needs a `//!`
   doc comment.
8. **No direct environment mutation in tests** (`AGENTS.md`). Use `temp-env`
   or inject through a port.
9. **The prebuilt-artefact path must not change behaviour.** Governed by
   `docs/adr-001-prebuilt-dylint-libraries.md`.
10. **Caret dependency requirements only**; no `*` or `>=`.

## Tolerances (exception triggers)

Stop and escalate; do not improvise.

- **Scope.** More than 70 files changed, or more than 7,000 net added lines.
  The current 80-file diff is accepted by D-11; the 7,000-line cap is set by
  D-12 for this plan only. `EP-M0` alone touches roughly 40 files, almost all
  mechanically.
- **`EP-M0` gate.** If `cargo package -p whitaker --no-verify` still fails
  after the extraction, stop. That command is the milestone's whole point.
- **Interface.** If anything requires removing or narrowing an existing `pub`
  item in `whitaker_installer`, stop (Constraint 3). Additive `*_for` functions
  are the sanctioned pattern.
- **Behaviour drift.** If any existing test under `installer/tests/` or any
  Dylint UI fixture needs its _assertions_ changed, stop. Relocation is fine;
  changed expectations are evidence of a Constraint 1 or 5 breach.
- **Dependencies.** Three new dependencies are pre-authorized: `ortho_config`,
  `googletest`, `pretty_assertions`. A fourth triggers escalation.
- **Iterations.** Three failed fix attempts on one root cause; report the log
  path.
- **Verification.** Kani verifier execution over 15 minutes for this plan's
  harnesses in aggregate, or Verus verifier execution over 10 minutes, or more
  than one working day spent authoring either. Cargo compilation and shared
  admission time are recorded separately and do not count toward verifier
  execution limits. These are the repository's first sequence-shaped proofs;
  budget overrun is the expected failure mode.
- **Ambiguity.** If `docs/whitaker-cli-design.md` and `docs/roadmap.md`
  disagree on whether a behaviour belongs to 3.5.1, stop and present both
  readings. This fired twice during planning; see `Decision log` D-2 and D-3.

## Risks

**R2 — Forwarding may misroute argv or lose subprocess status.** Severity:
high. The classifier selects Whitaker or forwards to `cargo dylint`; errors
break an interface. Kani, CLI tests, and unbounded Verus cover routing,
reachability, and child status. The revised proof verifies within tolerance
(`EP-M2b`).

**R6 — Publishing instructions may precede name reservation.** Severity:
medium, with high impact. The `whitaker` name remains unreserved. Keep official
publication gated on authorized reservation; `EP-M3b` remains open.

## Context and orientation

Work from this repository, using this plan as the task record. Run commands
from the repository root, which `git rev-parse --show-toplevel` identifies.

### The root package today

This subsection records the planning baseline before implementation, not the
current state; see [Progress](#progress) and
[Outcomes & retrospective](#outcomes--retrospective) for the current package
version and layout.

The repository root is itself a Cargo package:

```toml
[package]
name = "whitaker"
version = "0.2.7"
edition = "2024"
```

It has **no binary**, only `src/lib.rs`, which is the shared support library
for the Dylint lint crates. Its second line is the crux:

```rust
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]
```

Under that feature the library links `rustc_driver`. A comment in the same file
records the consequence — "duplicated `std`/`core` link errors seen during
all-features test runs" — and that is why `Makefile:23` excludes the package
from the test run.

Two things follow, and this plan exists to fix both. First, the package cannot
be published:

```console
cargo package -p whitaker --no-verify --allow-dirty
```

```plaintext
warning: manifest has no description, license, license-file, documentation,
         homepage or repository
error: failed to prepare local package for uploading
Caused by:
  no matching package named `rustc_ast` found
  location searched: crates.io index
  required by package `whitaker v0.2.7`
```

`crates/rustc_{ast,hir,lint,middle,session,span,attr_data_structures}` are all
`publish = false`. Second, eleven lint crates depend on this package, so
anything added to its `[dependencies]` propagates to all of them — Cargo has no
per-target dependency tables.

### The installer

`installer/` is the package `whitaker-installer`. It already has a library
(`installer/src/lib.rs`) exposing about 25 public modules, and four binaries
declared with `autobins = false` (`installer/Cargo.toml:11-27`). Users install
`whitaker-installer`, built from `installer/src/main.rs`; the other three are
release-packaging utilities and are out of scope.

`installer/src/main.rs` is 402 lines and dispatches:

```rust
fn run(cli: &Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<()> {
    match &cli.command {
        Some(Command::List(args)) => run_list(args, stdout),
        Some(Command::Install(args)) => run_install(args, stderr),
        None => run_install(cli.install_args(), stderr),
    }
}
```

Note the `None` arm: bare `whitaker-installer` installs. `Cli` flattens
`InstallArgs` alongside the optional subcommand (`installer/src/cli.rs:55-63`,
with `install_args()` at `:283-288`).

What matters here is what else lives in that binary and cannot be reached from
anywhere else. `installer/src/main.rs:7-8` declares `mod install_flow;` and
`mod staged_suite;` — neither appears in `installer/src/lib.rs`. Together with
ten functions in `main.rs` (`run_install`, `run_dry`,
`try_fast_path_installation`, `finish_install`,
`finish_install_and_record_metrics`, `resolve_requested_crates`,
`generate_and_report_wrapper`, `ensure_whitaker_workspace`, `resolve_toolchain`,
`ensure_toolchain_installed`) they hold the real orchestration.

Useful detail for `EP-M1`: items inside `installer/src/install_flow/mod.rs` are
already `pub(crate)`, and `PrebuiltInstallationHooks` at `:136` is fully
private. Once the module sits inside the library, `pub(crate)` already means
library-internal, so only a small named facade needs to become `pub`. Do not
over-promote.

### The wrapper script — the contested name

`installer/src/wrapper.rs:105` writes `bin_dir.join("whitaker")`, an executable
whose body is:

```bash
export DYLINT_LIBRARY_PATH="{library_path}"
exec cargo dylint "$@"
```

This is not a convenience. It is currently the only way to run Whitaker's
lints. It is documented (`docs/users-guide.md:25-27, 51-57`), wired into
`Makefile:74-75` and `:185`, and run by `.github/workflows/ci.yml:154`.

`Makefile:9` prepends `$(HOME)/.cargo/bin` to `PATH` while `Makefile:6` appends
`$HOME/.local/bin`. So a `cargo install`ed binary at `~/.cargo/bin/whitaker`
**deterministically wins** over the script at `~/.local/bin/whitaker`.
`Makefile:99-139` wraps the whole test run in a `trap`-based backup and restore
of that file, failing loudly on modification — somebody has already been burned
by it.

That is why this plan forwards unrecognized arguments rather than rejecting them
(`Decision log` D-2).

### Existing dependency-injection seams

The installer already has seams, in three styles for one concern:

| Trait or seam                             | Defined at                              | Style                                    |
| ----------------------------------------- | --------------------------------------- | ---------------------------------------- |
| `deps::CommandExecutor`                   | `installer/src/deps/mod.rs:36`          | public trait, `mockall`                  |
| `toolchain::CommandRunner`                | `installer/src/toolchain/mod.rs:44`     | **private** trait, `mockall`, same shape |
| `dirs::BaseDirs`                          | `installer/src/dirs.rs:41`              | public trait, `mockall`                  |
| `builder::CrateBuilder`                   | `installer/src/builder.rs:58`           | public trait, `mockall`                  |
| `artefact::download::ArtefactDownloader`  | `installer/src/artefact/download.rs:29` | public trait, `mockall`                  |
| `install_flow::PrebuiltInstallationHooks` | `installer/src/install_flow/mod.rs:136` | bare `fn` pointers                       |

_Table 1: Existing dependency-injection seams in the installer._

Three places spawn processes with no seam at all: `installer/src/git.rs`,
`Builder::build_crate` (`installer/src/builder.rs:83`), and
`install_flow::detect_host_target`.

This plan does **not** unify them (`Decision log` D-5).

### Where tests live

- Unit tests: colocated, inline `#[cfg(test)] mod tests` or a sibling
  `_tests.rs` declared with `#[cfg(test)] mod foo_tests;`.
- Behavioural tests: `<crate>/tests/behaviour_*.rs` with Gherkin files in
  `<crate>/tests/features/*.feature`, bound by `#[scenario(path, index)]`.
  Shared state uses a "World" fixture; see
  `installer/tests/behaviour_cli/support.rs`.
- End-to-end CLI tests spawn `env!("CARGO_BIN_EXE_<binname>")`. **Cargo sets
  that variable only for integration tests of the package that declares the
  binary.** There is no `assert_cmd` in this workspace.
- Snapshots: a `snapshots/` directory beside the test, `insta`'s default
  `<crate>__<module>__<name>.snap` naming.
- Kani: `#[cfg(kani)] mod verification` colocated with the code — see
  `docs/developers-guide.md:737` — run by `scripts/run-kani.sh` via `make kani`.
- Verus: standalone files in `verus/` at the repository root, run by
  `scripts/run-verus.sh` via `make verus`. They are _models_ of the
  implementation, not proofs of the literal Rust source; the trust boundary is
  documented in `docs/developers-guide.md`.

### The gates

```console
make check-fmt
make typecheck
make lint
make test
make test NEXTEST_PROFILE=ci
make markdownlint
```

Capture output, because long transcripts are truncated:

```console
make test 2>&1 | tee /tmp/test-whitaker-3-5-1.out
```

Run them **sequentially** — this environment relies on build caching and
concurrent Cargo jobs contend on the package-cache lock. Do not run `make fmt`
for a targeted documentation edit; it reflows every Markdown file. Note that
`make markdownlint` also runs a `typos` spelling gate enforcing Oxford spelling
(`-ize`, but `behaviour` keeps `-our`), and `typos` only inspects git-tracked
files — an untracked draft passes and then fails once committed.

## Conformance basis

No Terms of Reference document exists. Upstream artefacts:

- **Design:** `docs/whitaker-cli-design.md` at the revision in the working
  tree, §Public CLI surface and §Compatibility and migration. Cited as
  `CLI-DESIGN`.
- **Roadmap:** `docs/roadmap.md` item 3.5.1 (line 145). Its prerequisite,
  3.2.1 (line 108), is done.
- **ADRs:** `docs/adr-001-prebuilt-dylint-libraries.md` constrains the
  prebuilt path. No ADR covers the CLI boundary; this plan creates
  `docs/adr-011-whitaker-cli-boundary.md`.
- **Standards:** `AGENTS.md`, `docs/documentation-style-guide.md`.

| ID                 | Statement                                                                                                                        | Source                                            |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| `CLI-REQ-BIN`      | "Add a real `whitaker` binary at the root package"                                                                               | `CLI-DESIGN` §Compatibility and migration, step 1 |
| `CLI-REQ-LIB`      | "move the current installer logic behind an internal library boundary"                                                           | ditto                                             |
| `CLI-REQ-BINSTALL` | "copy the working `cargo-binstall` metadata pattern from `whitaker-installer` onto `whitaker`"                                   | ditto                                             |
| `CLI-REQ-LS`       | "`whitaker-ls` disappears in favour of `whitaker ls`"                                                                            | `CLI-DESIGN` §Public CLI surface                  |
| `CLI-REQ-L10N`     | "Every human-facing string, including `--help` … should be localizable"; command names are never translated                      | `CLI-DESIGN` §Accessibility and localization      |
| `CLI-REQ-FWD`      | Derived from Constraint 2; no upstream text. Recorded as a deviation in `Decision log` D-2 and added to `CLI-DESIGN` in `EP-M4`. | this plan                                         |

_Table 2: Upstream requirements traced by this plan._

```plaintext
CLI-REQ-BIN      -> EP-M0, EP-M2 -> tests::behaviour_whitaker::help_lists_subcommands
CLI-REQ-LIB      -> EP-M1        -> installer/tests/ unchanged + EP-INV-PARITY
CLI-REQ-BINSTALL -> EP-M3        -> tests::behaviour_binstall::whitaker_package_metadata
CLI-REQ-LS       -> EP-M2        -> tests::behaviour_whitaker::ls_text_and_json
CLI-REQ-L10N     -> EP-M2        -> localized parse entry point; NoOpLocalizer
CLI-REQ-FWD      -> EP-M2        -> EP-INV-DISPATCH (Kani) + EP-LEM-DISPATCH (Verus)
```

Explicitly **not** discharged here, with owning roadmap items: `whitaker check`
(3.5.2); release-artefact publishing (3.5.3); rule codes and selector
precedence (3.6.1, 3.6.2); `whitaker.toml`, `dylint.toml` bridging and
`DYLINT_*` migration (3.6.3); `--locale`/`--colour`/`--progress` (3.6.4); the
`--build-only` → `--build-from-source` rename and `--offline` (3.7.1); bundle
manifests (3.7.3); `doctor` and failure recording (3.8.x); the full `ls`
surface (3.8.1, 3.8.2); the `whitaker-installer` deprecation shim and `list`
alias (3.9.1); wrapper-script and `--skip-wrapper` removal (3.9.3).

## Verification plan

The first draft proposed a Verus proof of release-asset-name injectivity and a
Kani harness proving that a routing function rejects two conflicting flag
pairs. Both were **vacuous** and have been cut. Nothing in this repository ever
inverts an asset name — `cargo-binstall` composes by literal substitution, and
a grep for `rsplit_once|splitn|strip_prefix` across `installer/src/` finds
nothing — so injectivity of a never-inverted function is load-bearing for
nothing. And both conflict pairs are enforced by clap attributes
(`installer/src/cli.rs:108`, `:113`, `:131`), which reject the input during
parsing, so a routing function can never receive one; proving it rejects them
proves a property of an unreachable branch. Recording this is required by the
ExecPlan discipline: a passing check that cannot fail is not evidence.

What replaced them is a genuinely new obligation. The forwarding behaviour
(`CLI-REQ-FWD`) introduces a decision procedure that did not previously exist
and that no third-party library owns: given argv, dispatch a subcommand or
forward to `cargo dylint`. Getting it wrong silently breaks either the new CLI
or every existing user. That is worth verifying properly.

### Axioms (assumed, not verified)

- `clap` 4.5 parses argv per its documented derive semantics.
- `ortho_config` 0.10.0's `LocalizedParse::try_parse_localized_from` and
  `is_display_request` behave as documented. Repository-owned logic built on
  them **is** verified, against the real interface.
- `cargo-binstall` resolves `pkg-url` by literal substitution of `{name}`,
  `{target}`, `{version}`, `{archive-format}`.
- The wrapper script's contract is `exec cargo dylint "$@"` with
  `DYLINT_LIBRARY_PATH` set (`installer/src/wrapper.rs:107-113`).
- Kani sequentializes concurrency; no obligation here concerns concurrency.

### `EP-INV-DISPATCH` — argv classification is total and disjoint (bounded)

- **Obligation.** For every argv, `classify(argv)` returns exactly one of
  `Subcommand(_)`, `Forward`, or `Display`. No argv whose first non-global
  token names a Whitaker subcommand is ever classified `Forward`, and no argv
  the wrapper script would have accepted is classified as a usage error.
- **Method.** Bounded model check (Kani), plus parameterized `rstest` cases.
- **Rationale.** Totality and disjointness over a combinatorial token space is
  what bounded exhaustive exploration is for, and `classify` is small and pure.
- **Domain.** argv modelled as a bounded sequence of _token tags_, not
  strings: an enum over
  `{Install, Ls, Help, Version, DoubleDash, GlobalFlag, DylintFlag, Other}`,
  length bounded at 6 — 299,593 distinct token sequences across lengths 0–6
  (`sum(8^k for k in 0..=6)`). `#[kani::unwind(7)]`, one greater than the
  maximum iteration count.
- **Siting.** `classify` and its token enum live in `src/cli/dispatch.rs` in
  the root package and take the token enum rather than `clap::Cli`, so the
  harness compiles a small pure module. This is deliberate: siting Kani where
  it must codegen `clap` plus `ortho_config` plus `whitaker_installer` would be
  a roughly 250-crate graph, and a symbolic `Vec<String>` hits the documented
  heap cliff. Tokenizing real argv is a separate, testable function that is
  _not_ part of the harness.
- **Artefact.** `src/cli/dispatch.rs`, `#[cfg(kani)] mod verification`.
  Register a new `whitaker-cli` group in `scripts/run-kani.sh`, adding both a
  group function and a `case` arm. **Note the `*)` fallback at
  `scripts/run-kani.sh:97` routes unrecognized arguments into the
  _decomposition_ group**, so a mistyped filter silently verifies the wrong
  package and reports success. Add the arm before the fallback and confirm the
  harness names appear in the output.
- **Evidence.** `make kani 2>&1 | tee /tmp/kani-whitaker-3-5-1.out`. Expect
  `VERIFICATION:- SUCCESSFUL` for `verify_classify_is_total_and_disjoint`.
- **Non-vacuity.** Three separate reachability harnesses each assert `false`
  under a constraint selecting one outcome; each **must report a
  counterexample**, proving that outcome reachable. A harness that verifies
  successfully here is a failure. Negative control: remove `Ls` from the
  subcommand table; `verify_classify_is_total_and_disjoint` must fail with a
  concrete argv that names `ls` yet classifies as `Forward`. Restore and record
  the transcript.

### `EP-LEM-DISPATCH` — classification is total for unbounded argv

- **Obligation.** For every finite token sequence of _any_ length, exactly one
  classification applies. Kani bounds length at 6; this proof covers the
  unbounded tail that the bounded check cannot reach.
- **Method.** Formal proof (Verus) over `Seq<TokenTag>`.
- **Rationale.** The guarantee must hold for all admissible inputs — a user's
  `cargo dylint` invocation has no length bound. Bounded checking cannot
  establish it. The proof is a genuine mutual-exclusivity and exhaustiveness
  argument over the guard predicates, not a restatement.
- **Domain.** Unbounded `Seq<TokenTag>`. This would be the repository's first
  sequence-shaped proof; every file in `verus/` today is numeric or
  vector-algebraic. Budget accordingly and respect the verification tolerance.
- **Artefact.** `verus/whitaker_cli_dispatch.rs`, in the `cli` group and the
  default `all` group in `scripts/run-verus.sh`.
- **Evidence.** `make verus 2>&1 | tee /tmp/verus-whitaker-3-5-1.out`. Expect
  `verification results:: N verified, 0 errors`.
- **Non-vacuity.** No `assume` in the final proof. A witness lemma must
  exhibit an inhabiting sequence for each of the three classes _before_ the
  disjointness theorem is stated, or the theorem is vacuously true over an
  empty domain. Negative control: widen one guard so two overlap; the
  disjointness proof must fail. Record that transcript before restoring.

### `EP-INV-PARITY` — install-argument parity

- **Obligation.** For every argv `v` that `whitaker-installer` accepts as an
  install invocation, `whitaker install v` produces an equal `InstallRequest`;
  and every `v` the installer rejects, `whitaker install v` rejects with the
  same exit code.
- **Method.** Differential property test (`proptest`).
- **Rationale.** This is the precise formal content of "move the behaviour
  without changing it". Fourteen options with two conflict pairs is far too
  large to enumerate and too string-shaped for a model checker.
- **Domain.** Generated argv over the 14 options declared across `InstallArgs`
  (`installer/src/cli.rs:77-123`) and its three `#[command(flatten)]` groups
  `LintSelectionFlags` (`:129`), `ExecutionFlags` (`:143`), `SkipFlags`
  (`:157`). **Must include** short forms (`-t`, `-l`, `-j`, `-v`, `-q`), the
  long alias `--verbosity` (`:107`), repeated `--lint`, repeated `-v`, both
  conflict pairs, paths with spaces and non-ASCII characters, and the explicit
  `whitaker-installer install …` form as well as the bare form.
- **Artefact.** `tests/property_arg_parity.rs` in the root package.
- **Evidence.** `cargo nextest run -p whitaker property_arg_parity`. Red: a
  deliberately incomplete mapping that drops `--jobs` must fail naming it.
- **Non-vacuity.** Classify on **pairs**, not single flags: 2^14 is 16,384
  subsets, so marginal per-flag coverage says nothing about the combinations
  where the conflicts live. Require every unordered pair of options to co-occur
  in at least one case, and every option to appear set in at least 5% of cases.
  Run 4,096 cases — clap parsing is tens of microseconds, so this costs well
  under a second. A run where any pair is never exercised is a **failure**.
  Negative control: drop `--no-update` from the mapping; the test must fail
  naming it.

### `EP-INV-EXIT` — exit-code policy

- **Obligation.** Exit `0` for success and for a clap display request
  (`--help`, `--version`); **`2` for an argument-parsing or usage error**; `1`
  for an operational failure. Forwarded invocations propagate `cargo dylint`'s
  exit code unchanged.
- **Method.** Parameterized `rstest` over the finite partition, plus
  end-to-end assertions on the real spawned process.
- **Rationale.** A small finite partition is exactly what parameterized tests
  are for. The end-to-end case is what makes it non-vacuous: the unit mapping
  can be right while `main` discards it.
- **Ground truth, not a free choice.** `installer/src/main.rs:42` calls
  `Cli::parse()`, whose `Error::exit()` terminates with clap's code — **2** —
  never reaching `exit_code_for_run_result` at `:391-398`, the only source of
  `1`. So `whitaker-installer --lint x --individual-lints` exits 2 today, and
  Constraint 1 plus `EP-INV-PARITY` require `whitaker install` to match. The
  localized-parse arm must therefore use clap's own code, not a blanket failure
  code.
- **Domain.** `Ok(())`; each variant class of `InstallerError`; clap
  `DisplayHelp` and `DisplayVersion`; a genuine usage error; a forwarded
  invocation returning a non-zero code.
- **Artefact.** `src/cli/exit.rs` unit tests and `tests/e2e_exit_codes.rs`,
  **both in the root package** — because `env!("CARGO_BIN_EXE_whitaker")` is
  defined only for integration tests of the package declaring the binary. This
  is why `EP-M0` must remove `--exclude whitaker` from `TEST_EXCLUDES`; the
  end-to-end obligation is otherwise unrunnable.
- **Evidence.** `cargo nextest run -p whitaker exit`.
- **Non-vacuity.** The end-to-end test spawns the real binary and reads
  `ExitStatus::code()`. Negative control: drop the `is_display_request` branch;
  `whitaker --help` must then exit non-zero and the test must fail.

### Not verified, deliberately

- Internals of `clap`, `ortho_config`, `figment`, or `cargo-binstall`.
- The installer's existing behaviour beyond parity — already covered by
  `installer/tests/`.
- Localization catalogue content: `EP-M2` wires `NoOpLocalizer`, so there is
  no translation logic yet. Catalogues arrive with 3.6.4.

## Plan of work

### Stage A — extract the driver library (`EP-M0`)

This is the precondition, not a spike. Do it first and completely.

1. Create `crates/whitaker_lint_core` and move `src/config.rs`, `src/hir/`,
   `src/lints/`, `src/testing/`, the `dylint-driver` feature, and the root
   `tests/` files that exercise them (`build_config.rs`, `config_loading.rs`,
   `lint_template.rs`, `locale_resolution.rs`, `nextest_ui_filter.rs`,
   `ui_harness.rs`, plus `tests/features/` and `tests/support/`).
2. Repoint the eleven lint-crate manifests and `suite/`. Do not miss the
   literal path dependency at `crates/test_must_not_have_example/Cargo.toml:34`.
3. Strip the root `[package]` to a CLI package and add what crates.io
   requires: `description`, `license.workspace = true`,
   `repository.workspace = true`, `homepage.workspace = true`,
   `documentation.workspace = true`. Remove the now-unused optional `rustc_*`
   dependencies and the `dylint-driver` feature. **Also remove the unused
   `whitaker-installer` dependency at `Cargo.toml:71`** — grep confirms zero
   `whitaker_installer::` references under `src/` or `tests/` today — and
   re-add it deliberately in `EP-M1`, so the before-and-after dependency
   measurement is honest.
4. Remove `--exclude whitaker` from `TEST_EXCLUDES` (`Makefile:23`) **and**
   decide `DOCTEST_EXCLUDES` (`Makefile:64-68`) explicitly. Run the six newly
   included test binaries. Fix what they surface; if any fails for a
   pre-existing reason unrelated to this plan, record it in
   `Surprises & discoveries` and escalate rather than papering over it.
5. Add `-p whitaker` to `WHITAKER_PACKAGES` (`Makefile:81`) so the project's
   own lint suite covers the new code. Check `dylint.toml:35-52`
   `excluded_crates`: it currently names `whitaker`, and the meaning of that
   entry changes once the package changes character. Under Constraint 6, any
   new entry needs escalation.

**Gate — the whole point of the milestone:**

```console
cargo package -p whitaker --no-verify
```

must succeed. Then
`make check-fmt && make typecheck && make lint && make test NEXTEST_PROFILE=ci`,
and the Dylint UI suite must be unchanged.

Then answer the link question the first draft got wrong. `cargo check` does
**not** invoke the linker, so it cannot observe duplicate `std`/`core` symbols.
Use:

```console
cargo build --workspace --bins --all-features
cargo test -p whitaker --all-features --no-run
```

under the same
`RUSTFLAGS="-C prefer-dynamic -Z force-unstable-if-unmarked -D warnings"` the
test recipe uses. After the extraction the root package no longer enables
`rustc_private` at all, so this should be clean; if it is not, stop and
escalate.

### Stage B — red tests and feature specifications

No production behaviour yet.

1. `tests/features/whitaker_cli.feature` — reproduced in full under
   `Artefacts and notes`.
2. `tests/behaviour_whitaker.rs` with a `CliWorld` fixture modelled on
   `installer/tests/behaviour_cli/support.rs`. Named to avoid R4.
3. `tests/e2e_exit_codes.rs` (`EP-INV-EXIT`).
4. `tests/property_arg_parity.rs` (`EP-INV-PARITY`).
5. `src/cli/dispatch.rs` with the token enum, `classify`, and the
   `#[cfg(kani)] mod verification` harnesses including the three reachability
   harnesses (`EP-INV-DISPATCH`).
6. `verus/whitaker_cli_dispatch.rs`, witness lemmas first
   (`EP-LEM-DISPATCH`).

Every one must **fail**, for the expected reason. Record each red transcript.

### Stage C — implementation

1. **Promote the installer orchestration.** Add `pub mod install_flow;` to
   `installer/src/lib.rs`, move `installer/src/install_flow/` into the library,
   and move the ten `main.rs` orchestration functions into a new
   `installer/src/orchestration/` tree, each file under 400 lines.
   `installer/src/main.rs` becomes a thin composition root.
   `try_fast_path_installation` calls
   `staged_suite::try_test_staged_suite_installation`
   (`installer/src/main.rs:87`), so the fast path and the staged-suite hook
   move together; keep the hook behind its existing `#[cfg(debug_assertions)]`
   and `test_support` gating and expose only the facade the fast path needs.
   `installer/src/main.rs` also carries `#[cfg(test)] mod tests;` at `:402`;
   those unit tests move with their subjects. That is relocation, not an
   assertion change, and does not trip the behaviour-drift tolerance. **No
   behaviour changes.** Validate with `make test NEXTEST_PROFILE=ci` — not plain
   `make test`, which skips `behaviour_cli` and `behaviour_toolchain`, the
   very binaries that would detect a dispatch regression.

2. **Wrapper generation becomes a parameter.** Give
   `generate_and_report_wrapper` an argument naming which scripts to write.
   `whitaker-installer` passes `WrapperScripts::Both`, preserving Constraint 1
   exactly. `whitaker install` passes `WrapperScripts::None`: it must not
   overwrite the binary the user just invoked, and it cannot write the lister
   either, because the generated `whitaker-ls` shells out to
   `whitaker-installer run`, which a root install has no reason to assume is on
   `PATH`. The root CLI's own `whitaker ls` covers the listing instead. (The
   plan originally proposed `{whitaker-ls}` here; review showed the lister's
   dependency on a separately installed installer, so the policy became "write
   nothing".)

3. **Build the CLI in the root package.** `src/cli/` holds the clap types;
   `src/cli/dispatch.rs` the classifier; `src/cli/exit.rs` the exit policy;
   `src/adapters/` the driven adapters over `whitaker_installer`. The root
   `Cli` carries `command: Command` (not `Option`) plus the shared `-q`/`-v`
   flags that `CLI-DESIGN` declares common options; `whitaker install`
   **re-exports** `whitaker_installer::cli::InstallArgs` rather than mirroring
   it field for field, so parity is true by construction and `EP-INV-PARITY`
   guards the request mapping rather than struct shape.

4. **Argument forwarding.** When `classify` returns `Forward`, resolve the
   staged library directory the way the wrapper script does, set
   `DYLINT_LIBRARY_PATH`, and run `cargo dylint` with the original arguments,
   propagating its exit code. Keep this in one small adapter behind
   `LintRunner`.

5. **Composition root** at `src/main.rs`: construct adapters, parse via
   `ortho_config`, dispatch, map to an exit code. Target under 60 lines. Declare
   `[[bin]] name = "whitaker"` with `autobins = false`, matching
   `installer/Cargo.toml:11-27`.

6. **binstall metadata, additively.** Do **not** change the signatures of
   `expand_pkg_url` or `expand_bin_dir`
   (`installer/src/binstall_metadata.rs:45`, `:73`) — they are consumed by
   `installer/tests/behaviour_installer_release.rs:155`,
   `installer/tests/behaviour_binstall.rs:10-18`,
   `installer/src/installer_packaging_tests.rs:274`, and their own doctests, so
   narrowing them trips the interface tolerance. Add
   `expand_pkg_url_for(package, version, target)` and
   `expand_bin_dir_for(package, version, target)`, and reimplement the existing
   two-argument forms as delegates. `load_cargo_toml` resolves the manifest
   through `env!("CARGO_MANIFEST_DIR")`, baked to `installer/` at compile time,
   so give it a `&Path` parameter for the root package's test to use. Then add
   `[package.metadata.binstall]` to the root `Cargo.toml` mirroring
   `installer/Cargo.toml:95-101`, including the
   `overrides.x86_64-pc-windows-msvc` entry with `pkg-fmt = "zip"`.
   `installer/src/installer_packaging.rs:31` has the same hardcoding and is
   deliberately **not** touched; it belongs to 3.5.3.

Validate after each step:
`make check-fmt && make typecheck && make lint && make test NEXTEST_PROFILE=ci`,
captured with `tee`. Commit after each.

### Stage D — verification, reservation, documentation

1. Turn the Kani harnesses green; confirm the three reachability harnesses
   each report a counterexample; run the negative control and record it.
2. Turn the Verus proof green; run the negative control and record it.
3. Run the proptest pairwise classification report; confirm every pair is
   exercised.
4. Reserve the crates.io name (`EP-M3`) **before** any documentation tells
   users to install it.
5. Write `docs/adr-011-whitaker-cli-boundary.md` using the template at
   `docs/documentation-style-guide.md:414`, with the content required by
   `EP-M4`.
6. Update `docs/users-guide.md`, `docs/developers-guide.md`,
   `docs/whitaker-cli-design.md`, `docs/whitaker-dylint-suite-design.md`,
   `docs/publishing.md`, `docs/repository-layout.md`, and `docs/roadmap.md`.
7. `make markdownlint` and `make nixie`.

## Milestones and plateaus

### `EP-M0` — root package is a publishable, testable CLI package

- **Outcome.** The Dylint driver library lives in `crates/whitaker_lint_core`.
  The root `whitaker` package has no `rustc_private`, carries crates.io
  metadata, is covered by `make test`, and
  `cargo package -p whitaker --no-verify` succeeds. No binary yet.
- **Requirements.** Precondition for `CLI-REQ-BIN`.
- **Acceptance evidence.** `cargo package -p whitaker --no-verify` exits 0;
  the Dylint UI suite is unchanged; six previously-orphaned test binaries now
  run under `make test`; `cargo build --workspace --bins --all-features` links
  cleanly.
- **Conformance check.** Lint crates build and lint unchanged; no public
  behaviour changed; `dylint.toml` reviewed.
- **Recovery.** One commit; `git revert` is clean.
- **Remaining gaps.** No binary, no CLI.
- **Compatibility decision.** None. `whitaker_lint_core` is a new
  application-internal crate; the root library's consumers are all in-tree and
  updated in the same change. The root `whitaker` library was never published,
  so nothing external depends on its shape.

### `EP-M1` — installer orchestration behind the library boundary

- **Outcome.** `installer/src/main.rs` is a thin composition root; all
  orchestration is in the `whitaker_installer` library; `whitaker-installer`
  behaves identically.
- **Requirements.** `CLI-REQ-LIB`.
- **Acceptance evidence.** `make test NEXTEST_PROFILE=ci` passes with every
  existing `installer/tests/` test **unmodified**. Additionally, capture
  `whitaker-installer --dry-run` output as an `insta` snapshot _before_ the
  move and assert it after — a five-minute change that converts a vague parity
  claim into a hard oracle.
- **Conformance check.** Public surface grew, never shrank; prebuilt path
  untouched.
- **Recovery.** Revert the milestone's commits.
- **Remaining gaps.** No `whitaker` binary yet.
- **Compatibility decision.** None. Pre-1.0, application-internal; callers
  updated in the same change.

### `EP-M2` — the `whitaker` binary

- **Outcome.** `whitaker --help`, `whitaker install`, `whitaker ls`,
  `whitaker ls --json`, and `whitaker --all -- …` all work.
- **Requirements.** `CLI-REQ-BIN`, `CLI-REQ-LS`, `CLI-REQ-L10N`,
  `CLI-REQ-FWD`; `EP-INV-DISPATCH`, `EP-LEM-DISPATCH`, `EP-INV-PARITY`,
  `EP-INV-EXIT` all discharged.
- **Acceptance evidence.** The BDD scenarios pass under `NEXTEST_PROFILE=ci`;
  `whitaker --help` exits 0; `whitaker --all --version` reaches `cargo dylint`;
  `whitaker install` does not write a `whitaker` script.
- **Conformance check.** Command names untranslated; `--json` on `ls` only,
  not global, as `CLI-DESIGN` requires.
- **Recovery.** The binary is additive in the Cargo graph — but **not** in the
  `PATH` namespace. Reverting removes the binary; a user who has already
  installed it must `cargo uninstall whitaker` to restore the script's
  precedence. Say so in the ADR.
- **Remaining gaps.** `check` and `doctor` absent by design. `ls` output is
  the installer's current shape and will be replaced wholesale by 3.8.1 and
  3.8.2 once bundle manifests (3.7.3) exist — it is **not** a stable contract.
- **Compatibility decision.** Two, both with named consumers. (i)
  `whitaker-installer` remains, per Constraint 1 — consumers: users following
  `docs/users-guide.md`, published release assets; removal is 3.9.1. (ii)
  Argument forwarding, per Constraint 2 — consumers: users invoking
  `whitaker --all`, `Makefile:185`, `ci.yml:154`; removal is 3.9.3, once
  `whitaker check` (3.5.2) supersedes it.

### `EP-M3` — installable, and the name is ours

- **Outcome.** Root package carries binstall metadata; the crates.io name
  `whitaker` is reserved.
- **Requirements.** `CLI-REQ-BINSTALL`.
- **Acceptance evidence.** A behavioural test asserts the root package's
  binstall table matches the shared template constants and that its expanded
  `pkg-url` differs from `whitaker-installer`'s for **every** target in the
  release matrix (`release.yml:30-38`) — a finite disjointness check over the
  ten-element published namespace, which is what the cut Verus proof should
  have been. Record the stripped release binary size.
- **Conformance check.** ADR-001 still holds; no published asset changes.
- **Recovery.** Metadata-only. Name reservation is not reversible; that is the
  point.
- **Remaining gaps.** CI does not yet _publish_ a `whitaker` release artefact
  — roadmap 3.5.3. Until it does, `cargo binstall whitaker` will 404 and fall
  back to a source build, or to quickinstall for users who have it enabled.
  State this plainly in the ADR and the user guide; do not imply binstall works
  end to end.
- **Compatibility decision.** None.

### `EP-M4` — documented

- **Outcome.** ADR and documentation reflect reality.
- **Acceptance evidence.** `make markdownlint` and `make nixie` pass; roadmap
  item 3.5.1 ticked.
- **Required ADR content** (`docs/adr-011-whitaker-cli-boundary.md`): the
  `PATH` namespace decision and why forwarding was chosen over the
  alternatives; the promoted `whitaker_installer` public surface; the rule that
  `whitaker_installer` owns _how_ to install while the CLI owns _whether and
  when_, so 3.5.2's lazy repair has an unambiguous home; and the plain
  statement that no `whitaker` artefact is published until 3.5.3.
- **Conformance check.** Every `Surprises & discoveries` entry reconciled
  against `CLI-DESIGN`; `CLI-REQ-FWD` added to the design document, since it
  has no upstream source today.
- **Recovery.** Documentation-only.
- **Compatibility decision.** None.

## Interfaces and dependencies

### New dependencies

Add to `[workspace.dependencies]`, caret-pinned:

```toml
ortho_config = "0.10.0"
googletest = "0.14.3"
pretty_assertions = "1.4.1"
```

`ortho_config` is a normal dependency of the root package only. After `EP-M0`
the lint crates depend on `whitaker_lint_core`, not on `whitaker`, so this
subtree does not propagate into the fifty cross-compiled lint-crate builds in
`rolling-release.yml`. That containment is a direct benefit of `EP-M0` and
should be stated in the ADR.

`insta` is already a workspace dependency; add it as a root dev-dependency.

**`googletest` ordering rule.** `#[gtest]` must come **before** `#[rstest]`, or
the test registers and runs twice. Document in `docs/developers-guide.md`:

```rust
#[gtest]
#[rstest]
#[case::install(&["install", "--lint", "module_max_lines"])]
fn classifies_as_subcommand(#[case] argv: &[&str]) -> googletest::Result<()> {
    verify_that!(classify(&tokenize(argv)), matches_pattern!(Class::Subcommand(_)))
}
```

Also record how `verify_that!` interacts with the workspace's
`clippy::missing_assert_message` policy, and whether `googletest::Result<()>`
return types survive the `unwrap_used`/`expect_used` denials.

### Required signatures

`src/cli/dispatch.rs` — pure, the subject of both verification obligations:

```rust
/// How an argument vector should be handled.
pub enum Class {
    /// Dispatch to a Whitaker subcommand.
    Subcommand(Subcommand),
    /// Forward verbatim to `cargo dylint`.
    Forward,
    /// Print help or version and exit successfully.
    Display,
}

/// Classifies a tokenized argument vector.
#[must_use]
pub fn classify(tokens: &[TokenTag]) -> Class;
```

`src/cli/exit.rs` — pure, the subject of `EP-INV-EXIT`:

```rust
/// Maps an outcome onto a process exit code.
#[must_use]
pub const fn exit_code_for(outcome: &Outcome) -> u8;
```

`src/ports.rs`:

```rust
/// Performs an installation on behalf of the command layer.
pub trait InstallService {
    /// Runs an installation and reports what happened.
    ///
    /// # Errors
    ///
    /// Returns an error when the installation cannot complete.
    fn install(&self, request: &InstallRequest) -> Result<InstallReport, CliError>;
}

/// Reports the lints currently staged on this machine.
pub trait LintInventory {
    /// Lists staged lints in the given staging directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the staging directory cannot be scanned.
    fn list(&self, request: &ListRequest) -> Result<StagedInventory, CliError>;
}

/// Runs Dylint on behalf of a forwarded invocation.
pub trait LintRunner {
    /// Forwards arguments to `cargo dylint` and returns its exit code.
    ///
    /// # Errors
    ///
    /// Returns an error when the subprocess cannot be started.
    fn forward(&self, args: &[std::ffi::OsString]) -> Result<u8, CliError>;
}
```

`LintInventory` returns a `StagedInventory` struct rather than a bare `Vec`, so
3.7.3 and 3.8.1 can add manifest fields without replacing the trait.
`LintRunner` exists now because forwarding needs it, and 3.5.2's `check` will
reuse it.

### Localized parsing

```rust
use ortho_config::{LocalizedParse as _, NoOpLocalizer, is_display_request};

let localizer = NoOpLocalizer::new();
let cli = match Cli::try_parse_localized_from(std::env::args_os(), &localizer) {
    Ok(cli) => cli,
    Err(err) if is_display_request(&err) => { err.print()?; return Ok(0); }
    Err(err) => { err.print()?; return Ok(err.exit_code() as u8); }
};
```

Note `NoOpLocalizer::new()`, not a bare unit-struct reference, and
`err.exit_code()` rather than a blanket failure code — otherwise usage errors
exit 1 where the installer exits 2, breaking `EP-INV-PARITY`.

Bare `whitaker` never reaches this parse. `classify(&[])` returns
`Class::Forward`, so an empty argument vector runs `cargo dylint` with no
further arguments and exits with its status, as the legacy wrapper's
`exec cargo dylint "$@"` did. `is_display_request` matches only `DisplayHelp`
and `DisplayVersion`; that is sufficient because routing never hands Clap an
argument vector without a subcommand, help, or version token. The `empty` case
in `src/cli/dispatch.rs` and the bare-invocation assertion in
`tests/e2e_exit_codes.rs` hold that contract.

This establishes the seam that 3.6.4 fills with a `FluentLocalizer` and an
`en-GB` catalogue, without adopting a configuration model this plan does not
own.

## Concrete steps

Run everything from `$(git rev-parse --show-toplevel)`.

```console
git branch --show-current
```

```plaintext
3-5-1-root-whitaker-binary
```

The `EP-M0` gate, which decides whether the milestone succeeded:

```console
cargo package -p whitaker --no-verify
```

Focused iteration:

```console
cargo nextest run -p whitaker 2>&1 | tee /tmp/nextest-whitaker-3-5-1.out
```

The full gate, sequentially, before every commit:

```console
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-3-5-1.out
make typecheck 2>&1 | tee /tmp/typecheck-whitaker-3-5-1.out
make lint      2>&1 | tee /tmp/lint-whitaker-3-5-1.out
make test NEXTEST_PROFILE=ci 2>&1 | tee /tmp/test-whitaker-3-5-1.out
```

Verification:

```console
make kani  2>&1 | tee /tmp/kani-whitaker-3-5-1.out
make verus 2>&1 | tee /tmp/verus-whitaker-3-5-1.out
```

Smoke-test the binary:

```console
cargo run --bin whitaker -- --help
cargo run --bin whitaker -- ls --json
cargo run --bin whitaker -- --all -- -p whitaker-common --all-targets
```

Expected shape of the first:

```plaintext
Usage: whitaker <COMMAND>

Commands:
  install  Install or repair Whitaker dependencies and lint bundles
  ls       Show installed lints and bundle metadata
  help     Print this message or the help of the given subcommand(s)
```

Consider adding a `whitaker-smoke` Makefile target alongside `install-smoke`,
installing the root package into a temporary root _positioned to shadow_ a
generated wrapper script, and asserting that `--help`, `ls --json`, and `--all`
all behave. That is the only mechanism that would catch a forwarding regression
before users do.

## Validation and acceptance

### Red-Green-Refactor evidence to record

**Red.** Before Stage C production code:

```console
cargo nextest run -p whitaker 2>&1 | tail -20
```

The BDD scenarios and `e2e_exit_codes` must fail, naming a missing binary or a
wrong exit code — not a compile error in the test itself.

**Green.** Each Stage C step's focused command passes.

**Refactor.** After splitting any file approaching 400 lines, re-run the
focused command and then the full gate.

### Behaviour to observe

1. `whitaker --help` lists `install` and `ls`; `echo $?` prints `0`.
2. `whitaker ls --json` prints the same JSON as
   `whitaker-installer list --json`. Ordering is deterministic —
   `installer/src/list.rs:50` sorts by crate name and
   `InstalledLints.by_toolchain` is a `BTreeMap` — so the snapshot is stable.
   The snapshot is a **regression guard, not a compatibility promise**; note
   that inline.
3. `whitaker install --dry-run` prints what `whitaker-installer --dry-run`
   prints, and writes nothing to stdout — stdout carries command output only;
   diagnostics, progress, and errors go to stderr.
4. `whitaker install --lint module_max_lines --individual-lints` is rejected
   and `echo $?` prints `2`, matching `whitaker-installer`.
5. `whitaker --all -- -p whitaker-common --all-targets` runs the lint suite,
   exactly as the wrapper script does, and propagates its exit code.
6. `whitaker install` does not create or overwrite a `whitaker` script.
7. `whitaker-installer --help` is unchanged.

### Quality criteria

- **Tests.** `make test NEXTEST_PROFILE=ci` passes. Every existing
  `installer/tests/` test and every Dylint UI fixture passes **without
  assertion changes**.
- **Publishability.** `cargo package -p whitaker --no-verify` succeeds.
- **Verification.** `EP-INV-DISPATCH`, `EP-LEM-DISPATCH`, `EP-INV-PARITY`,
  `EP-INV-EXIT` discharged, each with its recorded negative-control failure
  transcript and, for the Kani reachability harnesses, its recorded
  counterexample. An obligation without a recorded negative control is not
  discharged.
- **Lint/typecheck.** `make check-fmt`, `make typecheck`, `make lint` pass
  with no new suppressions and no new `dylint.toml` exclusions.
- **Documentation.** `make markdownlint` and `make nixie` pass.
- **Cost.** Record
  `cargo tree -e normal -p module_max_lines --features dylint-driver | wc -l`
  before and after; after `EP-M0` it should _fall_. Record the stripped size of
  the `whitaker` binary.

## Idempotence and recovery

Every step is re-runnable. `EP-M0` is one large mechanical commit; keep it
self-contained so bisection is clean. Stage C steps are additive except the
relocations in step 1, which are verifiable by the unchanged test suite.

`make test` backs up and restores `~/.local/bin/whitaker` (`Makefile:99-139`)
and fails loudly if a test modifies it. If a run is interrupted, confirm that
file was restored before re-running.

Do not create an isolated Cargo cache; use the shared default and let the
package-cache lock serialize access.

## Artefacts and notes

### Feature specification (`tests/features/whitaker_cli.feature`)

New file only — never insert into an existing feature file (`Risk R3`).

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
    And stdout contains no diagnostic text

  Scenario: A dry-run install reports its configuration without building
    Given a Whitaker workspace checkout
    When I run whitaker with "install --dry-run"
    Then the command succeeds
    And no lint library is staged
    And stdout is empty

  Scenario: Conflicting lint selection flags are rejected
    Given the whitaker binary is available
    When I run whitaker with "install --lint module_max_lines --individual-lints"
    Then the command fails
    And the exit code is 2
    And the error names both conflicting options

  Scenario: Legacy lint invocations are forwarded to cargo dylint
    Given the whitaker binary is available
    When I run whitaker with "--all -- -p whitaker-common --all-targets"
    Then the invocation reaches cargo dylint
    And the exit code is the exit code cargo dylint returned

  Scenario: Installing does not overwrite the whitaker binary
    Given a Whitaker workspace checkout
    When I run whitaker with "install --dry-run"
    Then no wrapper script named "whitaker" is reported

  Scenario: The legacy installer binary is unaffected
    Given the whitaker-installer binary is available
    When I run whitaker-installer with "--help"
    Then the command succeeds
    And the output is unchanged from the recorded snapshot
```

### Verus proof skeleton (`verus/whitaker_cli_dispatch.rs`)

Witness lemmas first, so the disjointness theorem is not vacuous:

```rust
use vstd::prelude::*;

verus! {

/// Token kinds the classifier distinguishes.
pub enum TokenTag { Install, Ls, Help, Version, DoubleDash, GlobalFlag, DylintFlag, Other }

pub open spec fn is_subcommand(tokens: Seq<TokenTag>) -> bool;
pub open spec fn is_display(tokens: Seq<TokenTag>) -> bool;
pub open spec fn is_forward(tokens: Seq<TokenTag>) -> bool;

/// Each class is inhabited, so disjointness is not vacuously true.
proof fn lemma_classes_are_inhabited()
    ensures
        exists|t: Seq<TokenTag>| is_subcommand(t),
        exists|t: Seq<TokenTag>| is_display(t),
        exists|t: Seq<TokenTag>| is_forward(t),
{ /* witnesses: [Install], [Help], [DylintFlag] */ }

/// Exactly one class applies to any sequence, of any length.
proof fn lemma_classification_is_total_and_disjoint(tokens: Seq<TokenTag>)
    ensures
        is_subcommand(tokens) || is_display(tokens) || is_forward(tokens),
        !(is_subcommand(tokens) && is_forward(tokens)),
        !(is_display(tokens) && is_forward(tokens)),
        !(is_subcommand(tokens) && is_display(tokens)),
    decreases tokens.len(),
{ /* induction on the leading token */ }

} // verus!
```

No `assume` in the final version. Per `docs/developers-guide.md`, Verus proofs
here model the implementation rather than the literal Rust source; the Kani
harness in `EP-INV-DISPATCH` is what checks the real code against the same
property within bounds.

## Signposts

| Document                                                     | Why                                                                                        |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| `docs/whitaker-cli-design.md`                                | The specification. §Public CLI surface and §Compatibility and migration are normative.     |
| `docs/roadmap.md`                                            | Items 3.5.1 to 3.9.3 — what belongs here and what does not.                                |
| `docs/users-guide.md`                                        | The `whitaker --all` contract this plan must preserve; updated in `EP-M4`.                 |
| `docs/developers-guide.md`                                   | Installer architecture, Kani harness conventions, the Verus trust boundary.                |
| `docs/publishing.md`                                         | Publish order, updated in `EP-M4` once the root package is publishable.                    |
| `docs/repository-layout.md`                                  | Updated for `crates/whitaker_lint_core`.                                                   |
| `docs/ortho-config-users-guide.md`                           | Layering, subcommand merging, the localization API.                                        |
| `docs/rstest-bdd-users-guide.md`                             | `#[scenario]` bindings and step functions.                                                 |
| `docs/rust-testing-with-rstest-fixtures.md`                  | Fixture patterns for the `CliWorld` fixture.                                               |
| `docs/rust-doctest-dry-guide.md`                             | Doctests are gated; keep them DRY.                                                         |
| `docs/complexity-antipatterns-and-refactoring-strategies.md` | The suite lints this repository against itself; keep `classify` flat.                      |
| `docs/whitaker-dylint-suite-design.md`                       | Workspace layout, updated in `EP-M4`.                                                      |
| `docs/whitaker-clone-detector-design.md`                     | Confirms `whitaker_clones_core`/`whitaker_sarif` naming so new crate names do not collide. |
| `docs/documentation-style-guide.md`                          | ADR template and naming; Oxford spelling.                                                  |
| `docs/adr-001-prebuilt-dylint-libraries.md`                  | The prebuilt path this plan must not disturb.                                              |
| `AGENTS.md`                                                  | Gates, commit rules, the 400-line limit, test-environment rules.                           |

_Table 3: Documentation to read before starting._

Skills: `leta` for symbol navigation instead of grep; `hexagonal-architecture`
for the port boundaries; `kani` for `EP-INV-DISPATCH`; `verus` for
`EP-LEM-DISPATCH`; `proptest` for `EP-INV-PARITY`; `rust-unit-testing` for
`googletest` and `insta` style; `execplans` for keeping this document current.

## Surprises & discoveries

- Observation: the checked baseline reproduces the plan's packaging failure.
  Evidence: `cargo package -p whitaker --no-verify --allow-dirty` exited
  non-zero on 2026-08-24 with `no matching package named rustc_ast`; the
  baseline normal dependency tree for `module_max_lines` has 342 entries.
  Impact: confirms that `EP-M0` remains the required first milestone and gives
  the before-measurement for its dependency-containment objective.

- Observation: Cargo's workspace-wide `--all-features` unifies the core
  crate's `dylint-driver` feature into its integration-test executables.
  Evidence: `lint_template` links without the feature but fails with it,
  reproducing the `rustc_driver` duplicate `std`/`core` diagnostics; the two
  focused commands and logs are recorded in
  `docs/debugging/debugging-plan-2026-08-24-m0-core-test-linkage.md`. Impact:
  the all-feature workspace pass excludes only the compiler-private core test
  target, then `make test` and `publish-check` run its tests and doctests
  separately without that feature. All six formerly excluded test targets
  remain covered.

- Observation: the formerly excluded `build_config` test asserted dynamic
  linker flags in `.cargo/config.toml`, although that file deliberately keeps
  them out of workspace configuration and the Make recipes inject them only for
  Dylint-aware commands. Evidence: the current configuration comment states
  this policy and the test failed because `build.rustflags` is absent. Impact:
  the test now protects the actual scoped-flag contract; this is a pre-existing
  assertion correction exposed by `EP-M0`, not a behaviour change.

- Observation: the moved template behaviour tests still asserted the old root
  `whitaker` dev-dependency and root-relative path. Evidence: three scenarios
  failed after the extraction despite the template correctly rendering
  `whitaker_lint_core = { path = "../whitaker_lint_core" }`. Impact:
  expectations now validate the new internal boundary; no generated template
  behaviour changed after the relocation.

- Observation: the root `whitaker` package cannot be published at all.
  Evidence: `cargo package -p whitaker --no-verify --allow-dirty` fails with
  "no matching package named `rustc_ast`"; the four `rustc_*` shim crates are
  `publish = false`; the manifest has no `description`, `license`, or
  `repository`. Impact: decisive. Turned the driver-library extraction from a
  contingency into `EP-M0`, the plan's precondition, and made a separate CLI
  crate unnecessary.

- Observation: the installer writes an executable named `whitaker` into the
  user's binary directory, and a `cargo install`ed binary deterministically
  shadows it. Evidence: `installer/src/wrapper.rs:105`; `Makefile:9` prepends
  `~/.cargo/bin` while `Makefile:6` appends `~/.local/bin`; `Makefile:99-139`
  backs the script up around every test run; `Makefile:185` and `ci.yml:154`
  invoke `whitaker --all`. Impact: without forwarding, this change would break
  this repository's own lint gate and every user's primary workflow, with no
  replacement until 3.5.2. Drove `CLI-REQ-FWD` and Constraint 2.

- Observation: Clap's bare `help` subcommand is not equivalent to `--help` in
  the application's routing classifier. Evidence: dispatch selects forwarding
  before Clap parses arguments, and `token_tag` previously tagged only `-h` and
  `--help`. Impact: `whitaker help` reached `cargo dylint` instead of printing
  Whitaker help. The classifier now recognizes bare `help`, with an end-to-end
  regression in `tests/e2e_exit_codes.rs`.

- Observation: `env!("CARGO_BIN_EXE_<name>")` is defined only for integration
  tests of the package declaring the binary. Evidence: all four existing uses
  are package-local, for example
  `installer/tests/behaviour_cli/support.rs:206`. Impact: end-to-end tests must
  live in the root package, which is why removing `--exclude whitaker` is a
  required part of `EP-M0` rather than an optional tidy-up.

- Observation: `cargo check` does not invoke the linker.
  Evidence: the risk being tested is a _link_ error, per `src/lib.rs:4-6`.
  Impact: the first draft's feasibility spike used `cargo check` and so could
  never observe the failure it existed to detect. Replaced with
  `cargo build --bins` and `cargo test --no-run`.

- Observation: both documented flag conflicts are enforced by clap, not by
  repository-owned code. Evidence: `conflicts_with` at
  `installer/src/cli.rs:108`, `:113`, `:131`. Impact: a routing function can
  never receive a conflicting input, so the first draft's Kani obligation was
  vacuous. Cut.

- Observation: nothing in the repository ever parses a release-asset name.
  Evidence: `installer/src/binstall_metadata.rs` composes with `.replace()`; no
  `rsplit_once`, `splitn`, or `strip_prefix` under `installer/src/`. Impact:
  the first draft's Verus injectivity proof was load-bearing for nothing. Cut
  and replaced with a finite disjointness test over the ten-element published
  namespace.

- Observation: nextest's `binary()` filter matches by binary name, not
  package-qualified name. Evidence: `.config/nextest.toml` `default-filter`
  excludes `binary(behaviour_cli)`. Impact: a new `behaviour_cli.rs` in any
  package would be silently skipped. Drove the `behaviour_whitaker.rs` naming
  and the `NEXTEST_PROFILE=ci` discipline.

- Observation: `#[gtest]` must precede `#[rstest]` or the test runs twice.
  Evidence: `googletest` 0.14.3 documentation. Impact: recorded as a convention
  for `docs/developers-guide.md`.

- Observation: the post-rebase repository test gate is still blocked by the
  managed compiler wrapper and shared Cargo lock contention. Evidence: the
  2026-10-15 `make test` run completed 1,633 of 1,653 tests, with five failures
  and one timeout. Dylint 6.0.4 reported `Found no rustc invocations` for UI
  fixtures while using the managed `build-limits-rustc` wrapper; another UI
  build timed out after waiting for shared Cargo locks. See
  `/tmp/test-0c485c79-c21a-486e-b126-29c3ef23084f-3-5-1-root-whitaker-binary.out`.
  Impact: formatting, typecheck, and lint passed, but the full test gate is
  not green. The failure does not justify bypassing compiler admission or
  hiding affected tests. No code changes were made during this validation.

- Observation: `ortho_config` 0.9.0 exposes localized parsing and display
  request detection with the signatures assumed by the M2 sketch. Evidence:
  Firecrawl retrieved the pinned
  [`LocalizedParse` API](https://docs.rs/ortho_config/0.9.0/ortho_config/trait.LocalizedParse.html)
  and
  [`is_display_request`](https://docs.rs/ortho_config/0.9.0/ortho_config/fn.is_display_request.html)
  on 2026-10-15. `try_parse_localized_from<I, T>(iter, &dyn Localizer)` returns
  `Result<Self, clap::Error>` and `is_display_request(&clap::Error)` returns
  `bool`. Impact: the planned error guard can be implemented and tested against
  the library's real signature; no API adjustment is needed.

- Observation: `ortho_config` 0.10.0 leaves the parsing surface unchanged.
  Evidence: on 2026-10-25, `src/error/helpers.rs` and
  `src/localizer/clap_command/parse.rs` were byte-identical between the 0.9.0
  and 0.10.0 registry sources, and `NoOpLocalizer::new()` remains. The upgrade
  adds `ortho_l10n` and moves `dirs` from 5 to 7, which drops `dirs-sys` and
  nine `windows-*` 0.48 crates from `Cargo.lock`. Impact: D-23 bumps the pin
  with no call-site change, and the lockfile shrinks.

- Observation: Kani 0.67.0 omits the literal `Counterexample:` marker and its
  Ls negative-control report omits the satisfying model. The runner checks
  `VERIFICATION:- FAILED`, the named check, and the successful witness
  assertion. All reachability controls pass; the positive proof verified 360
  checks. The Ls control fails check 64/361, with `[Ls]` derived from the
  harness, not quoted as verifier output. Source hashes match after mutation;
  runner checks pass `bash -n` and ShellCheck.

- Observation: the initial Verus proof ran CPU-bound for 650 seconds without
  a verdict. On 2026-10-19, plain classification equalities replacing symbolic
  `by (compute)` assertions made the focused proof pass in one second. Pinned
  Verus `0.2026.03.17.a96bad0` guide files
  `source/docs/guide/src/assert_by_compute.md` and
  `source/docs/guide/src/proof_functions.md` explain that compute assertions
  reason over isolated expressions and symbolic expressions can leave work to
  SMT. This is context, not a confirmed root cause.
- Observation:
  `timeout --signal=TERM --kill-after=15s 600s ./scripts/run-verus.sh cli`
  passed 4/0 in 1 second (Verus `0.2026.03.17.a96bad0`, toolchain
  `1.94.0-x86_64-unknown-linux-gnu`). A scratch negative control widened the
  lemma guard to overlap `Help`; the expected assertion failed (3 verified, 1
  error) in under a second. Logs:
  `/tmp/verus-3-5-1-root-whitaker-binary-reviewrepair.out`;
  `/tmp/whitaker-cli-verus-negative-control.out`.

- Observation: fresh-home CI exposed `workspace_root()` selecting the
  manifest's parent. It now uses the package root.

- Observation: a timed-out Dylint example left its cross-process lock behind.
  Evidence: Nextest killed the first `collection_zero_diagnostic` attempt at
  ten minutes; recovery then waited for a thirty-minute mtime threshold, so two
  retries timed out on the abandoned lock. Its advisory liveness lock was free.
  Impact: reclaim immediately when that lock is free, while preserving active
  owners; regression tests cover both states.

- Observation: a later, more specific nextest override does not win. For a
  given test and setting, nextest applies the first override in file order that
  matches the test _and_ configures that setting, evaluated separately per
  setting. Evidence: the default profile's broad 20-minute UI override also
  named `trybuild_fixtures_compile_without_diagnostics`, so the test was
  terminated at 1,200 s on all three attempts while its own 120-minute override
  sat later in the file; the `ci` profile passed because its broad filter never
  named the test. Impact: an allowance only takes effect when every earlier
  filter for the same setting omits the test. Both long
  `rstest_helper_should_be_fixture` cases are now absent from the 20-minute
  filter, and the rule is recorded in the developers' guide beside the timeout
  budgets.

- Observation: Weave was selected for path `*.rs` by the user-wide attributes
  file, not by tracked repository policy. `git check-attr merge -- <path>`
  reported `weave`, `git config --get core.attributesFile` printed nothing (so
  the default `$XDG_CONFIG_HOME/git/attributes` applied), no attribute file
  existed under `.git/info`, and the tracked `.gitattributes` never mentions
  `merge`. The user-wide file carries an ansible-managed Weave enrolment block.
  Because an unattended merge across a long-lived branch is not repository
  consent, the merge ran under
  `git -c core.attributesFile=/dev/null -c merge.conflictStyle=zdiff3`, and
  `git check-attr` was re-run under the same override to confirm `unspecified`
  before starting.

## Decision log

- **D-1: Extract the Dylint driver library into `crates/whitaker_lint_core` as
  `EP-M0`, and put the CLI in the root package rather than a new crate.**
  Rationale: `cargo package -p whitaker` fails today, so the plan's headline
  outcome is unreachable without it. The extraction also lets
  `--exclude whitaker` come out of `TEST_EXCLUDES`, which is what makes
  `CARGO_BIN_EXE_whitaker` usable and the end-to-end obligation runnable; it
  removes the `rustc_private` hazard permanently; and it stops `ortho_config`
  propagating into eleven lint crates and fifty cross-compiled builds. Putting
  the CLI in the binary's own package is also the universal Rust idiom —
  `cargo`, `rustup`, `ruff`, `uv`, and `cargo-dylint` all do it, and
  `cargo-dylint` solves the same contamination the same way. Approved by the
  requester before drafting. Date/Author: 2026-08-21.

- **D-2: Resolve the `whitaker` name collision by forwarding unrecognized
  arguments to `cargo dylint`.** The ambiguity tolerance fired here: the design
  document anticipates wrapper removal but assigns it to 3.9.3, while a root
  binary shadows the wrapper immediately. Alternatives considered — removing
  wrapper generation now, renaming the script, or pulling `whitaker check`
  forward from 3.5.2 — each either strands existing users or is a large scope
  increase. Forwarding costs roughly thirty lines, breaks nobody, and keeps
  this repository's own `make lint` working. It is a compatibility behaviour
  with named consumers (Constraint 2) and a named removal point (3.9.3), not
  compatibility theatre. Approved by the requester. Date/Author: 2026-08-21.

- **D-3: Include `whitaker ls`, explicitly unstable.** The ambiguity tolerance
  fired: `docs/roadmap.md:194` places `ls` at 3.8.1 ("Requires 3.6.2 and
  3.7.3") and `--json` at 3.8.2, while `CLI-DESIGN` §Public CLI surface
  describes the end state. Both readings were presented. Decision: ship it now
  as a rename of `whitaker-installer list`, with its output marked a regression
  guard rather than a compatibility promise, and `EP-M2` recording that 3.8.1
  and 3.8.2 will replace it wholesale. Approved by the requester. Date/Author:
  2026-08-21.

- **D-4: Exit codes are `0` success and display, `2` usage error, `1`
  operational failure.** This is not a free choice: `Cli::parse()` in
  `installer/src/main.rs:42` already exits 2 via clap on usage errors, and
  Constraint 1 plus `EP-INV-PARITY` require matching it. The first draft left
  this undecided and its sample code would have produced 1. Date/Author:
  2026-08-21, planning agent.

- **D-5: Do not unify the installer's three dependency-injection styles.**
  Rationale: a large refactor with its own risk profile and no requirement
  driving it. Folding it in would blur the parity evidence `EP-INV-PARITY`
  depends on. Date/Author: 2026-08-21, planning agent.

- **D-6: Adopt `ortho_config` for localized parsing only.** The task brief asks
  for `ortho_config` with localized help; `CLI-DESIGN` sequences the
  configuration switch as migration step 3 and `docs/roadmap.md` gives it item
  3.6.3, which _requires_ 3.5.1. Wiring `whitaker.toml` discovery here would
  take work from 3.6.3 and half-activate a model this plan cannot finish.
  Counter-argument recorded: with `NoOpLocalizer` there is no observable
  localization benefit at this milestone, so the dependency's roughly 26 new
  lock entries buy a one-line call-site difference. It is retained because the
  brief asks for it and because `EP-M0` confines the cost to the CLI binary.
  Verify in `EP-M2` with a twenty-line spike that `#[derive(OrthoConfig)]`
  composes with the clap-derived arguments, rather than leaving 3.6.3 to
  discover that it does not. Date/Author: 2026-08-21, planning agent.

- **D-7: Use `googletest` and `pretty_assertions` in new test files only.**
  The brief authorizes both despite a sibling plan's opposite preference;
  existing tests stay unchanged. Date/Author: 2026-08-21, planning agent.

- **D-8: Cut the release-asset-name Verus proof and the routing-conflict Kani
  harness; verify argv classification instead.** Both original obligations were
  vacuous — see `Verification plan` and `Surprises & discoveries` for the
  evidence. The forwarding behaviour introduced by D-2 creates a genuinely new
  repository-owned decision procedure whose failure modes are severe in both
  directions, which is what the two verification tools now target. Date/Author:
  2026-08-21, planning agent.

- **D-9: Name the plan `3-5-1-root-whitaker-binary.md`.** The brief filename
  said 6.5.1, but its task, branch, and PR title identify 3.5.1; confirmed by
  the requester. Date/Author: 2026-08-21.

- **D-10: Test the compiler-private core outside the workspace all-feature
  invocation.** Cargo unifies `dylint-driver` through every lint crate, and its
  compiler-private runtime cannot link into an ordinary integration-test
  executable. A separate feature-free package invocation preserves each moved
  test target and doctest without restoring the old `whitaker` exclusion or
  weakening the Dylint driver pass. Date/Author: 2026-08-24, implementation
  agent; supported by the H1 falsification experiment.

- **D-11: Accept the 70-file tolerance breach for this plan.** The user resumed
  implementation with the 80-file rebase delta and unfinished milestones
  documented. This accepts the file-count overrun only; D-12 later changes the
  net-line ceiling. PR #356 and branch ownership were verified in
  `leynos/whitaker`. Date/Author: 2026-10-15, implementation supervisor.

- **D-12: Raise the net-line ceiling to 7,000 for this plan only.** The user
  approved the increase at 4,869 lines and reconfirmed it on 2026-10-17. This
  changes no product scope or D-11 decision. Verifier tolerances measure Kani
  or Verus execution, separately from compilation and admission; one recorded
  Kani run took 4.73 seconds. Date/Author: 2026-10-16, implementation
  supervisor, following explicit user approval.

- **D-13: Keep installer metadata helpers source-compatible.** The root package
  needs to expand cargo-binstall templates for its own package name and inspect
  the root manifest from an integration test. Preserve the existing
  `expand_pkg_url`, `expand_bin_dir`, and `load_cargo_toml` signatures; add
  `expand_pkg_url_for`, `expand_bin_dir_for`, and `load_cargo_toml_from` for
  the new callers; `BinstallRelease` groups inputs behind one renderer for both
  templates, eliminating duplicate rendering and preserving existing
  call-sites. Date/Author: 2026-10-17, implementation agent.

- **D-14: Serialize nested-build UI tests and recover killed owners.** Extend
  `serial-dylint-ui` with the observed fixture and call-site tests after shared
  Cargo target-lock contention caused Dylint capture failures. A later Nextest
  timeout showed that killed attempts left an ownerless lock until its
  thirty-minute age threshold; reclaim it as soon as the advisory liveness lock
  is free. Live owners remain protected. Keep timeout and retry policy
  unchanged. Date/Author: 2026-10-19, implementation supervisor.

- **D-15: Use the conventional module directory facade for installer
  orchestration.** After extracting dry-run and exit adapters, the public
  facade still exceeded the 400-line Dylint limit. Moving it under
  `installer/src/orchestration/` resolved the size concern but Clippy's
  `self_named_module_files` rule requires the facade to be named `mod.rs`. The
  module is now `installer/src/orchestration/mod.rs`; child modules remain
  adjacent and callers retain the same `whitaker_installer::orchestration`
  path. Its explicit `#[path]` to the test tree is adjusted to
  `../tests/mod.rs`, since child module paths resolve from the directory
  facade. Date/Author: 2026-10-17, implementation supervisor.

- **D-16: Split root CLI parsing from command execution.** The `cli` module
  exceeded the 400-line Dylint limit, and the combined install/list handling in
  `run_command` exceeded the conditional-cluster threshold. Keep
  `whitaker::cli` as the stable in-workspace facade while moving parser types
  and helpers to `cli/parser.rs` and process-facing execution to
  `cli/runner.rs`. Use command-specific helpers and a single result mapping per
  command rather than nested matches. This preserves routes, output, and exit
  behaviour while giving each responsibility an independently sized module.
  Date/Author: 2026-10-17, implementation supervisor.

- **D-17: Patch Dylint test capture without bypassing compiler admission.**
  Dylint 6.0.4 checks only the first command token for `rustc`, so its UI
  harness loses builds that begin with the managed wrapper. Upstream issue 1696
  remains open. Patch the vendored 6.0.4 parser to find the compiler token and
  retain the command suffix; keep the dependency scoped outside the workspace
  members and remove the patch when upstream supports wrappers. This restores
  existing UI tests without excluding them or disabling the managed compiler
  wrapper. Date/Author: 2026-10-17, implementation supervisor.

- **D-18: Stream installation output and resolve Dylint paths by precedence.**
  Pass stderr through the install service so progress remains visible during
  downloads and builds. For forwarded Dylint invocations, preserve a non-empty
  `DYLINT_LIBRARY_PATH`; otherwise use an existing local build directory, then
  an existing host-specific prebuilt directory. Do not inject a path when no
  candidate exists. This preserves user overrides and prebuilt installs while
  avoiding nonexistent or empty build paths. Date/Author: 2026-10-17,
  implementation supervisor, following CodeRabbit review.

- **D-19: Keep CLI Verus out of the default group until verified.** The first
  run exceeded tolerance; keep it in `cli`, outside `all`, until proof and
  negative control pass. **Met 2026-10-19:** both passed; restored to `all`.
  `make verus` passed six files (5, 8, 9, 10, 8, 4; 0 errors each). Log:
  `/tmp/verus-all-3-5-1-root-whitaker-binary-reviewrepair2.out`.

- **D-20: Persist exact install paths for forwarded Dylint commands.** A root
  CLI invoked outside the Whitaker checkout cannot infer a custom target
  directory from the active toolchain or default staging roots. Record the
  canonical staging path per toolchain at the installer's shared successful
  completion point. Preserve a non-empty explicit `DYLINT_LIBRARY_PATH`; when
  absent, pass every extant recorded path so Dylint selects the matching
  toolchain, then fall back to the existing default candidates. Lock the JSON
  record during read-modify-write. Date/Author: 2026-10-17, implementation
  supervisor, following CodeRabbit review.

- **D-21: Make record failures mode-sensitive.** The legacy wrapper embeds its
  staging path and warns on record failure; root install needs its record for
  forwarding and fails instead. Date/Author: 2026-10-17, following review.

- **D-22: Recover corrupt records only on successful install.** Keep
  `installed_paths` strict; successful install replaces malformed JSON. Derive
  snapshots from the pinned toolchain, gate XDG tests, and ship the vendored
  helper's selected MIT notice. Date/Author: 2026-10-17, following review.

- **D-23: Pin `ortho_config` 0.10.0.** The repository requires current caret
  requirements, and 0.10.0 keeps the `LocalizedParse` and `is_display_request`
  interface used here while removing ten net lock entries. Date/Author:
  2026-10-25, following review.

- **D-24: Accept the remaining file-count overrun for this plan.** On
  2026-11-04 the owner directed that PR #356 be carried through review
  reconciliation to merge. That direction accepts the file-count excess that
  D-11 left open; the rebased diff is 136 files, 56 beyond D-11's accepted 80.
  The same rebase brought the net added lines to +6,245, inside D-12's
  7,000-line ceiling, so no further line-ceiling decision is required. This
  accepts scope only. It does not authorize crates.io reservation or
  publication, and it does not mark the plan `COMPLETE`. Date/Author:
  2026-11-04, implementation supervisor, following the owner's instruction.

- **D-25: Carry the reconciled scope past the D-12 line ceiling.** Measured on
  `0886261b` against `origin/main` (`84eed837`), the branch changes 159 files
  for +10,491/−1,113, a net +9,378 lines. That is 2,378 lines above D-12's
  7,000-line ceiling and 79 files above D-11's accepted 80, because the review
  repairs added the typed library-path and telemetry adapters, the root install
  end-to-end test, the Verus routing proof, the trybuild fixture, and the
  release-workflow contract test since D-24 was recorded. The owner's
  2026-11-04 direction — carry PR #356 through review reconciliation to merge —
  governs here; the added lines are the evidence the review findings demanded,
  not unrelated growth. This accepts scope only. It does not authorize
  crates.io reservation or publication, and it does not mark the plan
  `COMPLETE`. Date/Author: 2026-11-06, implementation supervisor, recording the
  measurement after the replay onto current `main`.

- **D-26: Re-measure the scope after the third base merge.** Measured on
  `2c8f79da` against `origin/main` (`cfa208c4`), the branch changes 171 files
  for +11,732/−1,214, a net +10,518 lines. That is 3,518 lines above D-12's
  7,000-line ceiling and 91 files above D-11's accepted 80. The growth since
  D-25 is `main`'s own landed work carried by the merge
  (`string_continuation_style`, its UI and proof harness groups, and the
  rolling-release restore) plus this branch's reconciliation of it; none of it
  is new product scope. The owner's 2026-11-04 direction to carry PR #356
  through review reconciliation to merge still governs. This accepts scope
  only. It does not authorize crates.io reservation or publication, and it does
  not mark the plan `COMPLETE`. Date/Author: 2026-11-08, implementation
  supervisor.

- **D-27: Isolate the root-install end-to-end test through a debug-only data
  root.** CodeRabbit's approval-response comment on `7f57dd75` observed that
  `tests/root_install_fixture` resolved the child's data root from the platform
  API on Windows and macOS, so the test wrote an installation record into a
  developer's real data directory. Windows resolves the data directory from the
  known-folder API and ignores `XDG_DATA_HOME`, and the CLI offers no injection
  point for it, so the fixture cannot isolate the child by environment alone. A
  debug-only override, `WHITAKER_INSTALLER_TEST_DATA_HOME`, follows the
  existing test seams (`WHITAKER_INSTALLER_TEST_STAGE_SUITE`,
  `WHITAKER_INSTALLER_TEST_CLIPPY_SNAPSHOT_MANIFEST`): debug builds honour an
  absolute value and release builds ignore it, so it cannot relocate a real
  installation. The same review found that `recorded_library_paths` filtered
  recorded paths with `is_dir()` before the resolver could inspect them, hiding
  an unreadable data directory behind the ordinary "nothing installed" state;
  `InstallationRecord::into_absolute_paths` now leaves that distinction to
  `classify_candidate`. Date/Author: 2026-11-08, implementation supervisor.

- **D-28: Merge `9fc35c08` and adopt `main`'s workspace-inheritance model for
  lints.** While the candidate was being gated, `origin/main` advanced to
  `9fc35c08` (PR #371, "Harden workspace lint, format, and documentation
  gates") and the PR turned `DIRTY`. Both branches had independently moved the
  lint configuration into `Cargo.toml`'s workspace tables; `main` did the
  188-file work needed to satisfy the strict pedantic set in every crate and
  gave each package `[lints] workspace = true`, while this branch had reached a
  narrower root-only `[lints]` block for the four packages it touched. The
  merge keeps `main`'s model and drops this branch's package-level
  `[lints.rust]`/`[lints.rustdoc]` duplication, because two mechanisms for the
  same policy would drift and the branch's version covered less. The two
  independent `DryRunInfo` redesigns (this branch's `DryRunSkips` type and
  `main`'s reuse of `cli::SkipFlags`) reconciled in favour of `main`'s, which
  removes a duplicate type; `main`'s `clone_error!` macro in
  `installer/src/error.rs` was adopted and this branch's `InstallationRecord`
  variant added to its field table. The two independent complexity-budget
  repairs for `bootstrap::run` reconciled by keeping this branch's `RunOptions`
  plumbing inside `main`'s instrumentation strategy, with the span moved to the
  public entry points and `resolve_library_dir` extracted. All of `main`'s
  changes that landed in paths this branch had moved were verified present in
  the moved form, so no `main` work was silently dropped. Date/Author:
  2026-11-08, implementation supervisor.

- **D-29: Close the four residual qualifications on the isolation repair.**
  CodeRabbit accepted the two repairs in D-27 but raised four qualifications on
  `bfe97b2a`, each of which is valid. (1) `InstallationRecord::contents()`
  still classified every path with `is_dir()`, so a record naming only
  directories the process cannot inspect was reported `stale`; it now reads the
  metadata error kind, and a new `RecordContents::Uninspectable` (mapped to
  `RecordReadOutcome::Uninspectable`, label `uninspectable`) keeps a broken
  data directory distinct from an installation that has been removed. The
  classification is decided by a pure `classify_paths` helper, so every
  combination of path states is tested without staging permissions a test may
  not have; an end-to-end case makes an absolute path fail inspection with an
  interior NUL, which both Unix and Windows reject with `InvalidInput`. (2) A
  release-profile test binary compiles the debug overrides out, so the child
  would ignore them and write to the developer's real data directory; the
  fixture now exposes `child_isolation_is_available()` and the end-to-end test
  refuses to spawn a child when it is false, failing in CI rather than skipping
  so a silent loss of the only end-to-end proof is visible. (3) The bin-root
  assertions inspected the fixture directory, not the directory the child
  resolved, and Windows derives that directory from the known-folder API;
  `SystemBaseDirs::executables` now honours an absolute
  `WHITAKER_INSTALLER_TEST_EXECUTABLE_HOME`
  (`test_support::TEST_EXECUTABLE_HOME_ENV`) in debug builds, ahead of
  `XDG_BIN_HOME`, and the fixture names the bin root through it so the seeded
  executable and the no-wrapper check address the directory the child used. (4)
  The proof that the record drives forwarding used a record a test wrote
  through the production writer rather than one an install produced; the
  end-to-end test now installs, then starts `whitaker --all` from a temporary
  project outside the checkout with a stub `cargo` first on `PATH`, and asserts
  the recorded staging directory is what the forwarded `DYLINT_LIBRARY_PATH`
  carried, with the stub's exit code surviving. The stub is Unix-only because
  it is a shell script, matching the existing recorded-path behaviour suite.
  Date/Author: 2026-11-08, implementation supervisor.

- **D-30: Re-measure the scope after the fourth base merge.** Measured on
  `4a96209c` against `origin/main` (`c404ca4e`), the branch changes 185 files
  for +12,679/−1,218, a net +11,461 lines. That is 4,461 lines above D-12's
  7,000-line ceiling and 105 files above D-11's accepted 80. The growth since
  D-26 is 943 net lines, from merging `main`'s offline test execution work (ADR
  0011) and reconciling its two-pass coverage with this branch's, plus the
  root-install fixture repair. The owner's 2026-11-04 direction to carry PR
  #356 through review reconciliation to merge still governs. This accepts scope
  only. It does not authorize crates.io reservation or publication, and it does
  not mark the plan `COMPLETE`. Date/Author: 2026-11-09, implementation
  supervisor.

- **D-31: Validate the root CLI's package through command-scoped registry
  patches.** Hosted CI on `1140f56c` failed `linux-full` at "Package the root
  CLI": Cargo could not satisfy `whitaker-installer = "^0.2.12"`, because only
  0.2.11 is on crates.io. `main`'s `d0055cd4` bumped every crate to 0.2.12, so
  the step — added by `055406a9` while the workspace still matched published
  versions — cannot pass in a pull request whose workspace version has not been
  released. `docs/publishing.md` already keeps `whitaker` out of
  `publish-check` for that reason. The step now runs `make package-root-cli`,
  which points the installer and common registry requirements at the workspace
  sources for that command only, so the packaged manifest and file list stay
  under test before publication without pretending the registry copy exists.
  The registry-true build remains the documented post-publication
  `make publish-check PUBLISH_PACKAGES=whitaker`. The workflow contract pins
  the step's command and checks that the recipe keeps `$(CARGO_LOCKED)` and
  both patches. Date/Author: 2026-11-09, implementation supervisor.

- **D-32: Restore the nextest filter guard's recipe tab and guard the class.**
  Gating the fifth base merge found `test-nextest-filter`'s `cargo nextest run`
  line without its leading tab. Make parsed the line as a variable assignment,
  so the target ran only its `command -v` guard: the dedicated
  `nextest_ui_filter` suite never ran, while `make test` still exited 0. The
  earlier merge `afaa78ac` caused the loss when it joined `main`'s
  `CARGO_NET_OFFLINE=true` prefix to this branch's `whitaker_lint_core` command;
  `main` and the merge base keep the tab. Restoring it runs the 44
  `nextest_ui_filter` cases. The failure is silent and invisible to every other
  gate — the line still reads as the command, and `mbake validate` accepts the
  file — so `make_recipe_contract_test.py` now dry-runs each pass `make test`
  delegates and requires that pass's own command to appear in `make -n`. A
  discrimination probe that stripped the tab again failed exactly that case and
  left the other two passing. Date/Author: 2026-11-09, implementation
  supervisor.

- **D-33: Gate the fixture's external-project helper to Unix.** Hosted
  `windows-compat` on `f7373956` failed to compile `root_install_e2e`:
  `InstallFixture::external_project` is never used there, and `-D warnings`
  turns that dead code into an error. Its only caller,
  `assert_record_drives_forwarding`, already carries `#[cfg(unix)]` because the
  stub `cargo` it drives is a shell script, so the helper now carries the same
  gate. A Linux gate cannot see this: the path exists only on Windows, and the
  installed `x86_64-pc-windows-msvc` target cannot build `ring` locally without
  MSVC's `lib.exe`, so the hosted job is the authoritative proof. Date/Author:
  2026-11-09, implementation supervisor.

- **D-34: Adopt main's `{binary-ext}` bin-dir template in the refactored
  renderer.** Main advanced to `2a1d0602` (#593) to fix Windows `binstall`
  verification: `bin-dir` became `{name}-{target}-v{version}/{bin}{binary-ext}`
  and the inline expansion substituted `{binary-ext}`. This branch had already
  refactored that function into `expand_bin_dir_for`/
  `render_release`/`binary_name`, which appended `.exe` to `{bin}` directly, so
  the merge conflicted. The resolution keeps the refactor and takes main's
  template: `render_release` now substitutes `{bin}` with the package name and
  `{binary-ext}` with the target's suffix, and `binary_name` gives way to
  `binary_extension` and `archive_format` helpers. The branch's
  `expand_bin_dir` therefore resolves the Windows path through the placeholder
  the manifest actually declares, and main's new
  `bin_dir_path_exists_in_packaged_archive` test covers every release target.
  Date/Author: 2026-11-10, implementation supervisor.

- **D-35: Reconcile the root manifest's bin-dir with the shared template.**
  The merge gates caught what the conflict itself could not: D-34 changed the
  constant that `tests/behaviour_binstall.rs` compares against, but the root
  package's own `[package.metadata.binstall]` in `Cargo.toml` still declared
  `{name}-{target}-v{version}/{bin}`. Main carried no root binstall metadata,
  so that file neither conflicted nor was revisited, and the focused
  `-p whitaker-installer` run does not build the root package's behaviour
  binary. `make test` failed at `tests/behaviour_binstall.rs:41` with
  `left: .../{bin}` against `right: .../{bin}{binary-ext}`. The root template
  now declares `{binary-ext}` as well, which is what its archive layout needs
  on Windows, and the existing comparison test is the regression guard. Date/
  Author: 2026-11-10, implementation supervisor.

## Outcomes & retrospective

- **EP-M0, 2026-08-24.** Complete. Extracted the compiler-private driver into
  `whitaker_lint_core`; root metadata and a feature-free core test pass make
  the CLI package publishable and tested.
- **EP-M1, 2026-08-24.** Complete. Installer orchestration is library-owned;
  the old binary is a thin composition root, protected by its dry-run snapshot.
- **Status, 2026-10-19.** CLI/docs are implemented; Kani passed 360 checks,
  and Verus proof/control pass focused and in the default group. Crates.io
  reservation remains unauthorized, so the plan stays `IN PROGRESS`.
- **Validation follow-up, 2026-10-25.** Rebasing onto the PR target
  `harden-lint-config` was a no-op; all 17 branch commits remain unchanged.
  Hosted CI for `789f5212` found a Windows-only unused test import and the
  CodeScene string-argument threshold in installer metadata. The import is now
  Unix-gated, and the one-use archive-format helper is inlined. The complete
  local CodeScene delta is clean after these changes. The vendored UI example
  runner now owns its example-test method on the existing `Test` type, removing
  the five-argument helper without changing the test flow. Formatting, tests
  and doctests, typecheck, lint, Markdown, Mermaid, Makefile validation, and
  focused CodeScene checks passed on this candidate. Hosted coverage CI also
  exposed incompatible `--no-report` and `--no-clean` options in the Makefile;
  the target now uses `--no-report` for both passes, with a workflow-contract
  regression protecting that option combination. The plan stays `IN PROGRESS`,
  and crates.io reservation remains unauthorized.
- **Rebase and validation follow-up, 2026-10-25.** The fetched PR base
  `afb1e4ac` remains current. Rebase was a no-op, and `range-diff` matched all
  21 existing commits; there were no conflicts. Commit `80991c60` moves
  vendored example-target resolution and compiler-flag extraction into
  `vendor/dylint_testing/src/examples.rs`. The three vendored modules are 297,
  176, and 169 lines. CodeScene reports no issues. Vendor formatting,
  `make check-fmt`, `make test` (1,703 passed, 5 skipped; the secondary suite
  and doctests passed), `make typecheck`, `make lint`, and root-package
  packaging passed. Scoped CLI Kani proofs and reachability controls passed;
  `make verus` verified 44 obligations. The broad `make kani` run was
  interrupted when the shared filesystem filled, before a verdict. Before this
  note, the 128-file diff was +7,250 net; including it: +7,265 net (265 above
  D-12's 7,000-line ceiling). The later proof-comment correction reduced the
  current diff to +7,264 net (264 above). Owner approval remains outstanding;
  plan stays `IN PROGRESS`; crates.io reservation remains unauthorized.
- **Rebase and validation follow-up, 2026-10-31.** Rebased onto `41a00266`
  ("Record code health review results"). All 29 branch commits were replayed;
  conflicts were resolved by adopting the target wherever it already delivered
  the branch's intent, and by keeping both sides where each contributed
  something the other lacked. The three decisions worth recording:

  - `vendor/dylint_testing` is an add/add collision: the target had already
    vendored upstream `dylint_testing` 6.0.1, including the wrapper-aware
    `rustc_invocation`/`select_ui_test_flags` parsing that the branch's
    `examples.rs` split was reaching for. The target's copy is adopted
    wholesale, which retires the branch's vendored helper split as redundant.
    The branch-only `README.md` caveat and `LICENSE-MIT` are retained.
  - `Cargo.lock` takes the target's content, then is rebuilt. The rebuild
    surfaced a genuinely stale edge (`filetime` for
    `rstest_helper_should_be_fixture`); `cargo metadata --locked` now passes.
  - `InstallerError` gained a variant on each side — `DependencyCheck` from the
    target and `InstallationRecord` from the branch. The two installer
    `trybuild` fixtures pin exhaustive matching over the _merged_ variant set,
    so both were updated: the compile-pass fixture gains the missing arm and
    the compile-fail fixture's expected diagnostic names both uncovered
    variants.

  `make check-fmt`, `make test` (1,958 passed, 5 skipped; the secondary suite
  and doctests passed), `make typecheck`, and `make lint` all pass.
  `cargo metadata --locked` passes on the committed index. Plan stays
  `IN PROGRESS`; crates.io reservation remains unauthorized.
- **Rebase onto the current `main`, 2026-11-02.** The PR target advanced one
  commit while the previous rebase was being validated: `origin/main` moved from
  `bd4c93a8` to `395f1d3f` ("Add an on-demand Nextest split benchmark (#513)
  (#536)"), which only adds benchmark scripts, a workflow, and their contract
  tests. The branch was replayed onto it with the exclusive boundary `bd4c93a8`
  and 37 commits, under the unattended policy that replaces the Weave driver
  with `git merge-file` for the operation; there were no conflicts. The audit
  is exact: `range-diff` marks all 37 commits `=`, and
  `git diff bd4c93a8 395f1d3f` is byte-identical to the diff between the
  pre-rebase and post-rebase heads, so the replay changed nothing except the
  inherited target commit. New head `277a58c9`.

  Two candidate defects found while validating the merged result are fixed in
  this candidate rather than worked around. `3bd22456` points the suite's
  `exported_entry_point_registers_suite_lints` test at
  `whitaker_lint_core::run_ui_tests!` and swaps the suite dev-dependency,
  because main's new test still assumed the root package was the shared lint
  library. `84ca4f46` makes the legacy installer's dry-run snapshot
  machine-independent by normalizing the absolute workspace, staging-target and
  Dylint-driver paths and the Clippy-snapshot state that the merged bootstrap
  reporting now prints. `277a58c9` removes two long fixture tests from the
  default profile's broad 20-minute override, whose first-match precedence had
  been shadowing their dedicated 75- and 120-minute allowances.

  That last change was itself wrong, and `make test`'s second pass caught it:
  `whitaker_lint_core::nextest_ui_filter` requires the first override that
  configures `serial-dylint-ui` to keep naming the nested Cargo harnesses, so
  removing them broke the contract even though it unshadowed the allowances.
  The filter is restored to name every harness, and the long allowances now sit
  _above_ it, configuring only `slow-timeout` and `retries`. Precedence is
  per-setting, so the earlier tables are reached for their own settings while
  serialization still comes from the first override that configures
  `test-group`. The alias fixture's 75-minute allowance was shadowed the same
  way and is moved with them, and the developers' guide section on per-setting
  precedence now describes the arrangement that actually exists. This
  correction is the commit that carries this entry.

  Validation: `make check-fmt` and `make markdownlint` pass on `277a58c9`;
  `make lint`, `make typecheck`, and `make test` were started against that head
  and their results are recorded below when they complete. Plan stays
  `IN PROGRESS`; crates.io reservation remains unauthorized.
- **Second rebase onto `main` and replay repair, 2026-11-02.** `origin/main`
  advanced again, to `00186d43`, and the branch was replayed onto it from
  `eaccfe83` with the exclusive boundary `395f1d3f` and 39 commits under the
  unattended policy (the Weave driver replaced by `git merge-file --zdiff3` for
  the operation).

  The replay is the lesson of this entry. Conflicts in `installer/src/cli.rs`,
  its `source_policy` tests, `wrapper.rs` and `orchestration/dry_run.rs` were
  resolved by truncating each file at the marker instead of merging, and the
  truncated blobs were then carried into every later replayed commit. Nothing in
  `range-diff`, in the end-of-series diff, or in the first pass of
  `cargo check` pointed at the cause: the damage appeared as a handful of
  unrelated compile and dead-code errors, two of which named files that the
  truncation had not even touched. The reliable detector was a scripted audit
  comparing, for every path in the branch diff, a three-way
  `git merge-file --zdiff3` of the pre-rebase blob, its base and the target
  against the replayed `HEAD` blob. That found all four files; fixing only the
  two the compiler named would have left two silently short files.

  Each of the four is reconstructed from that three-way merge, keeping the
  target where it already delivered the branch's intent and keeping both sides
  where each contributed something the other lacked. `cli.rs` keeps the
  target's nested flag structs and the branch's equality derives, extended
  `Default` values and `install_args` accessor. The source-policy tests keep
  the target's `super::super::` paths and import only the type they construct.
  `wrapper.rs` keeps the target's path-free wrapper plus the branch's
  `WrapperScripts` selection; the legacy installer writes both scripts, and the
  root CLI writes none. `dry_run.rs` restores the target's bootstrap-plan
  reporting call, without which `report_bootstrap_plan` is dead code and the
  dry-run snapshot loses its Clippy-snapshot lines.

  The rest follows the target's renames and signatures: `BaseDirs` accessors are
  `home`, `executables` and `whitaker_data`; `load_cargo_toml` and
  `extract_binstall_table` return `Result`; the root package tests use both. A
  duplicate `#[cfg(test)] mod tests` in `lib.rs` is removed, because
  `orchestration/mod.rs` already wires the installer tests by path and the
  duplicate made those items inaccessible from the test crate. One doc comment
  drops an intra-doc link to a cfg-gated item that `cargo doc` cannot resolve.
  `Cargo.lock` is regenerated from the manifests.

  Validation on `0e7413f3`: `make check-fmt`, `make lint`, `make typecheck` and
  `make test` all pass. The test gate ran 2,186 tests with 2,186 passed and 6
  skipped, the secondary suite passed 84, and the workspace doctests passed.
  Plan stays `IN PROGRESS`; crates.io reservation remains unauthorized.

- **Hosted-review repairs on the rebased head, 2026-11-02.** The first push of
  `cd67d26a` failed three hosted checks, and each failure had a different
  cause. All three are repaired in the working tree rather than suppressed.

  CodeScene reported a large `InstallerError::clone` and a large `run_install`,
  and module-level duplication between the two root help tests.
  `InstallerError` now keeps one private `clone_simple_variant` for the
  ordinary owned payloads and expresses `Clone` as a short chain over the
  existing per-family helpers; the `unreachable!` arm lists the variants those
  helpers already handle, so a new variant fails loudly instead of silently
  cloning incorrectly. `run_install` now resolves its build inputs through a new
  `plan_install`, which returns an `InstallPlan` of the Clippy snapshot,
  workspace root, requested crates, toolchain and target directory. The two
  root help tests share a `run_whitaker` helper that returns
  `io::Result<Output>`, leaving the `.expect(...)` calls inside the tests so
  that the strict `expect_used` policy is satisfied without a suppression.
  `cs check` reports 10.00 for all three files.

  The MSRV check rejected a let-chain in `ensure_whitaker_workspace`, which
  Rust 1.85 predates. The notice is extracted to `report_workspace_action`,
  whose early returns for an unavailable clone directory or working directory
  reproduce the original control flow without the unstable syntax. The same
  extraction removes a nested block that CodeScene scored as a large method.

  The coverage job failed on the `run_install_for_whitaker` doctest, which
  still called the two-argument form after the `stdout` parameter was added.
  The example now declares and passes its own `stdout`, and `make test-doc`
  joins the local gate set because `make test` does not execute doctests.

  One install-order point is unchanged: the Clippy snapshot is still installed
  before the workspace and toolchain are resolved, so a release that omits its
  snapshot fails early rather than after minutes of other work.

- **Workflow-contract repair on `8b6d0113`, 2026-11-02.** The `linux-full` job
  failed five assertions in `tests/workflow_contracts`, all caused by the
  rebase: `main` had since tightened the nextest-profile and lane-deduplication
  contracts, and this branch's `.config/nextest.toml` and `Makefile` predated
  them.

  The three default-profile long-allowance overrides for the nested Cargo
  fixtures now name `test-group = "serial-dylint-ui"` themselves. That is what
  the contract requires, and it is also the correct mechanism: the group, not
  the broad filter, is what prevents those builds from racing on the shared
  target directory. The broad default override's filter is restored to the
  contract's exact string, so it no longer lists those three tests; listing
  them there was only ever a way to inherit serialization, and it forced the
  longer allowances to be declared first to avoid the 20-minute cap shadowing
  them. The overrides are restored to `main`'s order, with the broad filter
  first, and the comments state that arrangement.

  `crates/whitaker_lint_core/tests/nextest_ui_filter.rs` selected the first
  override naming the group and required every nested-Cargo clause in its
  filter. That only held while the broad filter listed those tests. The guard
  now collects every group-naming override and requires each clause to appear
  in at least one of them, which is the property that actually keeps the builds
  serialized; the `(binary(ui) & test(=ui))` and retry checks still target the
  broad override, identified by its `test(driver::ui::)` clause.

  The `coverage` recipe now writes `$(MAKE) test TEST_RUNNER=...` on one line.
  The recipe already delegated to the test recipe; the contract matches the
  literal prefix, and the line break had hidden it.

  `make test-workflow-contracts` passes 793 tests.

- **Rebase onto current `main`, 2026-11-04.** `origin/main` was `5bf23eab`
  when the rebase started. The exclusive boundary was `bb6dab1a`, the old head
  `a7d09b2c`, and the series 44 linear commits; the replay used
  `--merge --no-fork-point --no-update-refs --no-autostash
  --reapply-cherry-picks --keep-empty --empty=stop`
  with `zdiff3` and, per the unattended policy,
  `core.attributesFile=/dev/null` so the globally selected Weave driver could
  not participate. Two commits conflicted.

  The `Makefile` conflict was main's #539 doctest change against this branch's
  `whitaker_lint_core` extraction. Both were kept: `CORE_TEST_CARGO_FLAGS` and
  the `whitaker_lint_core` exclusions remain, and main's explanation of why
  `bumpy_road_function` is doctested, together with its
  `DOCTEST_EXCLUDES ?= --exclude clippy_utils` prefix, is restored. Dropping
  either side would have removed coverage: the branch's core tests cannot run
  in the all-feature pass, and main had just made the bumpy-road examples
  compile-fail contracts.

  `Cargo.lock` was taken from `origin/main` wholesale, as required for
  packaging lock files, then rebuilt with
  `cargo metadata --format-version 1 --offline`. The rebuild adds the
  `whitaker_lint_core` package and `googletest`/`pretty_assertions`, and moves
  the lint crates from the old `whitaker` dependency to the extracted core.

  The audit explains the whole replay. `range-diff` marks 41 of the 44 commits
  `=`; commits 11 and 12 changed only to adopt main's doctest exclusions, and
  commit 40 is the lock-file rebuild. `git diff --check` is clean. New head
  `807a4486` changes 136 files for +7,126/-881, a net **+6,245** lines, inside
  D-12's 7,000-line ceiling. The file count remains 56 above D-11's accepted
  80; D-24 records the owner's acceptance of that remainder.

  `origin/main` advanced by one further commit while the gates ran: `15e65ba8`
  updates only `tests/workflows/requirements.txt`, and a `git merge-tree` of
  that target with this head is clean, so no second replay was required.

  `ortho_config` resolves to 0.10.0 in the workspace manifest, the root
  package, and `Cargo.lock`, and the root CLI calls only the 0.10.0
  `LocalizedParse`, `Localizer`, and `is_display_request` interfaces.

  `b99f01d6` then rebuilt the lock file and recorded this replay. The final
  candidate changes 138 files for +7,255/-900, a net **+6,355** lines, still
  inside D-12's ceiling.

  Gate evidence on `b99f01d6`, each gate run once and sequentially, with Cargo
  and compiler admission allowed to queue:

  - `make check-fmt` — pass in 14 s.
  - `make typecheck` — pass in 734 s after queueing in compiler admission.
  - `make lint` — pass (cached; 3 s).
  - `make test` — pass in 10,675 s: main nextest profile 2,185 passed / 6
    skipped, the secondary suite 84 passed, and the `dylint_testing` library 19
    passed.
  - `make test-workflow-contracts` — pass, 793 tests.
  - `make markdownlint` — pass, 0 errors over 114 files (36 s), spelling
    included.
  - `make nixie` — pass.
  - `scripts/run-kani.sh whitaker-cli` — `verify_classify_is_total_and_disjoint`
    verified 360 checks; the three reachability harnesses each produced their
    expected counterexample.
  - `scripts/run-verus.sh cli` — `verification results:: 4 verified, 0 errors`.

  The four negative controls were re-run on `b99f01d6` after the rebase,
  because the replay and the `ortho_config` 0.10.0 port changed the code each
  one guards. Each mutation was applied alone, the targeted check run, and the
  file restored to its exact `HEAD` blob (verified by `git hash-object`
  equalling `git rev-parse HEAD:<path>`):

  - `EP-INV-DISPATCH`: classifying `Ls` as `Forward` made
    `verify_classify_is_total_and_disjoint` fail with
    `assertion failed: result == Class::Subcommand(Subcommand::Ls)`; the script
    exited 1.
  - `EP-LEM-DISPATCH`: widening `is_display` to include the subcommand class
    made the Verus proof report `3 verified, 1 errors`, with the failed
    postcondition `!(is_subcommand(tokens) && is_display(tokens))`.
  - `EP-INV-PARITY`: forcing `skip.no_update = false` in the mapping made the
    4,096-case property test fail at case 12, `["--no-update"]`, with
    `no_update: false` against the expected `no_update: true`. The generated
    proptest regression file was deleted, since it recorded a deliberately
    broken candidate.
  - `EP-INV-EXIT`: mapping a display request to `Outcome::UsageError` made
    `root_help_exits_successfully` fail with actual status 2 against expected
    status 0.

**Forwarding and diagnostics repair.** Two P1 findings on the published head
were confirmed against the source and repaired:

- Forwarded invocations started `cargo dylint` directly, which bypassed the
  installed bootstrap: no verified Clippy snapshot, no managed driver slot, and
  no cold-build serialization, so `whitaker --all` regressed to the first-run
  failure the bootstrap exists to prevent. `bootstrap::run` now takes
  `RunOptions`, the root CLI passes the multi-toolchain library path it
  resolved, and the same run path performs verification, locking, and the
  built-driver notice.
- Nothing in the root executable installed a `tracing` subscriber, so the
  installer library's events were discarded. `src/diagnostics.rs` installs the
  root-owned subscriber before routing, with the installer binary's policy:
  `-q` admits only errors, any `-v` admits debug, and the default admits
  warnings.

Routing every forward through the bootstrap unconditionally would have broken
the branch's own `CLI-REQ-FWD` compatibility contract for a source checkout or
a manually configured `DYLINT_LIBRARY_PATH`, where no bootstrap exists to run.
Forwarding therefore selects: `bootstrap::is_installed` answers whether a
record exists, the recorded bootstrap runs when it does, and a machine with no
record keeps the legacy direct `cargo dylint`. A record that exists but is
unreadable still counts as installed, so the bootstrap reports the defect
rather than a silent fallback hiding a broken installation. The exit contract
tests pin this by pointing `XDG_DATA_HOME` at an empty directory, which selects
the legacy path deterministically instead of inheriting whatever the
developer's real data root holds.

The behaviour fixtures now install a real (fixture) bootstrap and a stub
`cargo` that records its argument vector and `DYLINT_LIBRARY_PATH`, so both
properties are asserted end to end rather than through a PATH override alone.
The verbose scenario asserts the bootstrap's cold-build `info` event on stderr,
which passes only when the root binary installs a subscriber.

One bootstrap fixture also had to learn where a stub program may live. The
explicit-override test removes the recorded library directory to prove that the
override skips the recorded-directory check, and the stub `cargo` had been
written through `library_dir/../../..`. Path resolution requires every
intermediate component to exist, so removing `lib` made the spawn fail with
`ENOENT` even though the lexical destination was untouched. The stub now sits
directly at the layout's data root, which the test never removes, so the test
measures the override rule rather than a deleted-parent side effect.

**Release ordering.** CodeRabbit observed that the root package's registry
dependency on `whitaker-installer 0.2.11` does not contain the `orchestration`
and `installation_record` modules this branch imports, so a
`cargo package --no-verify` check cannot prove that a release would compile,
and the installer therefore has to be published before the root crate. The
observation is correct and already governs the release: `docs/publishing.md`
requires a coordinated bump of `common`, `installer`, and the root manifest,
followed by publication of `whitaker-common`, `whitaker-installer`, then
`whitaker`. `EP-M3b` (crates.io name reservation) remains unauthorized, so no
version is bumped and nothing is published here. `--no-verify` stays until that
release step, when the coordinated bump and installer-first order make the
verification build resolvable.

**Root-install coverage.** The root CLI's install route is now exercised end to
end by `tests/root_install_e2e.rs`, which drives the public binary against
isolated directories and asserts a real successful install: the staged suite,
the persisted installation record, preservation of an existing `whitaker`
executable, the absence of both generated wrapper scripts, and the root
diagnostics. `installer/src/orchestration/finish.rs` additionally carries unit
coverage for the record-failure policy in both directions — fatal for the root
CLI, warn-and-continue for the legacy installer. The dry-run behaviour scenario
still proves argument routing without building the workspace, but it does not
prove the no-wrapper policy: a dry run generates nothing, so its absence proves
nothing. Only the end-to-end test, which inspects the bin root after a real
install, establishes that policy. The route is therefore covered at three
levels rather than delegated entirely to the installer's own `behaviour_cli`
suite.

**Gate evidence for the repair commit.** Commit `620cf541` carries the two P1
repairs, the bootstrap fixture correction, and the remaining mechanical
findings. (The pre-replay history recorded the same change as `625a0600`;
`620cf541` is its replay onto current `main`.) `make fmt`, `make check-fmt`,
`make lint` and `make typecheck` all exit zero on that tree. `make test` was
split into `nextest run --partition count:1/2` and `count:2/2` because a single
invocation of the full suite exceeds the harness's two-hour background cap; the
pass runs after the partition filter, so each half is a different set of tests
rather than a repeat. Together they run 2199 workspace tests, all passing, plus
84 `whitaker_lint_core --no-default-features` tests and 19 `dylint_testing`
tests that the two invocations each execute in full. The earlier uninterrupted
run reached 2185 of 2199 with no failures before the cap stopped it, which is
consistent with the split result.

**Replay onto current `main`, 2026-11-06.** The branch was replayed onto
`origin/main` (`84eed837`) with `zdiff3`; 57 commits were replayed and the
resulting head was `6190082e`. Two target commits changed the rules the branch
had to satisfy, and both are now met:

- `c1e87255` denies `missing_docs` in every workspace crate and adds the shared
  `[workspace.lints.rust]` and `[workspace.lints.rustdoc]` sets. The root
  package mirrors those tables rather than inheriting them, because Cargo
  forbids `workspace = true` beside its own lint tables and the root crate's
  Clippy set cannot be exported workspace-wide (D-24 measures the 134 denials
  that alternative produces). Mirroring is what keeps the root crate inside the
  new denials without loosening the other four packages.
- `a46cf1e5` adds `DEV_STANDARD_FLAGS` (ADR 010) and requires it in every
  recipe that assigns `RUSTFLAGS`, because an assigned `RUSTFLAGS` replaces all
  other configuration sources. The `test:` and `test-doc:` recipes now carry it
  alongside the branch's core-test and doctest lines.

The replay also produced two tree defects that were repaired before gating. The
root manifest ended up with two `[lints.rust]` tables, which is invalid TOML;
`0886261b` keeps the single table that `main` introduced. `Cargo.lock` was
taken from `main` and regenerated, as the lock-file policy requires; the
rebuild added no further change, so the replayed lock already encoded both
`main`'s newest versions and the branch's dependency removals.

Gate evidence on `0886261b`, each gate run once and sequentially, with Cargo
and compiler admission allowed to queue: `make check-fmt`, `make lint`,
`make typecheck`, `make test` (2,218 passed / 6 skipped in the main profile, 84
in the secondary suite, 19 `dylint_testing`), `make test-workflow-contracts`
(795 passed), `make nixie`, and `scripts/run-verus.sh cli`
(`8 verified, 0 errors`) all passed. `make markdownlint` failed only at its
`spelling` prerequisite, which rejected three `-ise`/`-ifact` forms in the new
test comments; `c15be632` corrects them, and the markdownlint step proper then
ran clean over 115 files.

**CodeRabbit repair round, 2026-11-06.** The review of `3849310c` left three
unresolved inline threads and eleven pre-merge rows. Each was verified against
the tree before repair, and the repairs are behavioural rather than
suppressions:

- `.config/nextest.toml` carried a sentence claiming that a `test(ui::ui)`
  filter does not capture other tests in the same binary. That is false — the
  filter matches the test _name_, not the binary — and the sentence was
  deleted. The authoritative later block, which explains that the binary clause
  matches only the top-level `ui` test, is unchanged.
- `installer/src/orchestration/finish.rs` reported wrapper-script paths for a
  root install that deliberately writes no scripts. The root branch now reports
  the installed binary statically (`whitaker ls`, `whitaker --all`) and never
  resolves the wrapper directory, which would otherwise create an empty
  directory the root install has no use for.
- `installer/src/list.rs` swallowed every toolchain-detection failure into
  `None`. A directory that declares no toolchain is still `None`, but a
  `rust-toolchain.toml` that exists and cannot be read or parsed is now a typed
  `InstallerError`, surfaced through the new `CliError::ToolchainDetection` in
  the library-path adapter. The lossy `detect_active_toolchain` remains for
  `whitaker ls`, where the toolchain is display metadata only.
- `tests/root_install_e2e.rs` now seeds a pre-existing `whitaker` executable
  in the bin root and asserts the install neither rewrites nor removes it, so
  the "no wrapper scripts" claim is tested against a file that could collide
  with one.
- `tests/workflows/test_ci_workflow.py` gained the `Package the root CLI` step
  in both the linux-full step list and the linux-only set, with an assertion
  pinning its command, so the packaging contract is enforced rather than merely
  documented.
- The Docstring Coverage warning was answered with substance: the production
  functions the diff introduced without a `///` comment now carry one, in
  `src/adapters/mod.rs`, `src/main.rs`, `installer/src/bootstrap/run.rs`,
  `installer/src/installation_record.rs`, and `verus/whitaker_cli_dispatch.rs`.
  The remaining shortfall is test functions and fixtures, which the repository
  does not document by policy.

**Replay onto `main` after the repair round, 2026-11-06.** `main` advanced to
`a3b4487f` ("Enforce Python docstring coverage in make lint (DF104)"), which
made the branch conflict on `Makefile` and `AGENTS.md`. The branch was replayed
onto `a3b4487f` with `zdiff3`; all 61 branch commits replayed without a
conflict, and `git range-diff` reports every one as identical, so the replay
changed only the base. The branch keeps main's `interrogate` gate and its own
test-target wiring.

The replay surfaced one spelling defect that the new base's stricter gate
rejects: the root install end-to-end test's doc comment said `recognisable`,
which en-GB-oxendict takes as `recognizable`. Fixed in `ebe1b454`. The new base
also raises the diff to 161 files, +10,799/−1,157 (net +9,642 lines) against
the 7,000-line soft tolerance, recorded as D-25.

**Base merges and the round-three repairs, 2026-11-07.** `main` advanced twice
while the round-two repairs were under review. The branch merged it both times
rather than being replayed, so its published history and the review evidence
attached to it stay intact:

- `origin/main` at `66155f55` merged as `77ef7abd`. The conflicts were the root
  `Cargo.toml`, `Cargo.lock`, the two `context/mod.rs` files, `src/lib.rs`, and
  the example harness sources; each kept the branch's structure and took main's
  newer logic.
- `origin/main` at `3a370bf9` (the #560 revert of the `nightly-2026-10-30`
  move) merged as `f4929e7e`. Main's revert removes `crates/rustc_attr_ir` and
  the `attributes_from_hir` helper this branch had adopted for the newer
  toolchain, so both are gone again, and `convert_attributes` is back to main's
  `&[hir::Attribute]` form with its import repointed at
  `whitaker_lint_core::hir`. The root package keeps the CLI boundary and the
  lint sources stay under `crates/whitaker_lint_core`.
- `Option::map_or_default` is unstable on the restored `nightly-2026-05-28`, so
  the `map_unwrap_or` repair in `tests/behaviour_whitaker/support.rs` uses
  `map_or_else` instead: the same fix in a stable spelling.

CodeScene then failed head `f4929e7e` on its advisory duplication rule:
`counter_value` and `histogram_sample` in `src/adapters/telemetry/tests.rs`
repeated the same name-and-label predicate. The predicate now lives in one
`recorded_value` helper and each reader matches the `DebugValue` variant it
expects, so `cs delta origin/main` reports no issues for the branch. The merge
and repair round raises the diff to 168 files, +11,648/−1,207 (net +10,441
lines) against the 7,000-line soft tolerance recorded as D-25.

Gates on `2b4ec653` passed sequentially: `make check-fmt` (6 s), `make lint`
(38 s, interrogate 100%, Clippy clean), `make typecheck` (32 s), `make test`
(1830 s: 2225 nextest tests passed with 6 filtered by the default profile, 84
lint-core tests, and 19 `dylint_testing` tests), `make markdownlint` (28 s), and
`make nixie` (5 s). The filtered six are the installer behaviour binaries that
CI runs under `NEXTEST_PROFILE=ci`; the hosted `linux-full` job is the evidence
for those, not the local default profile.

**Third base merge, 2026-11-08.** `main` advanced again while the round-three
repairs were queued for review, to `cfa208c4` (`string_continuation_style` as a
post-expansion early lint, #355, and the rolling-release restore, #563). The
branch merged it as `d0bf6227` so the published history and the review evidence
attached to it stay intact.

- `crates/no_unwrap_or_else_panic/src/lib_ui_tests.rs`: main's #355 moved the
  environment guard into `run_with_runner` itself (it now holds
  `runner_env_guard`, which serializes and sets the toolchain variables) and
  deleted the per-harness `temp_env` workaround from all three example
  harnesses. This branch already carries that central guard, so the merge takes
  main's simplified runner body with the branch's
  `whitaker_lint_core::testing::ui` path; the local `with_vars_unset` guard and
  its two imports are gone, matching `no_expect_outside_tests`.
- `scripts/run-kani.sh` and `scripts/run-verus.sh`: main's `string-literals`
  harness group and this branch's `whitaker-cli` / `cli` group are both kept,
  in the default run, the case dispatch, and the `all` proof list.
- `docs/contents.md`: main indexes the early-lint ADR as 010, so the
  CLI-boundary record this branch added becomes **ADR 011**
  (`docs/adr-011-whitaker-cli-boundary.md`), and every inbound link — the
  design document and this plan's own references — moves with it. That keeps
  the locally allocated serials unique; main's own
  `adr-010-keep-the-fast-linker…` file stays untouched.
- `docs/repository-layout.md`: the lint-crate list gains
  `string_continuation_style`, the support-crate list keeps
  `whitaker_lint_core`, and main's pure-domain crate entry
  (`whitaker_string_literals`) is kept.
- `docs/v0-3-0-migration-guide.md`: main's note that
  `string_continuation_style` joins the default suite now precedes this
  branch's root-package section.
- `crates/string_continuation_style` depended on the root `whitaker` package
  for `SharedConfig` and the UI runner. On this branch those live in
  `whitaker_lint_core`, so the crate's manifest, `src/diagnostics.rs`, and
  `tests/support/ui.rs` are repointed at it.
- The root `Cargo.toml` auto-merged into a duplicate `googletest` and
  `pretty_assertions` in `[workspace.dependencies]`, which Cargo rejects as a
  duplicate key; the branch's copies are kept and main's are removed.
- `Cargo.lock` starts from main's file and is regenerated against the merged
  manifests; the only package it adds is the path crate `whitaker_lint_core`.

`c57529ce` then removes the `weave explain` comment the merge driver appends
and lets rustfmt order the `whitaker_lint_core` import in the new crate.

Gates on `5cf6e2c5` passed sequentially: `make check-fmt` (10 s), `make lint`
(43 s, interrogate 100%, Clippy clean), `make typecheck` (7 s), `make test`
(2690 s: 2373 nextest tests passed with 6 filtered by the default profile, 84
lint-core tests, and 19 `dylint_testing` tests), `make markdownlint` (34 s), and
`make nixie` (3 s). The filtered six are the installer behaviour binaries that
CI runs under `NEXTEST_PROFILE=ci`; the hosted `linux-full` job is the evidence
for those, not the local default profile.

`857194ba` then extracted the snapshot staging fixture out of
`json_inventory_keeps_its_current_shape`, because the refreshed inline snapshot
had pushed the test over CodeScene's 70-line Large Method advisory. Extracting
it made the helper a free function outside `#[test]`, which is not covered by
`allow-expect-in-tests`, so the six `expect` calls it inherited failed the
denied `expect_used` lint. `2d9d2e9b` makes the helper return `Result` and
propagate with `?`, leaving a single `expect` at the test's own call site where
the exemption applies. Both commits are test-only.

Gates on `2d9d2e9b` passed sequentially: `make check-fmt` (9 s), `make lint` (5
s, warm cache), `make typecheck` (7 s), `make test` (900 s: 2373 nextest tests
passed with 6 filtered by the default profile, 84 lint-core tests, and 19
`dylint_testing` tests), `make markdownlint` (29 s), and `make nixie` (2 s).
One test was reported flaky rather than failed:
`rstest_helper_should_be_fixture::ui example_compiles_without_diagnostics::case_1`
failed to exec the cached dylint driver under
`~/.cache/whitaker-dylint-drivers/dylint-6.3.0/clippy-e4608a25…/` while that
directory was still being populated, and passed on nextest's automatic retry.
That is a driver-cache race, not a candidate failure.

CodeRabbit's approval-response comment on `7f57dd75` raised two findings that
still applied to that head, and both were verified against the source before
repair. The first: `tests/root_install_fixture` resolved the child's data root
from the platform API on Windows and macOS, so the end-to-end install wrote its
installation record into a developer's real data directory. The fixture now
names the data root through a debug-only installer override
(`WHITAKER_INSTALLER_TEST_DATA_HOME`), which keeps the record inside the
fixture's temporary tree on every platform, and the test asserts that the real
record is byte-for-byte unchanged. The second: `recorded_library_paths`
filtered recorded paths with `is_dir()` before `classify_candidate` could
inspect them, so a directory that exists but cannot be read was reported as
"nothing installed". `InstallationRecord::into_absolute_paths` leaves that
distinction to the resolver, whose error branch is now reachable and covered.

**Merge with `main` at `2b7a0189`, 2026-11-09.** `main` advanced to `2b7a0189`
(the six-crate dependency bump) after `04ac2311` and reported the PR as
conflicting, so `origin/main` was merged with `zdiff3` and the Weave driver
bypassed through `core.attributesFile=/dev/null`, matching the earlier merge in
`bfe97b2a`. Twelve paths conflicted and each was resolved from both sides'
intent rather than by taking one wholesale:

- `installer/src/bootstrap/run.rs` keeps this branch's forwarding options
  (`RunOptions::library_path`, `run_inner`, `resolve_library_dir`) and adopts
  main's split, where `execute` and the driver lock moved to
  `bootstrap/driver_run.rs`. `PreparedRun::library_dir` stays an `OsString` so
  a multi-directory override passes through untouched.
- `installer/src/main.rs` stays the 32-line composition root; main's inline
  orchestration block is gone because that code already lives in
  `orchestration`.
- `installer/src/install_flow/mod.rs` keeps main's extracted `prune.rs` and its
  `PruneLibrariesFn` signature. `install_flow/prune.rs` referenced the crate
  through `whitaker_installer::`, which no longer resolves once the module is
  inside the library; it now uses `crate::`.
- `installer/src/tests/dylint_tools.rs` (new on main) had the same problem with
  a second consequence: the crate depends on itself as a dev-dependency for its
  integration tests, so `whitaker_installer::` named the _dev-dependency_ copy
  of the crate and its traits did not match the ones under test. It now imports
  from `crate::`.
- `installer/src/tests/mod.rs` keeps main's `dylint_tools` module and this
  branch's `orchestration` imports.
- `dylint.toml` keeps main's expanded exclusion list with `whitaker` replaced
  by `whitaker_lint_core`.
- `Cargo.toml` takes main's 0.2.12 baseline, keeps this branch's
  `whitaker_lint_core` workspace entry, and keeps `ortho_config` 0.10.0. The
  crate's own manifest moves to 0.2.12 to match.
- `crates/test_must_not_have_example/Cargo.toml` keeps the
  `whitaker_lint_core` dependency.
- `crates/whitaker_lint_core/tests/nextest_ui_filter.rs` adopts main's
  precedence-aware rewrite, relocated to the core crate beside the other test
  harness modules it moved with on this branch. Its five `#[path]` submodules
  moved with it, and `config.rs` resolves `.config/nextest.toml` through
  `workspace_support::workspace_root()` because the crate is now nested under
  `crates/`. The core crate gains main's `proptest` dev-dependency for the
  generated filter-expression cases.
- `.config/nextest.toml` takes main's precedence-safe structure, including the
  whole-disjunction package exclusion and the 120-minute nested-trybuild
  override, plus this branch's
  `test(ortho_config_and_clap_derives_compile_together)` compile contract in
  both profiles.
- `docs/publishing.md` records both main's staged-MSRV detail and this branch's
  package list.
- `tests/workflows/test_ci_workflow.py` names every step of the merged
  `linux-full` lane.
- `Cargo.lock` was taken from `main` and regenerated after the manifests
  settled, per the packaging policy.

`cargo check --workspace --all-targets --all-features` passes on the merged
tree, and the relocated filter guard passes all 44 cases against the merged
configuration.

**Gate repairs on the merged tree.** The full gate runs over the merge reported
five defects, none of which is a conflict hunk, and each was fixed at its cause
rather than suppressed:

- `installer/src/installation_record.rs` spanned 412 lines, past the 400-line
  module cap, because the root-install repair added the record classification
  API. The locked-file helpers (`record_path`, `ensure_parent`, `open_record`,
  `read_paths`, `write_paths`, `record_io_error`) moved to
  `installation_record/store.rs`, and the module became a directory with
  `mod.rs`, `store.rs`, and `tests.rs` so Clippy's `self_named_module_files`
  rule is satisfied. The public API and its doc examples are unchanged.
- CodeScene reported code duplication in `installer/src/dirs.rs`, scoring the
  file 9.39. The two debug-only override resolvers and the XDG bin resolver
  each repeated the same read-an-absolute-environment-path body, and the four
  override tests were near-identical pairs. One `absolute_env_path` helper and
  one `debug_override` helper now serve all three resolvers, and the tests are
  `rstest` cases over a `HomeOverride` selector. A local `cs delta` analysis
  reports the duplication issue fixed and the file back at 10.00.
- The `whitaker ls --json` snapshot still listed `string_continuation_style`,
  which `main` moved from `LINT_CRATES` to `EXPERIMENTAL_LINT_CRATES` in
  `64a5011a`. The suite inventory it describes no longer includes that lint by
  default, so the snapshot drops it instead of the resolver regaining it.
- `installer/src/wrapper_tests.rs` spanned 402 lines once `lint-whitaker`
  reached it, past the same module cap. The two root-policy tests moved to
  `installer/src/wrapper_root_policy_tests.rs`, declared with `#[path]` so the
  file stays flat and Clippy's `self_named_module_files` rule has no
  `wrapper_tests/` directory to object to. The script-execution tests are
  unchanged.
- `main` added `test-root-lib` and `test-nextest-filter` to `make test`, and
  `test-nextest-filter` selected `-p whitaker --test nextest_ui_filter`. This
  branch moved that integration binary to
  `crates/whitaker_lint_core/tests/nextest_ui_filter.rs`, so the target could
  not resolve. The target now selects
  `-p whitaker_lint_core --test nextest_ui_filter --no-default-features`: the
  feature-free shape is the one that links, because `dylint-driver` pulls
  `rustc_private`. The contract test in
  `tests/workflow_contracts/lint_makefile_test.py` and the
  `test-nextest-filter` narrative in `docs/developers-guide.md` follow the same
  package, and the guide's stale `tests/nextest_ui_filter*` paths were
  corrected to the moved location. `test-root-lib` is unchanged and still
  passes 56 of 56.

After these repairs,
`cargo clippy -p whitaker-installer --all-targets --all-features -- -D warnings`
passes, the affected `installation_record` and `dirs` tests pass 24 of 24, the
`whitaker ls` snapshot passes, and the
`ortho_config_and_clap_derives_compile_together` contract passes.

**Merge with `main` at `c404ca4e`, and the fixture repair, 2026-11-09.**
`origin/main` advanced to `c404ca4e` (PR #537, "Fetch once, then run nested
cargo calls --frozen"), so it was merged into the branch head `9b83ac35` with
`zdiff3` and the Weave driver bypassed through `core.attributesFile=/dev/null`,
matching the earlier merges. Fourteen paths conflicted, and each kept whatever
both sides contributed:

- `Makefile` keeps both Nextest passes — the workspace pass and this branch's
  feature-free `whitaker_lint_core` pass — and layers main's prepared metadata,
  offline environment, and driver routing over each. This branch's eight-test
  trybuild filter lists stay, the two `--no-report` coverage passes compose one
  `llvm-cov report`, and `coverage-build` gives its second runner `--no-run` so
  the core pass compiles instead of executing tests there.
- `docs/developers-guide.md` keeps main's ADR 0011 narrative beside this
  branch's two-pass coverage and core-pass explanations.
- `tests/workflows`: main's cargo-locked harness and coverage-environment
  assertions now expect the extra core-pass invocation, in
  `cargo_locked_harness.py`, `test_cargo_locked_makefile.py`, and
  `test_coverage_target_makefile.py`; `test_ci_workflow.py` names every step of
  the merged `linux-full` lane.
- `vendor/dylint_testing/README.md` keeps main's driver documentation and this
  branch's removal and licence paragraph.
- `Cargo.lock` is taken from `main` and regenerated against the merged
  manifests; the rebuilt lock is byte-identical to the automatic merge, which
  confirms the resolution.
- The remaining paths keep both sides' additions: the CI workflow, the
  `whitaker_lint_core` toolchain helper, the four documentation indexes and
  guides, and the suite design document.

The merge commit is `afaa78ac`. `make check-fmt`, `make lint`, and
`make typecheck` passed on it, and `make test` then failed on
`whitaker::root_install_e2e::root_install_stages_the_suite_and_records_its_directory`.
The failure is not a conflict artefact: the fixture redirects `HOME`, so
`rustup` derived `CARGO_HOME` inside the temporary home, installed the pinned
channel there, and aborted with `rustup is not installed at '<temp>/.cargo'`.
The test had been skipping on this host because the pinned nightly was absent;
that toolchain is now installed, so the install path ran for real for the first
time and exposed the latent fixture bug. A minimal reproduction outside the
suite (`HOME=<temp> rustup toolchain install nightly-...`) fails the same way.

`4a96209c` passes the host's `CARGO_HOME` and `RUSTUP_HOME` through to the
child, resolving each from its variable or from the conventional directory
under the user's home, so the child sees the toolchain the fixture probed
before deciding to run. The resolution lives in a new
`tests/root_install_fixture/host_home.rs` with unit tests for the precedence,
and the stub `cargo` moves to `tests/root_install_fixture/stub_cargo.rs`; the
fixture module drops from 453 lines to 374, inside the 400-line cap.

Gates on `4a96209c` passed sequentially: `make check-fmt` (9 s), `make lint`
(76 s), `make typecheck` (34 s), and `make test` (3,466 s: 2,437 Nextest tests
passed with 6 filtered by the default profile, 128 lint-core tests, 58 and 14
`dylint_testing` tests, 56 `test-root-lib` tests, and the filter guard). The
previously failing test passes in 3.858 s. Plan stays `IN PROGRESS`; crates.io
reservation remains unauthorized.

- **Packaging gate repair, 2026-11-09.** Hosted CI on `1140f56c` ran
  `linux-full` to step 35, "Package the root CLI", and failed there; the job
  was still in progress, so the failing log was not yet downloadable. The local
  reproduction was exact: `cargo package --locked -p whitaker --no-verify`
  reports that it cannot select a version for the requirement
  `whitaker-installer = "^0.2.12"`, and the candidates are 0.2.11 and earlier.
  The diagnosis is version skew, not a branch defect: `main` at `c404ca4e` runs
  no such step (its `linux-full` step list has no packaging step and passes),
  and its `docs/publishing.md` records that `whitaker` cannot be packaged until
  the release publishes the installer. The step now runs
  `make package-root-cli`, whose command-scoped `[patch.crates-io]` overrides
  point the installer and common requirements at the workspace sources;
  `cargo package --locked -p whitaker --no-verify` under those overrides packs
  21 files and exits 0. The workflow contract
  (`tests/workflow_contracts/root_package_contract_test.py`) now pins the
  step's command and asserts the recipe keeps `$(CARGO_LOCKED)` and both
  patches, and its docstring no longer claims a `whitaker_lint_core` dependency
  the root manifest does not declare.

To be completed at each milestone boundary and at completion. Before setting
this plan to `COMPLETE`, reconcile every entry in `Surprises & discoveries`
against `docs/whitaker-cli-design.md`. In particular, `CLI-REQ-FWD` has no
upstream source today and **must** be added to the design document, since
forwarding is a public behaviour carrying a compatibility commitment. Do not
mark the plan `COMPLETE` while any upstream change or deviation is unrecorded.

## Revision note

Revisions 2–3 and D-6–D-12 record design decisions and the package baseline.
