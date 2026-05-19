# Create the `rstest_helper_should_be_fixture` lint crate

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
 `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 8.2.1 starts the first `rstest` fixture hygiene lint. After this
plan is approved and implemented, Whitaker will contain a new experimental
Dylint crate named `rstest_helper_should_be_fixture`. The crate will declare
and register the public lint `RSTEST_HELPER_SHOULD_BE_FIXTURE`, load its
configuration with sensible defaults, and be visible to the existing suite and
installer wiring without yet attempting the full call-site aggregation
algorithm.

The observable result is structural rather than diagnostic-heavy: a maintainer
can build the new lint crate, see the lint name in Whitaker's registration
metadata, configure its defaults in `dylint.toml`, and run the unit,
behavioural, lint, and formatting gates successfully. The later roadmap items
8.2.2, 8.2.3, and 8.2.4 will add call-site collection, cross-test aggregation,
diagnostic emission, and UI pass/fail coverage.

This plan must be approved before implementation starts.

## Constraints

- This plan covers roadmap item 8.2.1 only. Do not implement the full repeated
  helper-call detector, cross-test grouping, final diagnostic wording, or UI
  failure expectations that belong to 8.2.2 through 8.2.4.
- Keep the new lint experimental. It must not become part of the standard
  default lint set until later tuning against real repositories.
- Reuse existing repository conventions for Dylint crates, suite registration,
  installer resolution, configuration, `rstest` helpers, behaviour tests, and
  documentation.
- Preserve the hexagonal boundary in a pragmatic Rust form: pure rule and
  configuration policy should be testable without rustc or filesystem I/O, and
  compiler/Dylint integration should stay in the driver adapter layer.
- Do not add external dependencies unless implementation proves there is no
  reasonable existing workspace dependency or standard-library alternative.
- Do not mark roadmap item 8.2.1 done until implementation has landed, gates
  pass, CodeRabbit has no unresolved concerns, and the implementation PR is
  ready for review.
- Follow the repository rule that `make check-fmt`, `make lint`, and
  `make test` are the commit gates for code changes. Run command suites
  sequentially and capture output with `tee` into `/tmp`.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests where
  behaviour is user-meaningful. Property tests, Kani, or Verus are not required
  for the 8.2.1 bootstrap because this step introduces no new algorithmic
  invariant over a range of inputs.
- Keep every Rust source file at or below 400 lines and ensure each module
  starts with a `//!` module-level purpose comment.

## Tolerances

- Scope: if the implementation requires more than 14 repository files or more
  than 650 net code lines, stop and ask whether 8.2.1 should be split.
- Interface: if a public API in an existing crate must change incompatibly,
  stop and present options.
- Dependencies: if a new third-party dependency is needed, stop and justify it
  before adding it.
- Test strategy: if the implementation cannot provide both unit coverage and
  behaviour coverage for configuration and registration, stop and explain the
  missing observable behaviour.
- Diagnostics: if meaningful diagnostic emission becomes necessary to satisfy
  tests, stop and confirm whether part of 8.2.2 or 8.2.3 should be pulled into
  this task.
- Validation: if any of `make check-fmt`, `make lint`, or `make test` still
  fails after two fix attempts, stop and document the failing command, log
  path, and available choices.
- Review: if `coderabbit review --agent` reports concerns after a major
  milestone, address them or record why they are out of scope before moving on.
- Ambiguity: if experimental suite wiring conflicts with the existing install
  path, stop and ask whether the lint should be standalone-only for 8.2.1.

## Risks

- Risk: The existing suite has only a `dylint-driver` feature and derives
  experimental feature names from
  `installer::resolution::EXPERIMENTAL_LINT_CRATES`. Severity: medium.
  Likelihood: medium. Mitigation: add a suite feature named
  `experimental-rstest-helper-should-be-fixture` that enables the new optional
  lint dependency, and keep the installer source of truth in sync.

- Risk: A lint that does not yet emit diagnostics can look like dead code.
  Severity: medium. Likelihood: medium. Mitigation: make the first milestone
  observable through registration, configuration loading, suite metadata,
  installer resolution, and tests that exercise those contracts.

- Risk: Configuration schema drift can create user-facing documentation that
  does not match runtime defaults. Severity: medium. Likelihood: low.
  Mitigation: define a single `Config::default()` shape in the driver policy
  layer, test deserialization with `rstest`, and copy the same defaults into
  `docs/users-guide.md` and
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md` only if the
  implementation changes the design.

- Risk: `rstest` procedural macro expansion can hide source attributes from
  late lint passes. Severity: medium. Likelihood: medium. Mitigation: for
  8.2.1, rely only on the existing `common::rstest` pure detection and
  `whitaker::hir` span-recovery hooks. Defer macro-heavy call-site collection
  and UI diagnostics to 8.2.2 through 8.2.4.

- Risk: Full workspace gates are expensive and can expose unrelated failures.
  Severity: medium. Likelihood: low. Mitigation: run targeted tests before full
  gates, then run the required `make` gates sequentially with logs under
  `/tmp`. If unrelated failures are discovered, record evidence and ask for
  direction.

## Progress

- [x] (2026-05-18T18:36:20Z) Loaded the `leta`, `rust-router`,
  `execplans`, `hexagonal-architecture`, `firecrawl-mcp`, `arch-crate-design`,
  `rust-errors`, `en-gb-oxendict-style`, `commit-message`, and `pr-creation`
  skills needed for planning, branch work, Rust crate boundaries, configuration
  errors, documentation style, commits, and PR creation.
- [x] (2026-05-18T18:36:20Z) Created the Leta workspace for this checkout with
  `leta workspace add`.
- [x] (2026-05-18T18:36:20Z) Renamed the local branch to
  `8-2-1-create-the-rstest-helper-lint-crate`.
- [x] (2026-05-18T18:36:20Z) Used two Wyvern agents to inspect repository
  crate/configuration patterns and roadmap/design-document requirements.
- [x] (2026-05-18T18:36:20Z) Used Firecrawl to confirm current Dylint
  registration/configuration contracts and `rstest` fixture/test semantics from
  upstream documentation.
- [x] (2026-05-18T18:36:20Z) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-18T18:36:20Z) Ran `make fmt` to format the draft and reverted
  unrelated formatter-only changes outside this plan file.
- [x] (2026-05-18T18:36:20Z) Validated the plan milestone with
  `make check-fmt`, `make markdownlint`, `make lint`, and `make test`.
- [x] (2026-05-18T18:36:20Z) Reviewed and revised this plan after local
  formatting and linting.
- [x] (2026-05-18T18:36:20Z) Ran `coderabbit review --agent` for the plan
  milestone and cleared Markdown wrapping concerns.
- [x] (2026-05-18T18:36:20Z) Reran `coderabbit review --agent` and received
  0 findings.
- [x] (2026-05-18T19:15:16Z) Committed this plan after validation passed.
- [x] (2026-05-18T19:15:16Z) Pushed the branch and created draft PR
  <https://github.com/leynos/whitaker/pull/231> for the ExecPlan.
- [x] (2026-05-20T00:00:00Z) Received explicit user approval to implement
  the planned functionality.
- [x] (2026-05-20T00:00:00Z) Created the experimental lint crate and driver
  bootstrap with `RSTEST_HELPER_SHOULD_BE_FIXTURE`, configuration defaults,
  normalization, and `RstestDetectionOptions` construction.
- [x] (2026-05-20T00:00:00Z) Wired the crate into installer and suite
  experimental registration via `EXPERIMENTAL_LINT_CRATES` and the suite feature
  `experimental-rstest-helper-should-be-fixture`.
- [x] (2026-05-20T00:00:00Z) Added unit coverage for configuration defaults,
  TOML deserialization, unknown fields, numeric threshold normalization, and
  provider-attribute normalization in the new crate.
- [x] (2026-05-20T00:00:00Z) Confirmed behaviour coverage for experimental
  registration through installer `rstest-bdd` scenarios and suite registration
  scenarios. Targeted run
  `cargo nextest run -p whitaker_suite -p whitaker-installer --all-targets
  --all-features` passed 527 tests.
- [x] (2026-05-20T00:00:00Z) Updated user, developer, design, roadmap, and
  plan documentation. `docs/roadmap.md` now marks 8.2.1 done.
- [x] (2026-05-20T00:00:00Z) Ran `coderabbit review --agent` after the
  crate/bootstrap milestone and received 0 findings.
- [x] (2026-05-20T00:00:00Z) Ran final `coderabbit review --agent`. It
  reported two findings: clamp thresholds to at least 2 and avoid constructing
  a full default config for provider fallback. Both findings were accepted and
  fixed.
- [x] (2026-05-20T00:00:00Z) Reran `coderabbit review --agent` after the
  fixes. It reported one trivial Rustdoc concern for the private `Config`
  type, which was accepted and fixed.
- [x] (2026-05-20T00:00:00Z) Ran final local gates. `make check-fmt`,
  `make markdownlint`, `make lint`, and `make test` all passed; `make test`
  reported 1428 passed and 2 skipped.
- [x] (2026-05-20T00:00:00Z) Reran final gates after the Rustdoc review fix.
  `make check-fmt`, `make markdownlint`, `make lint`, and `make test` all
  passed; `make test` again reported 1428 passed and 2 skipped.
- [x] (2026-05-20T00:00:00Z) Committed the implementation as `813d2cc`,
  pushed it to `origin/8-2-1-create-the-rstest-helper-lint-crate`, and updated
  draft PR <https://github.com/leynos/whitaker/pull/231> for implementation
  review.

## Surprises & discoveries

- Observation: The installer already has an experimental lint path even though
  `EXPERIMENTAL_LINT_CRATES` is currently empty. Evidence:
  `installer/src/builder.rs` derives feature names as
  `experimental-{lint-name-with-hyphens}` and appends them when building the
  suite with `--experimental`. Impact: 8.2.1 should use that existing mechanism
  instead of inventing a new feature naming scheme.

- Observation: The roadmap prerequisites are already complete.
  Evidence: `docs/roadmap.md` marks 8.1.1 and 8.1.3 as `[x]`. Impact:
  implementation can use `common::rstest` detection and fingerprint APIs
  directly without adding prerequisite work to this plan.

- Observation: Current Dylint upstream docs list `dylint_linting` 6.0.0 as the
  latest release, but this workspace pins `dylint_linting = "5"`. Evidence:
  Firecrawl retrieved the docs.rs crate page for `dylint_linting` latest, and
  `Cargo.toml` pins workspace dependency `dylint_linting = "5"`. Impact:
  implementation must follow the local workspace pin and existing macro usage,
  not upgrade Dylint as part of this task.

- Observation: The first implementation slice compiles without adding a new
  workspace dependency. Evidence:
  `cargo check -p rstest_helper_should_be_fixture --all-targets --all-features`
  and `cargo check -p whitaker_suite --all-targets --all-features` both exited
  with status 0. Impact: the planned crate boundary and experimental feature
  wiring fit the existing workspace structure.

- Observation: `make fmt` can introduce Markdown reference-link breakage around
  issue-style links and bracketed reference text. Evidence: the first
  formatting run split `[#180][issue-180]` and a
  ``[`FluentLanguageLoader`]`` reference across lines, causing Markdown lint
  failures. Impact: those passages were rewritten to avoid the formatter edge
  case before continuing.

- Observation: CodeRabbit caught that normalizing `min_calls` and
  `min_distinct_tests` to 1 would make a "repeated" lint meaningful for a
  single occurrence. Evidence: final review finding on
  `crates/rstest_helper_should_be_fixture/src/driver.rs`. Impact: thresholds
  now normalize to at least 2, matching the design defaults and repeated-call
  semantics.

- Observation: Even private configuration types benefit from Rustdoc when they
  are the boundary between user TOML and lint policy. Evidence: final
  CodeRabbit finding on `Config`. Impact: `Config` now documents that it is
  loaded from `dylint.toml` and normalized before use.

- Observation: A final confirmation rerun of `coderabbit review --agent` was
  blocked by CodeRabbit rate limiting after the Rustdoc fix. Evidence: two
  retry attempts returned recoverable rate-limit errors. Impact: all reported
  CodeRabbit findings were fixed and local gates were rerun, but the tool could
  not provide a zero-finding confirmation after the last documentation-only
  code comment change.

## Decision log

- Decision: Keep this task to scaffolding, registration, and configuration
  loading, with no attempt at the full call-site detector. Rationale: Roadmap
  item 8.2.1 is explicitly followed by 8.2.2 for call-site collection, 8.2.3
  for aggregation and actionable diagnostics, and 8.2.4 for UI pass/fail
  coverage. Date/Author: 2026-05-18T18:36:20Z / Codex.

- Decision: Introduce the new lint as experimental.
  Rationale: `docs/developers-guide.md` states that new lints should typically
  start experimental, and the lint design warns that these fixture-hygiene
  lints need false-positive tuning before promotion. Date/Author:
  2026-05-18T18:36:20Z / Codex.

- Decision: Use the existing Rust crate pattern rather than a new architectural
  directory layout. Rationale: The repository already organizes lints as
  independent Dylint crates under `crates/`, with pure helper code in `common/`
  and driver glue inside each lint crate. This satisfies the useful part of
  hexagonal architecture without transplanting a foreign package structure.
  Date/Author: 2026-05-18T18:36:20Z / Codex.

- Decision: Do not require Kani or Verus for 8.2.1.
  Rationale: The bootstrap creates no substantive invariant over state spaces
  or contractual business axiom. Later algorithmic milestones can revisit
  property testing or bounded checking when they add grouping logic.
  Date/Author: 2026-05-18T18:36:20Z / Codex.

## Outcomes & retrospective

Implementation has landed on branch
`8-2-1-create-the-rstest-helper-lint-crate` and draft PR
<https://github.com/leynos/whitaker/pull/231> is updated for review. The
shipped behaviour is a bootstrap: the experimental Dylint crate exists, the
lint declaration and configuration defaults are wired, suite and installer
registration understand the experimental lint, and documentation states that
diagnostics follow later roadmap items.

The plan milestone has passed local validation:

- `make check-fmt` succeeded.
- `make markdownlint` succeeded with 0 Markdown errors.
- `make lint` succeeded.
- `make test` succeeded with 1418 tests passed and 2 skipped under the default
  nextest profile.
- `coderabbit review --agent` completed with 0 findings after review fixes.

The implementation milestone has passed local validation:

- `cargo check -p rstest_helper_should_be_fixture --all-targets --all-features`
  succeeded.
- `cargo check -p whitaker_suite --all-targets --all-features` succeeded.
- `cargo nextest run -p rstest_helper_should_be_fixture --all-targets
  --all-features` passed 9 tests.
- `cargo nextest run -p whitaker_suite -p whitaker-installer --all-targets
  --all-features` passed 527 tests.
- `make check-fmt` succeeded.
- `make markdownlint` succeeded with 0 Markdown errors.
- `make lint` succeeded.
- `make test` succeeded with 1428 tests passed and 2 skipped under the default
  nextest profile.

After addressing CodeRabbit's threshold, provider-fallback, and Rustdoc
findings, the final gates were rerun with the same successful outcomes. A
final CodeRabbit confirmation pass was attempted twice and was blocked by
recoverable rate limits.

## Context and orientation

Whitaker is a Rust Cargo workspace. The root `Cargo.toml` declares workspace
members `common`, `crates/*`, `installer`, and `suite`. Individual Dylint lint
crates live under `crates/`; shared pure helpers live in `common/`; the root
`whitaker` crate exposes shared compiler-facing glue; and the `suite/` crate
aggregates lint crates into one Dylint cdylib.

The relevant roadmap entry is in `docs/roadmap.md`:

```plaintext
8.2.1. Create the `rstest_helper_should_be_fixture` lint crate, register
`RSTEST_HELPER_SHOULD_BE_FIXTURE`, and wire configuration loading defaults.
```

The design source of truth is
`docs/lints-for-rstest-fixtures-and-test-hygiene.md`, especially "Lint A:
call-site fixture extraction". That document defines:

- crate name: `rstest_helper_should_be_fixture`,
- lint name: `RSTEST_HELPER_SHOULD_BE_FIXTURE`,
- default configuration keys:
  `min_calls`, `min_distinct_tests`, `require_identical_fixture_arg_names`,
  `provider_param_attributes`, and `use_source_callee_fallback`,
- strict `rstest` detection for `rstest` and `rstest::rstest`,
- default provider parameter attributes:
  `case`, `values`, `files`, `future`, and `context`,
- first-version fixture-local support for simple identifier bindings only.

Roadmap prerequisites 8.1.1 and 8.1.3 are already complete. They provide the
shared `common::rstest` APIs in `common/src/rstest/`:

- `is_rstest_test` and `is_rstest_test_with` for strict test detection,
- `is_rstest_fixture` and `is_rstest_fixture_with` for strict fixture
  detection,
- `RstestDetectionOptions` and `fixture_local_names` for fixture-local
  parameter policy,
- `ArgFingerprint` and `ArgAtom` for later helper-call fingerprinting.

Existing lint crates show the local Dylint pattern. For example,
`crates/conditional_max_n_branches/src/driver.rs` declares its lint with
`dylint_linting::impl_late_lint!`, loads per-lint configuration through
`dylint_linting::config::<Config>(LINT_NAME)`, falls back to defaults on
missing or invalid configuration, and loads shared locale settings through
`whitaker::SharedConfig`. The new crate should match this style.

The suite registration files are:

- `suite/Cargo.toml`,
- `suite/src/lints.rs`,
- `suite/src/driver.rs`,
- `suite/tests/registration.rs`,
- `suite/tests/features/suite_registration.feature`.

The installer registration files are:

- `installer/src/resolution.rs`,
- `installer/src/builder.rs`,
- `installer/src/scanner.rs`,
- `installer/tests/behaviour_core.rs`,
- `installer/tests/features/installer.feature`.

The user-facing documentation that may need updates is:

- `docs/users-guide.md` for the new lint section and configuration example,
- `docs/developers-guide.md` for any new internal convention,
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md` for implementation
  decisions if runtime behaviour differs from the design,
- `docs/roadmap.md` when implementation is complete.

Useful skills for implementation are `leta`, `rust-router`, `arch-crate-design`,
 `rust-errors`, `hexagonal-architecture`, `en-gb-oxendict-style`,
`commit-message`, and `pr-creation`. If later algorithm work introduces
invariants, route again through `rust-router` and consider `kani` or `verus`.

External references checked during planning:

- Dylint `dylint_linting` docs:
  <https://docs.rs/crate/dylint_linting/latest>. The macros `impl_late_lint!`
  and configuration helpers such as `config_or_default`, `config`, and
  `init_config` are the relevant public contracts.
- `rstest` fixture docs:
  <https://docs.rs/rstest/latest/rstest/attr.fixture.html>. Fixtures are
  functions annotated with `#[fixture]` and injected as `#[rstest]` test
  parameters.
- `rstest` test docs:
  <https://docs.rs/rstest/latest/rstest/attr.rstest.html>. `#[rstest]` supports
  fixture injection, cases, values, files, `future`, and `context` parameter
  attributes.

## Plan of work

Stage A is pre-implementation review. Read this ExecPlan, compare it with
`docs/roadmap.md` and `docs/lints-for-rstest-fixtures-and-test-hygiene.md`, and
obtain explicit approval. Do not edit production code before approval.

Stage B creates the lint crate skeleton. Add
`crates/rstest_helper_should_be_fixture/Cargo.toml`,
`crates/rstest_helper_should_be_fixture/src/lib.rs`, and a driver module such as
 `src/driver.rs`. The manifest should follow existing lint crates:
`crate-type = ["cdylib", "rlib"]`, `test = false`, a `dylint-driver` feature
that enables `dylint_linting`, `rustc_*`, `serde`, `log`, `whitaker`, and
`whitaker-common`, and a `constituent` feature that enables
`dylint_linting/constituent`. The non-driver build should compile through a
small stub module so `cargo check --workspace --all-targets --all-features` and
documentation builds stay healthy.

Stage C implements registration and configuration policy. In the driver,
declare:

```rust
dylint_linting::impl_late_lint! {
    pub RSTEST_HELPER_SHOULD_BE_FIXTURE,
    Warn,
    "repeated rstest helper calls should be extracted into fixtures",
    RstestHelperShouldBeFixture::default()
}
```

Define a `Config` with `#[serde(default, deny_unknown_fields)]` and these
defaults:

```plaintext
min_calls = 2
min_distinct_tests = 2
require_identical_fixture_arg_names = false
provider_param_attributes = ["case", "values", "files", "future", "context"]
use_source_callee_fallback = false
```

Keep pure validation and normalization near the config type. The driver should
load configuration in `check_crate`, clamp invalid numeric thresholds to at
least `2` or reject them by falling back to defaults in the same manner as
existing lints, and build `RstestDetectionOptions` from the provider
attributes. If a value is invalid enough that clamping would hide a user
mistake, use the existing "log and default" pattern rather than panic.

Stage D adds a minimal late-lint pass body that proves wiring without claiming
8.2.2 behaviour. The pass may inspect crate or item structure enough to prove
that it can construct configuration and detection options, but it should not
emit helper-call diagnostics until the collection and aggregation milestones
exist. Any placeholder should be named as a bootstrap path, not as completed
analysis.

Stage E wires registration outside the crate. Add
`rstest_helper_should_be_fixture` to `installer/src/resolution.rs`
`EXPERIMENTAL_LINT_CRATES`, not to `LINT_CRATES`. Add the suite optional
dependency and a feature named `experimental-rstest-helper-should-be-fixture` to
 `suite/Cargo.toml`, following the feature name that `installer/src/builder.rs`
derives from the experimental crate list. Update `suite/src/lints.rs` and
`suite/src/driver.rs` so the lint is included only when that experimental
feature is enabled. Update suite and installer tests to expect the new
experimental lint when experimental mode is on and to exclude it otherwise.

Stage F adds tests. Unit tests should use `rstest` for `Config::default()`,
valid TOML deserialization, unknown-field rejection, provider-attribute
normalization, threshold handling, and feature-name expectations if helper
functions are added. Behaviour tests should use `rstest-bdd` where the
observable behaviour is "experimental lint resolution includes the new crate"
or "the suite metadata exposes the new lint only with experimental features".
If a crate-local behaviour file is added, keep it focused on configuration and
registration, not future diagnostic detection.

Stage G updates documentation. Add a concise experimental lint section to
`docs/users-guide.md` that identifies the purpose, scope, and configuration
defaults. Say that this first implementation registers and configures the lint
and that diagnostic emission follows the later roadmap items if that remains
true at implementation time. Update `docs/developers-guide.md` only if a new
experimental-suite convention is introduced or clarified. Update
`docs/lints-for-rstest-fixtures-and-test-hygiene.md` with an implementation
decision note only if the implementation materially differs from the current
design. Mark `docs/roadmap.md` item 8.2.1 done only after the implementation
passes gates and review.

Stage H validates, reviews, commits, and prepares the implementation PR. Run
targeted tests first, then the full gates. Run `coderabbit review --agent`
after the crate/configuration milestone and again before final push if the
review tool is available. Address all relevant concerns. Commit in small
logical units: crate bootstrap, registration wiring, tests, documentation, and
roadmap completion if those land separately.

## Concrete steps

From the repository root, confirm branch and workspace state:

```sh
git branch --show-current
git status --short --branch
```

Expected output includes:

```plaintext
8-2-1-create-the-rstest-helper-lint-crate
```

After approval, create the crate files and add registration wiring. Use
`apply_patch` for manual edits. Prefer existing crate files as templates:

```sh
sed -n '1,180p' crates/conditional_max_n_branches/src/driver.rs
sed -n '1,140p' crates/conditional_max_n_branches/Cargo.toml
sed -n '1,140p' suite/src/lints.rs
sed -n '1,180p' installer/src/resolution.rs
```

Run targeted checks after the first code milestone:

```sh
ACTION=check-rstest-helper
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
cargo check -p rstest_helper_should_be_fixture --all-targets --all-features 2>&1 | tee "$LOG"
```

Run targeted unit and behaviour tests after tests are added:

```sh
ACTION=test-rstest-helper
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features 2>&1 | tee "$LOG"
```

Run suite and installer tests touched by registration:

```sh
ACTION=test-registration
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
cargo nextest run -p whitaker_suite -p whitaker-installer --all-targets --all-features 2>&1 | tee "$LOG"
```

Run documentation checks after documentation changes:

```sh
ACTION=markdownlint
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make markdownlint 2>&1 | tee "$LOG"
```

If Mermaid diagrams are edited, also run:

```sh
ACTION=nixie
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make nixie 2>&1 | tee "$LOG"
```

Run final gates sequentially before committing code as complete:

```sh
ACTION=check-fmt
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make check-fmt 2>&1 | tee "$LOG"

ACTION=lint
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make lint 2>&1 | tee "$LOG"

ACTION=test
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make test 2>&1 | tee "$LOG"
```

Run CodeRabbit after each major milestone:

```sh
coderabbit review --agent
```

Commit using a file-based commit message:

```sh
git status --short
git diff --stat
git add <changed files>
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Create rstest helper fixture lint crate

Add the experimental Dylint crate scaffold, registration wiring,
configuration defaults, tests, and documentation for roadmap item 8.2.1.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Push and open a draft pull request only after validation succeeds:

```sh
git push -u origin 8-2-1-create-the-rstest-helper-lint-crate
echo "${LODY_SESSION_ID}"
```

Use the Lody session URL in the PR body:

```plaintext
https://lody.ai/leynos/sessions/${LODY_SESSION_ID}
```

## Validation and acceptance

The plan is accepted for implementation when the user explicitly approves it.
Silence is not approval.

The implementation is accepted when all of the following are true:

- `crates/rstest_helper_should_be_fixture` exists and builds with
  `--features dylint-driver`.
- The crate exposes `RSTEST_HELPER_SHOULD_BE_FIXTURE` and the pass type
  `RstestHelperShouldBeFixture`.
- Configuration defaults match the lint design and are covered by `rstest`
  unit tests.
- Unknown configuration fields are rejected or defaulted consistently with the
  repository's existing Dylint configuration policy.
- The installer recognizes the crate as experimental and includes it only when
  experimental lints are requested or when it is explicitly named.
- The suite exposes the lint only through the experimental feature
  `experimental-rstest-helper-should-be-fixture`.
- Behaviour tests cover experimental registration and configuration loading in
  user-observable terms.
- `docs/users-guide.md`, `docs/developers-guide.md`, and the rstest lint
  design document are updated where behaviour or internal practice changed.
- `docs/roadmap.md` marks 8.2.1 done only after all implementation checks pass.
- `coderabbit review --agent` has no unresolved relevant concerns.
- `make check-fmt`, `make lint`, and `make test` all succeed.

Expected final gate transcript shape:

```plaintext
$ make check-fmt
...
Finished

$ make lint
...
Finished

$ make test
...
test result: ok
```

The exact test count may change as new tests are added; success means the
commands exit with status 0.

## Idempotence and recovery

All edit stages are ordinary file edits and can be retried from `git status`.
If a step fails part-way through, inspect `git diff` and either continue from
the incomplete file or revert only the files changed for this task. Do not
revert unrelated user changes.

If a generated or copied lint crate scaffold is wrong, delete only
`crates/rstest_helper_should_be_fixture/` before it is committed, then recreate
it from the existing lint crate pattern. After a commit, prefer a corrective
follow-up commit over rewriting history unless the user asks for history
cleanup.

If validation fails, inspect the matching
`/tmp/*-whitaker-8-2-1-create-the-rstest-helper-lint-crate.out` log before
changing code. Record persistent failures in `Surprises & Discoveries` or
`Decision Log` with the command and log path.

## Artifacts and notes

Wyvern repository-pattern findings:

```plaintext
Existing lint crates use `dylint_linting::impl_late_lint!`, per-lint
`Config` structs, `LINT_NAME` matching the crate name, `SharedConfig` for
locale, and suite/installer registration through `suite/` and
`installer/src/resolution.rs`.
```

Wyvern documentation findings:

```plaintext
8.2.1 is the bootstrap step. 8.2.2 starts call-site collection, 8.2.3 adds
aggregation and actionable diagnostics, and 8.2.4 adds UI pass/fail coverage.
```

Firecrawl findings:

```plaintext
Dylint's `impl_late_lint!` macro wraps Dylint library registration, lint
declaration, lint-pass implementation, and late-pass registration for a
single-lint library. Dylint configuration is read from `dylint.toml` with
helpers such as `config`, `config_or_default`, and `init_config`.

`rstest` fixtures are functions marked with `#[fixture]` and injected as test
arguments. `#[rstest]` parameters can also be provider-driven through
attributes such as `case`, `values`, `files`, `future`, and `context`.
```

## Interfaces and dependencies

The new crate should expose these driver-facing items when `dylint-driver` is
enabled:

```rust
pub const RSTEST_HELPER_SHOULD_BE_FIXTURE: &rustc_lint::Lint;

pub struct RstestHelperShouldBeFixture {
    config: Config,
    detection_options: whitaker_common::rstest::RstestDetectionOptions,
    localizer: whitaker_common::Localizer,
}
```

The exact field visibility may remain private. The public contract is the lint
constant and pass type. If the Dylint macro exposes a different lint constant
type than the simplified sketch above, follow the macro's actual expansion and
the pattern used by existing lint crates.

The configuration policy should be equivalent to:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Config {
    min_calls: usize,
    min_distinct_tests: usize,
    require_identical_fixture_arg_names: bool,
    provider_param_attributes: Vec<String>,
    use_source_callee_fallback: bool,
}
```

Do not export `Config` unless tests or downstream code genuinely require it. If
tests need access, prefer `pub(crate)` helpers or driver-internal tests.

The new crate should depend on existing workspace dependencies only:

- `dylint_linting`,
- `log`,
- `rustc_hir`,
- `rustc_lint`,
- `rustc_session`,
- `rustc_span`,
- `serde`,
- `whitaker`,
- `whitaker-common`,
- `rstest`,
- `rstest-bdd`,
- `rstest-bdd-macros`,
- `dylint_testing` if a UI harness is required for bootstrap smoke coverage.

No new production dependency is expected for 8.2.1.

## Revision note

Initial draft created on 2026-05-18. It records the pre-implementation scope,
repo orientation, external references, experimental-suite decision, validation
commands, and approval gate for roadmap item 8.2.1.

Revision on 2026-05-18 after local validation: recorded successful
`make check-fmt`, `make markdownlint`, `make lint`, and `make test` results.
This does not change the implementation scope; it only adds evidence for the
ExecPlan review milestone.

Revision on 2026-05-18 after CodeRabbit review: wrapped one long Markdown line
reported by `coderabbit review --agent`. This is a formatting-only change with
no effect on implementation scope.

Second revision on 2026-05-18 after CodeRabbit review: moved the `rstest`
test-documentation URL onto its own line and wrapped the following sentence.
This is a formatting-only change with no effect on implementation scope.

Third revision on 2026-05-18 after CodeRabbit review: changed an `-isation`
spelling to the matching Oxford `-ization` spelling. This is a spelling-only
change with no effect on implementation scope.

Fourth revision on 2026-05-18 after CodeRabbit review: recorded the clean
`coderabbit review --agent` result. This adds review evidence only and does not
change the implementation scope.

Fifth revision on 2026-05-18 after draft PR creation: recorded the commit,
push, and draft PR URL in `Progress`. This is a process-tracking update with no
effect on implementation scope.
