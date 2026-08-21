# Add a root `whitaker` binary behind an internal library boundary

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

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

The invisible half of the outcome is the reason the work is worth doing, and
it is larger than it first appears. Two structural problems block a root
binary today, and both are fixed here:

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

- [ ] `EP-M0` — extract the Dylint driver library; root package becomes a
  publishable, testable CLI package.
- [ ] `EP-M1` — installer orchestration moved behind the library boundary.
- [ ] `EP-M2` — the `whitaker` binary: `install`, `ls`, and `cargo dylint`
  forwarding.
- [ ] `EP-M3` — binstall metadata and crates.io name reservation.
- [ ] `EP-M4` — ADR `docs/adr-005-whitaker-cli-boundary.md` and documentation.

Record an ISO-8601 UTC timestamp against each item as it completes, for
example `- [x] (2026-08-21T14:05Z) EP-M0 …`. Split any partially completed
item into "done" and "remaining" rather than leaving it ambiguous.

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

1. **`whitaker-installer` keeps working unchanged.** Its command-line surface,
   exit codes, and output must be identical before and after. It is a released
   binary documented in `docs/users-guide.md`, published to GitHub Releases,
   exercised by `make install-smoke` and `installer/tests/`. Its deprecation
   is roadmap 3.9.1, not this plan. This is not a shim invented to make a
   milestone viable; it is the shipping product.
2. **`whitaker --all` keeps working.** The wrapper script's contract is
   documented at `docs/users-guide.md:25-27` and consumed by `Makefile:185`
   and `.github/workflows/ci.yml:154`. Named consumers: existing users, and
   this repository's own lint gate.
3. **The public library surface of `whitaker_installer` must not shrink.**
   Integration tests under `installer/tests/` compile against it as an
   external crate and see only `pub` items; `installer/Cargo.toml:29-55` gates
   `StubExecutor` and `InstallerError::StubMismatch` behind the `test-support`
   feature, which roadmap 3.2.2 declares supported. Additions are fine;
   removals and signature narrowings are not.
4. **The root package stays named `whitaker` at version `0.2.7`**, and the new
   binary is named `whitaker`.
5. **The Dylint lint crates must keep building and their UI tests passing**
   throughout `EP-M0`. They are the oracle for that milestone.
6. **No new lint suppressions.** `Cargo.toml [workspace.lints]` sets
   `unsafe_code = "forbid"`, `missing_docs = "deny"`, `allow_attributes =
   "deny"`, clippy `pedantic` at warn with `-D warnings`. Adding `#[allow(…)]`
   to pass a gate is a tolerance breach. Adding an `excluded_crates` entry to
   `dylint.toml` counts as a suppression for this purpose and requires the
   same escalation.
7. **No file may exceed 400 lines** (`AGENTS.md`). Every module needs a `//!`
   doc comment.
8. **No direct environment mutation in tests** (`AGENTS.md`). Use `temp-env`
   or inject through a port.
9. **The prebuilt-artefact path must not change behaviour.** Governed by
   `docs/adr-001-prebuilt-dylint-libraries.md`.
10. **Caret dependency requirements only**; no `*` or `>=`.

## Tolerances (exception triggers)

Stop and escalate; do not improvise.

- **Scope.** More than 70 files changed, or more than 2,500 net added lines.
  `EP-M0` alone touches roughly 40 files, almost all mechanically.
- **`EP-M0` gate.** If `cargo package -p whitaker --no-verify` still fails
  after the extraction, stop. That command is the milestone's whole point.
- **Interface.** If anything requires removing or narrowing an existing `pub`
  item in `whitaker_installer`, stop (Constraint 3). Additive `*_for`
  functions are the sanctioned pattern.
- **Behaviour drift.** If any existing test under `installer/tests/` or any
  Dylint UI fixture needs its _assertions_ changed, stop. Relocation is fine;
  changed expectations are evidence of a Constraint 1 or 5 breach.
- **Dependencies.** Three new dependencies are pre-authorized: `ortho_config`,
  `googletest`, `pretty_assertions`. A fourth triggers escalation.
- **Iterations.** Three failed fix attempts on one root cause; report the log
  path.
- **Verification.** Kani over 15 minutes for this plan's harnesses in
  aggregate, or Verus over 10 minutes, or more than one working day spent
  authoring either. These are the repository's first sequence-shaped proofs;
  budget overrun is the expected failure mode.
- **Ambiguity.** If `docs/whitaker-cli-design.md` and `docs/roadmap.md`
  disagree on whether a behaviour belongs to 3.5.1, stop and present both
  readings. This fired twice during planning; see `Decision log` D-2 and D-3.

## Risks

**R1 — `EP-M0` is a wide mechanical rename that can break the lint suite.**
Severity: high. Likelihood: medium.
Eleven lint-crate manifests and roughly 25 `use whitaker::` sites move. Note
`crates/test_must_not_have_example/Cargo.toml:34` uses a literal path
dependency, not the `workspace = true` alias, so a global search-and-replace
on the alias misses it.
Mitigation: the Dylint UI suite already exercises every affected crate. Run it
before and after and require identical results. Do the extraction as one
commit so bisection is clean.

**R2 — Argument forwarding can swallow a real subcommand.**
Severity: high. Likelihood: medium.
The binary must decide, from argv alone, whether to dispatch a subcommand or
forward to `cargo dylint`. Get it wrong and `whitaker install` silently runs
`cargo dylint install`, or `whitaker --all` prints a usage error.
Mitigation: this is the plan's one genuinely new decision procedure, and it is
the subject of both verification obligations — `EP-INV-DISPATCH` (Kani) and
`EP-LEM-DISPATCH` (Verus).

**R3 — Index-based BDD scenario bindings break silently.**
Severity: medium. Likelihood: medium.
`#[scenario(path = "…", index = N)]` binds by position; inserting a scenario
mid-file rebinds every later one, often still passing. See the warning at
`installer/tests/behaviour_cli/scenarios.rs:7`.
Mitigation: new scenarios go only in new feature files.

**R4 — A new test binary named `behaviour_cli` is skipped by default.**
Severity: medium. Likelihood: high if unaddressed.
`.config/nextest.toml` sets
`default-filter = "not (binary(behaviour_toolchain) | binary(behaviour_cli) | kind(example))"`,
and nextest's `binary()` matches by binary _name_, not package-qualified name.
Mitigation: name the new file `behaviour_whitaker.rs`, and run
`make test NEXTEST_PROFILE=ci` at every milestone boundary.

**R5 — `--help` and `--version` may exit non-zero.**
Severity: medium. Likelihood: medium.
clap reports both as `Err`. Routing every `Err` to a failure code makes
`whitaker --help` fail.
Mitigation: `EP-INV-EXIT`, asserted end-to-end on the real process.

**R6 — Publishing instructions may precede name reservation.**
Severity: medium. Likelihood: low, but severe if it lands.
The name `whitaker` is unregistered on crates.io. If `EP-M4` publishes
`cargo install whitaker` before the name is claimed, and 3.5.3 slips, users
following official documentation install someone else's crate.
Mitigation: `EP-M3` reserves the name; `EP-M4` is explicitly gated behind it.

**R7 — `ortho_config` brings a second `toml` stack.**
Severity: low. Likelihood: certain.
`ortho_config` 0.9.0 reaches `figment` → `toml 0.8` + `toml_edit 0.22`
alongside the workspace's existing `toml 1.x` + `toml_edit 0.25`, and enables
`figment`'s `test` feature (pulling `tempfile` and `parking_lot`) into the
production graph. Roughly 26 genuinely new lock entries; about 80% of the
closure is already present.
Mitigation: after `EP-M0` the root package is _not_ a dependency of the lint
crates, so this cost is confined to the CLI binary. Record the stripped
release size in `EP-M3` rather than guessing.

**R8 — Removing `--exclude whitaker` switches on six never-run test binaries.**
Severity: medium. Likelihood: high.
`tests/{build_config,config_loading,lint_template,locale_resolution,nextest_ui_filter,ui_harness}.rs`
total roughly 1,045 lines and have never run under `make test`, because
`Makefile:23` excludes the package. `tests/ui_harness.rs` drives the Dylint UI
harness under `RUSTFLAGS="-C prefer-dynamic -Z force-unstable-if-unmarked
-D warnings"`.
Mitigation: treat this as its own step inside `EP-M0` with its own validation,
not a one-line edit. Note `Makefile:64-68` `DOCTEST_EXCLUDES` carries a
_second_ `--exclude whitaker`; decide both explicitly. `make coverage`
(`Makefile:151`) reuses the same recipe, so a bad landing reddens two CI jobs.

## Context and orientation

You have only this repository and this document. Run everything from the
repository root; obtain it with `git rev-parse --show-toplevel`.

### The root package today

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

Under that feature the library links `rustc_driver`. A comment in the same
file records the consequence — "duplicated `std`/`core` link errors seen
during all-features test runs" — and that is why `Makefile:23` excludes the
package from the test run.

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
anything added to its `[dependencies]` propagates to all of them — Cargo has
no per-target dependency tables.

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
`generate_and_report_wrapper`, `ensure_whitaker_workspace`,
`resolve_toolchain`, `ensure_toolchain_installed`) they hold the real
orchestration.

Useful detail for `EP-M1`: items inside `installer/src/install_flow/mod.rs`
are already `pub(crate)`, and `PrebuiltInstallationHooks` at `:136` is fully
private. Once the module sits inside the library, `pub(crate)` already means
library-internal, so only a small named facade needs to become `pub`. Do not
over-promote.

### The wrapper script — the contested name

`installer/src/wrapper.rs:105` writes `bin_dir.join("whitaker")`, an
executable whose body is:

```bash
export DYLINT_LIBRARY_PATH="{library_path}"
exec cargo dylint "$@"
```

This is not a convenience. It is currently the only way to run Whitaker's
lints. It is documented (`docs/users-guide.md:25-27, 51-57`), wired into
`Makefile:74-75` and `:185`, and run by `.github/workflows/ci.yml:154`.

`Makefile:9` prepends `$(HOME)/.cargo/bin` to `PATH` while `Makefile:6`
appends `$HOME/.local/bin`. So a `cargo install`ed binary at
`~/.cargo/bin/whitaker` **deterministically wins** over the script at
`~/.local/bin/whitaker`. `Makefile:99-139` wraps the whole test run in a
`trap`-based backup and restore of that file, failing loudly on modification —
somebody has already been burned by it.

That is why this plan forwards unrecognized arguments rather than rejecting
them (`Decision log` D-2).

### Existing dependency-injection seams

The installer already has seams, in three styles for one concern:

| Trait or seam | Defined at | Style |
| --- | --- | --- |
| `deps::CommandExecutor` | `installer/src/deps/mod.rs:36` | public trait, `mockall` |
| `toolchain::CommandRunner` | `installer/src/toolchain/mod.rs:44` | **private** trait, `mockall`, same shape |
| `dirs::BaseDirs` | `installer/src/dirs.rs:41` | public trait, `mockall` |
| `builder::CrateBuilder` | `installer/src/builder.rs:58` | public trait, `mockall` |
| `artefact::download::ArtefactDownloader` | `installer/src/artefact/download.rs:29` | public trait, `mockall` |
| `install_flow::PrebuiltInstallationHooks` | `installer/src/install_flow/mod.rs:136` | bare `fn` pointers |

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
  `docs/developers-guide.md:737` — run by `scripts/run-kani.sh` via
  `make kani`.
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
`make markdownlint` also runs a `typos` spelling gate enforcing Oxford
spelling (`-ize`, but `behaviour` keeps `-our`), and `typos` only inspects
git-tracked files — an untracked draft passes and then fails once committed.

## Conformance basis

No Terms of Reference document exists. Upstream artefacts:

- **Design:** `docs/whitaker-cli-design.md` at the revision in the working
  tree, §Public CLI surface and §Compatibility and migration. Cited as
  `CLI-DESIGN`.
- **Roadmap:** `docs/roadmap.md` item 3.5.1 (line 145). Its prerequisite,
  3.2.1 (line 108), is done.
- **ADRs:** `docs/adr-001-prebuilt-dylint-libraries.md` constrains the
  prebuilt path. No ADR covers the CLI boundary; this plan creates
  `docs/adr-005-whitaker-cli-boundary.md`.
- **Standards:** `AGENTS.md`, `docs/documentation-style-guide.md`.

| ID | Statement | Source |
| --- | --- | --- |
| `CLI-REQ-BIN` | "Add a real `whitaker` binary at the root package" | `CLI-DESIGN` §Compatibility and migration, step 1 |
| `CLI-REQ-LIB` | "move the current installer logic behind an internal library boundary" | ditto |
| `CLI-REQ-BINSTALL` | "copy the working `cargo-binstall` metadata pattern from `whitaker-installer` onto `whitaker`" | ditto |
| `CLI-REQ-LS` | "`whitaker-ls` disappears in favour of `whitaker ls`" | `CLI-DESIGN` §Public CLI surface |
| `CLI-REQ-L10N` | "Every human-facing string, including `--help` … should be localizable"; command names are never translated | `CLI-DESIGN` §Accessibility and localization |
| `CLI-REQ-FWD` | Derived from Constraint 2; no upstream text. Recorded as a deviation in `Decision log` D-2 and added to `CLI-DESIGN` in `EP-M4`. | this plan |

_Table 2: Upstream requirements traced by this plan._

```plaintext
CLI-REQ-BIN      -> EP-M0, EP-M2 -> tests::behaviour_whitaker::help_lists_subcommands
CLI-REQ-LIB      -> EP-M1        -> installer/tests/ unchanged + EP-INV-PARITY
CLI-REQ-BINSTALL -> EP-M3        -> tests::behaviour_binstall::whitaker_package_metadata
CLI-REQ-LS       -> EP-M2        -> tests::behaviour_whitaker::ls_text_and_json
CLI-REQ-L10N     -> EP-M2        -> localized parse entry point; NoOpLocalizer
CLI-REQ-FWD      -> EP-M2        -> EP-INV-DISPATCH (Kani) + EP-LEM-DISPATCH (Verus)
```

Explicitly **not** discharged here, with owning roadmap items: `whitaker
check` (3.5.2); release-artefact publishing (3.5.3); rule codes and selector
precedence (3.6.1, 3.6.2); `whitaker.toml`, `dylint.toml` bridging and
`DYLINT_*` migration (3.6.3); `--locale`/`--colour`/`--progress` (3.6.4); the
`--build-only` → `--build-from-source` rename and `--offline` (3.7.1); bundle
manifests (3.7.3); `doctor` and failure recording (3.8.x); the full `ls`
surface (3.8.1, 3.8.2); the `whitaker-installer` deprecation shim and `list`
alias (3.9.1); wrapper-script and `--skip-wrapper` removal (3.9.3).

## Verification plan

The first draft proposed a Verus proof of release-asset-name injectivity and a
Kani harness proving that a routing function rejects two conflicting flag
pairs. Both were **vacuous** and have been cut. Nothing in this repository
ever inverts an asset name — `cargo-binstall` composes by literal
substitution, and a grep for `rsplit_once|splitn|strip_prefix` across
`installer/src/` finds nothing — so injectivity of a never-inverted function
is load-bearing for nothing. And both conflict pairs are enforced by clap
attributes (`installer/src/cli.rs:108`, `:113`, `:131`), which reject the
input during parsing, so a routing function can never receive one; proving it
rejects them proves a property of an unreachable branch. Recording this is
required by the ExecPlan discipline: a passing check that cannot fail is not
evidence.

What replaced them is a genuinely new obligation. The forwarding behaviour
(`CLI-REQ-FWD`) introduces a decision procedure that did not previously exist
and that no third-party library owns: given argv, dispatch a subcommand or
forward to `cargo dylint`. Getting it wrong silently breaks either the new CLI
or every existing user. That is worth verifying properly.

### Axioms (assumed, not verified)

- `clap` 4.5 parses argv per its documented derive semantics.
- `ortho_config` 0.9.0's `LocalizedParse::try_parse_localized_from` and
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
  strings: an enum over `{Install, Ls, Help, Version, DoubleDash, GlobalFlag,
  DylintFlag, Other}`, length bounded at 6 — under 10^5 states.
  `#[kani::unwind(7)]`, one greater than the maximum iteration count.
- **Siting.** `classify` and its token enum live in `src/cli/dispatch.rs` in
  the root package and take the token enum rather than `clap::Cli`, so the
  harness compiles a small pure module. This is deliberate: siting Kani where
  it must codegen `clap` plus `ortho_config` plus `whitaker_installer` would
  be a roughly 250-crate graph, and a symbolic `Vec<String>` hits the
  documented heap cliff. Tokenizing real argv is a separate, testable function
  that is _not_ part of the harness.
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
  concrete argv that names `ls` yet classifies as `Forward`. Restore and
  record the transcript.

### `EP-LEM-DISPATCH` — classification is total for unbounded argv

- **Obligation.** For every finite token sequence of _any_ length, exactly one
  classification applies. Kani bounds length at 6; this closes the tail.
- **Method.** Formal proof (Verus) over `Seq<TokenTag>`.
- **Rationale.** The guarantee must hold for all admissible inputs — a user's
  `cargo dylint` invocation has no length bound. Bounded checking cannot
  establish it. The proof is a genuine mutual-exclusivity and exhaustiveness
  argument over the guard predicates, not a restatement.
- **Domain.** Unbounded `Seq<TokenTag>`. This would be the repository's first
  sequence-shaped proof; every file in `verus/` today is numeric or
  vector-algebraic. Budget accordingly and respect the verification tolerance.
- **Artefact.** `verus/whitaker_cli_dispatch.rs`, added as a new `cli` group
  in `scripts/run-verus.sh` — which needs **both** the `case` at `:10-26` and
  the second `case` at `:53-54` edited, not one.
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
  localized-parse arm must therefore use clap's own code, not a blanket
  failure code.
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
   literal path dependency at
   `crates/test_must_not_have_example/Cargo.toml:34`.
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
   decide `DOCTEST_EXCLUDES` (`Makefile:64-68`) explicitly. Expect R8: six
   test binaries begin running for the first time. Fix what they surface; if
   any fails for a pre-existing reason unrelated to this plan, record it in
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

must succeed. Then `make check-fmt && make typecheck && make lint &&
make test NEXTEST_PROFILE=ci`, and the Dylint UI suite must be unchanged.

Then answer the link question the first draft got wrong. `cargo check` does
**not** invoke the linker, so it cannot observe duplicate `std`/`core`
symbols. Use:

```console
cargo build --workspace --bins --all-features
cargo test -p whitaker --all-features --no-run
```

under the same `RUSTFLAGS="-C prefer-dynamic -Z force-unstable-if-unmarked
-D warnings"` the test recipe uses. After the extraction the root package no
longer enables `rustc_private` at all, so this should be clean; if it is not,
stop and escalate.

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
   `installer/src/lib.rs`, move `installer/src/install_flow/` into the
   library, and move the ten `main.rs` orchestration functions into a new
   `installer/src/orchestration/` tree, each file under 400 lines.
   `installer/src/main.rs` becomes a thin composition root.
   `try_fast_path_installation` calls
   `staged_suite::try_test_staged_suite_installation`
   (`installer/src/main.rs:87`), so the fast path and the staged-suite hook
   move together; keep the hook behind its existing `#[cfg(debug_assertions)]`
   and `test_support` gating and expose only the facade the fast path needs.
   `installer/src/main.rs` also carries `#[cfg(test)] mod tests;` at `:402`;
   those unit tests move with their subjects. That is relocation, not an
   assertion change, and does not trip the behaviour-drift tolerance.
   **No behaviour changes.** Validate with `make test NEXTEST_PROFILE=ci` —
   not plain `make test`, which skips `behaviour_cli` and
   `behaviour_toolchain`, the very binaries that would detect a dispatch
   regression.

2. **Wrapper generation becomes a parameter.** Give
   `generate_and_report_wrapper` an argument naming which scripts to write.
   `whitaker-installer` passes `{whitaker, whitaker-ls}`, preserving
   Constraint 1 exactly. `whitaker install` passes `{whitaker-ls}` — it must
   not overwrite the binary the user just invoked.

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
   `ortho_config`, dispatch, map to an exit code. Target under 60 lines.
   Declare `[[bin]] name = "whitaker"` with `autobins = false`, matching
   `installer/Cargo.toml:11-27`.

6. **binstall metadata, additively.** Do **not** change the signatures of
   `expand_pkg_url` or `expand_bin_dir` (`installer/src/binstall_metadata.rs:45`,
   `:73`) — they are consumed by
   `installer/tests/behaviour_installer_release.rs:155`,
   `installer/tests/behaviour_binstall.rs:10-18`,
   `installer/src/installer_packaging_tests.rs:274`, and their own doctests,
   so narrowing them trips the interface tolerance. Add
   `expand_pkg_url_for(package, version, target)` and
   `expand_bin_dir_for(package, version, target)`, and reimplement the
   existing two-argument forms as delegates. `load_cargo_toml` resolves the
   manifest through `env!("CARGO_MANIFEST_DIR")`, baked to `installer/` at
   compile time, so give it a `&Path` parameter for the root package's test to
   use. Then add `[package.metadata.binstall]` to the root `Cargo.toml`
   mirroring `installer/Cargo.toml:95-101`, including the
   `overrides.x86_64-pc-windows-msvc` entry with `pkg-fmt = "zip"`.
   `installer/src/installer_packaging.rs:31` has the same hardcoding and is
   deliberately **not** touched; it belongs to 3.5.3.

Validate after each step: `make check-fmt && make typecheck && make lint &&
make test NEXTEST_PROFILE=ci`, captured with `tee`. Commit after each.

### Stage D — verification, reservation, documentation

1. Turn the Kani harnesses green; confirm the three reachability harnesses
   each report a counterexample; run the negative control and record it.
2. Turn the Verus proof green; run the negative control and record it.
3. Run the proptest pairwise classification report; confirm every pair is
   exercised.
4. Reserve the crates.io name (`EP-M3`) **before** any documentation tells
   users to install it.
5. Write `docs/adr-005-whitaker-cli-boundary.md` using the template at
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
  metadata, is covered by `make test`, and `cargo package -p whitaker
  --no-verify` succeeds. No binary yet.
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
  `whitaker --help` exits 0; `whitaker --all --version` reaches
  `cargo dylint`; `whitaker install` does not write a `whitaker` script.
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
  State this plainly in the ADR and the user guide; do not imply binstall
  works end to end.
- **Compatibility decision.** None.

### `EP-M4` — documented

- **Outcome.** ADR and documentation reflect reality.
- **Acceptance evidence.** `make markdownlint` and `make nixie` pass; roadmap
  item 3.5.1 ticked.
- **Required ADR content** (`docs/adr-005-whitaker-cli-boundary.md`): the
  `PATH` namespace decision and why forwarding was chosen over the
  alternatives; the promoted `whitaker_installer` public surface; the rule
  that `whitaker_installer` owns _how_ to install while the CLI owns _whether
  and when_, so 3.5.2's lazy repair has an unambiguous home; and the plain
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
ortho_config = "0.9.0"
googletest = "0.14.3"
pretty_assertions = "1.4.1"
```

`ortho_config` is a normal dependency of the root package only. After `EP-M0`
the lint crates depend on `whitaker_lint_core`, not on `whitaker`, so this
subtree does not propagate into the fifty cross-compiled lint-crate builds in
`rolling-release.yml`. That containment is a direct benefit of `EP-M0` and
should be stated in the ADR.

`insta` is already a workspace dependency; add it as a root dev-dependency.

**`googletest` ordering rule.** `#[gtest]` must come **before** `#[rstest]`,
or the test registers and runs twice. Document in
`docs/developers-guide.md`:

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

`LintInventory` returns a `StagedInventory` struct rather than a bare `Vec`,
so 3.7.3 and 3.8.1 can add manifest fields without replacing the trait.
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
exit 1 where the installer exits 2, breaking `EP-INV-PARITY`. Confirm the
exact signature of `is_display_request` against
`docs/ortho-config-users-guide.md:333` before Stage B writes red tests against
it: if it takes `&OrthoError` rather than `&clap::Error`, the guard needs
adjusting.

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
generated wrapper script, and asserting that `--help`, `ls --json`, and
`--all` all behave. That is the only mechanism that would catch a forwarding
regression before users do.

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
2. `whitaker ls --json` prints the same JSON as `whitaker-installer list
   --json`. Ordering is deterministic — `installer/src/list.rs:50` sorts by
   crate name and `InstalledLints.by_toolchain` is a `BTreeMap` — so the
   snapshot is stable. The snapshot is a **regression guard, not a
   compatibility promise**; note that inline.
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
- **Cost.** Record `cargo tree -e normal -p module_max_lines --features
  dylint-driver | wc -l` before and after; after `EP-M0` it should _fall_.
  Record the stripped size of the `whitaker` binary.

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

| Document | Why |
| --- | --- |
| `docs/whitaker-cli-design.md` | The specification. §Public CLI surface and §Compatibility and migration are normative. |
| `docs/roadmap.md` | Items 3.5.1 to 3.9.3 — what belongs here and what does not. |
| `docs/users-guide.md` | The `whitaker --all` contract this plan must preserve; updated in `EP-M4`. |
| `docs/developers-guide.md` | Installer architecture, Kani harness conventions, the Verus trust boundary. |
| `docs/publishing.md` | Publish order, updated in `EP-M4` once the root package is publishable. |
| `docs/repository-layout.md` | Updated for `crates/whitaker_lint_core`. |
| `docs/ortho-config-users-guide.md` | Layering, subcommand merging, the localization API. |
| `docs/rstest-bdd-users-guide.md` | `#[scenario]` bindings and step functions. |
| `docs/rust-testing-with-rstest-fixtures.md` | Fixture patterns for the `CliWorld` fixture. |
| `docs/rust-doctest-dry-guide.md` | Doctests are gated; keep them DRY. |
| `docs/complexity-antipatterns-and-refactoring-strategies.md` | The suite lints this repository against itself; keep `classify` flat. |
| `docs/whitaker-dylint-suite-design.md` | Workspace layout, updated in `EP-M4`. |
| `docs/whitaker-clone-detector-design.md` | Confirms `whitaker_clones_core`/`whitaker_sarif` naming so new crate names do not collide. |
| `docs/documentation-style-guide.md` | ADR template and naming; Oxford spelling. |
| `docs/adr-001-prebuilt-dylint-libraries.md` | The prebuilt path this plan must not disturb. |
| `AGENTS.md` | Gates, commit rules, the 400-line limit, test-environment rules. |

_Table 3: Documentation to read before starting._

Skills: `leta` for symbol navigation instead of grep; `hexagonal-architecture`
for the port boundaries; `kani` for `EP-INV-DISPATCH`; `verus` for
`EP-LEM-DISPATCH`; `proptest` for `EP-INV-PARITY`; `rust-unit-testing` for
`googletest` and `insta` style; `execplans` for keeping this document current.

## Surprises & discoveries

- Observation: the root `whitaker` package cannot be published at all.
  Evidence: `cargo package -p whitaker --no-verify --allow-dirty` fails with
  "no matching package named `rustc_ast`"; the four `rustc_*` shim crates are
  `publish = false`; the manifest has no `description`, `license`, or
  `repository`.
  Impact: decisive. Turned the driver-library extraction from a contingency
  into `EP-M0`, the plan's precondition, and made a separate CLI crate
  unnecessary.

- Observation: the installer writes an executable named `whitaker` into the
  user's binary directory, and a `cargo install`ed binary deterministically
  shadows it.
  Evidence: `installer/src/wrapper.rs:105`; `Makefile:9` prepends
  `~/.cargo/bin` while `Makefile:6` appends `~/.local/bin`; `Makefile:99-139`
  backs the script up around every test run; `Makefile:185` and `ci.yml:154`
  invoke `whitaker --all`.
  Impact: without forwarding, this change would break this repository's own
  lint gate and every user's primary workflow, with no replacement until
  3.5.2. Drove `CLI-REQ-FWD` and Constraint 2.

- Observation: `env!("CARGO_BIN_EXE_<name>")` is defined only for integration
  tests of the package declaring the binary.
  Evidence: all four existing uses are package-local, for example
  `installer/tests/behaviour_cli/support.rs:206`.
  Impact: end-to-end tests must live in the root package, which is why
  removing `--exclude whitaker` is a required part of `EP-M0` rather than an
  optional tidy-up.

- Observation: `cargo check` does not invoke the linker.
  Evidence: the risk being tested is a _link_ error, per `src/lib.rs:4-6`.
  Impact: the first draft's feasibility spike used `cargo check` and so could
  never observe the failure it existed to detect. Replaced with
  `cargo build --bins` and `cargo test --no-run`.

- Observation: both documented flag conflicts are enforced by clap, not by
  repository-owned code.
  Evidence: `conflicts_with` at `installer/src/cli.rs:108`, `:113`, `:131`.
  Impact: a routing function can never receive a conflicting input, so the
  first draft's Kani obligation was vacuous. Cut.

- Observation: nothing in the repository ever parses a release-asset name.
  Evidence: `installer/src/binstall_metadata.rs` composes with `.replace()`;
  no `rsplit_once`, `splitn`, or `strip_prefix` under `installer/src/`.
  Impact: the first draft's Verus injectivity proof was load-bearing for
  nothing. Cut and replaced with a finite disjointness test over the
  ten-element published namespace.

- Observation: nextest's `binary()` filter matches by binary name, not
  package-qualified name.
  Evidence: `.config/nextest.toml` `default-filter` excludes
  `binary(behaviour_cli)`.
  Impact: a new `behaviour_cli.rs` in any package would be silently skipped.
  Drove the `behaviour_whitaker.rs` naming and the `NEXTEST_PROFILE=ci`
  discipline.

- Observation: `#[gtest]` must precede `#[rstest]` or the test runs twice.
  Evidence: `googletest` 0.14.3 documentation.
  Impact: recorded as a convention for `docs/developers-guide.md`.

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
  `cargo-dylint` solves the same contamination the same way.
  Approved by the requester before drafting. Date/Author: 2026-08-21.

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
  as a rename of `whitaker-installer list`, with its output marked a
  regression guard rather than a compatibility promise, and `EP-M2` recording
  that 3.8.1 and 3.8.2 will replace it wholesale. Approved by the requester.
  Date/Author: 2026-08-21.

- **D-4: Exit codes are `0` success and display, `2` usage error, `1`
  operational failure.** This is not a free choice: `Cli::parse()` in
  `installer/src/main.rs:42` already exits 2 via clap on usage errors, and
  Constraint 1 plus `EP-INV-PARITY` require matching it. The first draft left
  this undecided and its sample code would have produced 1.
  Date/Author: 2026-08-21, planning agent.

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

- **D-7: Use `googletest` and `pretty_assertions` in new test files only, and
  flag the inconsistency.** The task brief authorizes both. However
  `docs/execplans/7-3-1-map-candidate-spans-and-extract-ast-feature-vectors.md`
  records the opposite decision for a sibling item — follow the in-repo
  `assert_eq!` plus `insta` idiom, on the reading that the brief lists these as
  available tools rather than mandates — and neither crate appears anywhere in
  the workspace today. Adopting them here makes the repository inconsistent
  with itself. This plan follows the brief but raises the conflict for the
  reviewer to settle; if consistency is preferred, cut both and use
  `assert_eq!` plus `insta`, which covers every assertion described here.
  Date/Author: 2026-08-21, planning agent.

- **D-8: Cut the release-asset-name Verus proof and the routing-conflict Kani
  harness; verify argv classification instead.** Both original obligations were
  vacuous — see `Verification plan` and `Surprises & discoveries` for the
  evidence. The forwarding behaviour introduced by D-2 creates a genuinely new
  repository-owned decision procedure whose failure modes are severe in both
  directions, which is what the two verification tools now target.
  Date/Author: 2026-08-21, planning agent.

- **D-9: The plan file is named `3-5-1-root-whitaker-binary.md`.** The task
  brief gave a filename referencing roadmap 6.5.1, a different item. The task
  body, branch name, and required pull-request title all identify 3.5.1.
  Confirmed with the requester. Date/Author: 2026-08-21.

## Outcomes & retrospective

To be completed at each milestone boundary and at completion. Before setting
this plan to `COMPLETE`, reconcile every entry in `Surprises & discoveries`
against `docs/whitaker-cli-design.md`. In particular, `CLI-REQ-FWD` has no
upstream source today and **must** be added to the design document, since
forwarding is a public behaviour carrying a compatibility commitment. Do not
mark the plan `COMPLETE` while any upstream change or deviation is unrecorded.

## Revision note

**Revision 2, 2026-08-21.** Rewritten after a six-perspective design review.

What changed, and why. The structural bet was wrong: `cargo package -p
whitaker` fails, so the root package cannot be published and the plan's stated
outcome was unreachable. The driver-library extraction, previously filed as a
contingency under a risk entry, is now `EP-M0` and the plan's precondition;
the separate `crates/whitaker_cli` crate it was designed to avoid is gone,
because the extraction makes the root package a normal, testable, publishable
CLI package. That also fixed three downstream defects: `CARGO_BIN_EXE_whitaker`
is now available to the end-to-end tests, the policy layer no longer imports
clap through a supposedly pure domain boundary, and `ortho_config` no longer
propagates into eleven lint crates.

A collision the first draft missed entirely — the installer generates an
executable named `whitaker` that a `cargo install`ed binary deterministically
shadows, breaking `whitaker --all` and this repository's own `make lint` — is
now Constraint 2, requirement `CLI-REQ-FWD`, and the subject of both
verification obligations.

Both original verification obligations were cut as vacuous: nothing ever
inverts an asset name, and clap rejects the flag conflicts before any
repository-owned routing code runs. They are replaced by argv-classification
totality and disjointness, bounded by Kani and closed unbounded by Verus. Exit
codes are settled at 0, 2, and 1 rather than left to the implementer. The
feasibility spike now uses `cargo build` rather than `cargo check`, which
cannot link and so could never have observed the failure it tested for.

How it affects remaining work. `EP-M0` is new and is roughly 40 mechanical
files; the overall scope tolerance rose from 45 to 70 files to accommodate it.
`EP-M1` is unchanged. Three items still want a reviewer's eye: the
`googletest` and `pretty_assertions` inconsistency with the sibling plan for
7.3.1 (D-7), whether `ortho_config` earns its place at this milestone given
that `NoOpLocalizer` does not translate (D-6), and confirmation of
`is_display_request`'s exact signature before Stage B writes red tests against
it.
