# RFC 0001: Lint candidates derived from CodeRabbit review findings

## Preamble

- **RFC number:** 0001
- **Status:** Proposed
- **Created:** 2026-07-07

## Summary

A corpus of 192 CodeRabbit review findings was analysed, of which 83 target
Rust source files. Recurring finding patterns that are mechanical enough to
detect, not already covered by Clippy or rustc, and not already scheduled in
`docs/roadmap.md` were distilled into seven candidate enforcement paths. One
lint, `test_helper_must_return_result`, is recommended for immediate
scheduling. Two paths require Clippy-first evaluation before Whitaker adds only
the unmatched behaviour. Two further candidates are recommended as follow-ups,
and two are recorded but deferred.

## Problem

CodeRabbit reviews across df12 repositories repeatedly raise the same classes
of Rust defect. Each occurrence costs a review round trip, and the guidance is
applied inconsistently because it is enforced by a reviewing agent rather than
by tooling. Where a finding class is mechanical — a syntactic or type-level
property that a Dylint pass can check — it is a candidate for promotion into
Whitaker, so the defect is caught at commit time rather than at review time.

The existing roadmap already schedules lints for several review themes (fixture
extraction in §8, clone detection in §7, documentation coverage in §2.2). The
corpus was screened against those entries so that only unscheduled patterns are
proposed here.

## Current state

The corpus is a JSON Lines capture of 192 findings
(`~/docs/coderabbit-sample-findings.txt`), spanning Rust, Python, and Ansible
targets across several repositories. The 83 Rust findings were grouped by the
behaviour the reviewer asked for. Clusters that map onto existing coverage were
excluded:

- Duplicate module roots (`neighbour_scoring.rs` beside
  `neighbour_scoring/mod.rs`, five findings) — rustc rejects this as E0761; no
  lint is needed.
- Missing `# Errors` rustdoc sections (two findings) —
  `clippy::missing_errors_doc` already exists; consumers should enable it.
- Repeated helper extraction and duplicated test paragraphs (four findings)
  — scheduled as the `rstest` hygiene lints (roadmap §8) and the clone detector
  pipeline (roadmap §7).
- Module and public-item documentation gaps (three findings) — scheduled as
  `module_must_have_inner_docs` (roadmap 2.2.3) and `public_fn_must_have_docs`
  (roadmap 2.2.4).

The remaining clusters require either explicit Clippy configuration or a new
owner. The table below summarizes them; the sections that follow specify the
enforcement paths.

| Cluster                                                    | Findings | Severity profile  | Candidate lint                               |
| ---------------------------------------------------------- | -------- | ----------------- | -------------------------------------------- |
| Test steps and helpers panic instead of returning `Result` | 7        | all major         | `test_helper_must_return_result`             |
| Assertions and mismatch arms omit diagnostic context       | 5        | minor and trivial | Clippy plus `assertion_context_incomplete`   |
| Direct process-environment access or mutation              | 4        | major and trivial | Clippy plus possible `no_std_env_operations` |
| Error type or context erased at propagation sites          | 5        | mixed             | `error_context_discarded`                    |
| `drop()` used to silence unused-variable warnings          | 2        | major and trivial | `no_drop_to_silence_unused`                  |
| Fallible calls between resource acquisition and guard      | 3        | all major         | `fallible_gap_before_guard`                  |
| Near-identical tests differing only in literals            | 3        | all trivial       | `parameterizable_duplicate_tests`            |

_Table 1: Unscheduled Rust finding clusters in the CodeRabbit corpus._

## Goals and non-goals

- Goals:
  - Identify CodeRabbit finding classes that are mechanically detectable and
    recurrent enough to justify a Whitaker lint.
  - Specify each candidate precisely enough for roadmap scheduling and
    subsequent design work.
  - Record explicitly which clusters were rejected and why, so the corpus
    does not need re-analysis.
- Non-goals:
  - Full technical designs for the candidate lints; each accepted candidate
    receives its own design document or design section before
    implementation.
  - Lints for non-Rust findings (Python and Ansible clusters are out of
    scope for Whitaker).
  - Replacing CodeRabbit review; the lints reduce round trips but do not
    cover judgement-based findings.

## Proposed lints

### Tier 1: immediate and Clippy-first candidates

#### `test_helper_must_return_result`

Seven major findings ask for the same change: behaviour-driven development
(BDD) step functions and test helpers that panic — via `panic!`, `expect`,
`unwrap`, or a `match` arm that panics — should instead return
`Result<(), BoxError>` (or equivalent) and propagate failures with the `?`
operator. Panicking helpers abort the harness without context, whereas `Result`
propagation surfaces the failing step and preserves the source error.

Detection sketch: within test contexts (reusing the context detection in
`common`), flag functions bearing `#[given]`, `#[when]`, `#[then]`, or
`#[fixture]` attributes (and, when configured, `fn` items only called from
such functions) whose return type is not `Result` and whose body contains a
panicking construct. The lint composes with the existing
`no_expect_outside_tests`, which deliberately permits `expect` in tests; this
lint narrows that permission for step and fixture functions specifically.

Configuration: an attribute allowlist (defaults to the `rstest` and
`rstest-bdd` step attributes) and a switch for whether plain `#[test]`
functions are in scope (default off, since panicking assertions are idiomatic
in unit tests).

Two later findings in the same corpus (items asking for `.expect()` over
`match` plus `panic!`) show reviewers steering towards concise panics when a
`Result` return is unavailable. The lint should therefore emit a suggestion to
change the signature, not merely to reword the panic.

#### `assertion_context_incomplete`

Five findings report assertions or error arms that fail without enough context
to diagnose. Clippy's `missing_assert_message` already covers `assert!`,
`assert_eq!`, and `assert_ne!` calls without a custom panic message. Consumers
should enable that lint rather than duplicate its macro detection in Whitaker.

The unmatched behaviour is narrower: mismatch branches and `format!`-style
error construction inside `Err(...)` returns where only one of the expected and
actual bindings appears in the failure payload. A possible
`assertion_context_incomplete` lint should target only that semantic imbalance.
Before implementation, probe the corpus with `missing_assert_message` enabled
and retain only findings that survive that screen.

Configuration: severity per failure-construction shape. The macro-expansion
span helpers from roadmap 8.1.2 apply where a surviving case originates in a
third-party assertion helper.

#### `no_std_env_operations`

Four findings concern direct process-environment access: local `ENV_LOCK`
mutexes duplicating a shared guard, `std::env::set_var`/`remove_var` calls
mutating global state in tests, and helpers reading `std::env::var` directly
instead of through an injectable reader (one finding asks explicitly for
"mockable or an injected environment reader"). The test-side symptoms are
downstream of a production-side design gap: code that reads the environment
ambiently forces its tests to mutate process-global state, which races across
threads. The lint therefore targets the mechanism in all contexts, not only in
tests, mirroring how `no_std_fs_operations` pushes filesystem access behind
injectable handles.

Clippy's `disallowed_methods` can already reject resolved calls such as
`std::env::var`, including aliased imports. Consumers should first configure it
for `var`, `var_os`, `vars`, `vars_os`, `set_var`, and `remove_var`; the
compile-time `env!` and `option_env!` macros remain out of scope.

A Whitaker-specific `no_std_env_operations` lint is justified only for policy
that the Clippy configuration and narrowly scoped `allow` attributes cannot
express adequately: centrally configured module-boundary exemptions,
operation-specific severity, and context-sensitive remediation. Production
diagnostics would recommend an environment handle (for example
`&dyn mockable::Env`, with `DefaultEnv` supplied at the composition root),
while test diagnostics would recommend a stub such as `MockEnv` rather than
process-global mutation. The implementation decision must follow a downstream
trial of `disallowed_methods`, not precede it.

### Tier 2: recommended as follow-ups

#### `error_context_discarded`

Five findings concern error-fidelity loss at propagation sites: fields and
signatures typed `Result<_, String>`, `map_err(|e| e.to_string())` erasing the
source type and backtrace, and bare `?` on spawn-like operations where the
reviewer asked for contextual mapping. The first two shapes are cleanly
detectable (a `String` error type in a local `Result`, or a `map_err` closure
whose body is a bare `to_string`/`format!` of the error). The bare-`?` shape is
noisy to detect in general and should be scoped to a configurable list of
context-worthy callees, or omitted from the first iteration.

#### `no_drop_to_silence_unused`

Two findings flag `drop(binding)` statements whose sole purpose is to mark a
parameter as used, where the idiomatic remedies are an underscore prefix or an
`#[expect(unused_variables, reason = ...)]` attribute. `clippy::drop_non_drop`
fires only when the type has no `Drop` implementation; this lint instead
targets the intent, flagging `drop` of an otherwise-unused parameter regardless
of type. Low frequency in the corpus, but trivially cheap to implement on the
HIR and with an unambiguous machine-applicable suggestion.

### Tier 3: recorded but deferred

#### `fallible_gap_before_guard`

Three major findings describe resource leaks where a fallible call sits between
acquiring a resource and constructing the RAII guard that releases it (a
spawned child before `kill_on_drop`, a server handle before its `Drop` guard, a
database created before setup completes). The defect class is real and severe,
but detection requires knowing which calls acquire resources and which types
are guards — a semantic judgement that would need either annotation support or
MIR-based escape analysis akin to the ownership-shape lints (roadmap §9).
Deferred until the §9 infrastructure exists; revisit as a possible extension of
`common::ownership_shape`.

#### `parameterizable_duplicate_tests`

Three trivial findings ask for near-identical test functions, differing only in
literal arguments, to be collapsed into table-driven `#[rstest]` cases. This
overlaps substantially with the clone detector pipeline (roadmap §7), which
will already surface Type-2 duplicates in test code, and with the `rstest`
hygiene family (roadmap §8). Deferred pending experience with those suites; if
clone-detector output proves too coarse for test-specific advice, this can be
revisited as a specialized consumer of the same fingerprints.

## Compatibility and migration

`test_helper_must_return_result` follows the established Whitaker delivery
pattern: a dedicated lint crate with UI tests (roadmap 2.1.1), Fluent
localization entries (roadmap §2.3), and feature-gated wiring into
`whitaker_suite`. It should launch as experimental, mirroring the promotion
path defined for the `rstest` hygiene lints (roadmap 8.5.4).

The assertion and environment paths begin as Clippy configuration trials.
Whitaker lint crates should be scheduled only after corpus and downstream
results demonstrate residual behaviour that Clippy cannot express.

`test_helper_must_return_result` must be sequenced after the shared `rstest`
detection helpers (roadmap 8.1.1), which it reuses for step-attribute
recognition. If the Clippy trial justifies `no_std_env_operations`, it can
reuse the resolved-path classifier approach from `no_std_fs_operations`.

## Alternatives considered

### Option A: rely on Clippy restriction lints

Clippy offers adjacent and directly applicable lints. `missing_assert_message`
checks assertion macros without custom panic messages, and `disallowed_methods`
can reject configured environment calls such as `std::env::var`. Those
mechanisms should be enabled and evaluated before a new Whitaker lint is
implemented.

Clippy still has no notion of BDD step-function fallibility, does not compare
the expected and actual bindings carried by a constructed error, and cannot
centrally model Whitaker's sanctioned environment boundaries with
context-sensitive remediation. Whitaker candidates are restricted to those
residual behaviours. Where Clippy covers a cluster, this RFC excludes that arm
rather than duplicating it.

### Option B: keep enforcement in CodeRabbit

Review-time enforcement catches the defects eventually but costs a round trip
per occurrence, depends on reviewer configuration per repository, and produces
inconsistent phrasing (the corpus itself shows contradictory advice about
panics between review rounds). Lint-time enforcement is deterministic, local,
and self-documenting. CodeRabbit remains valuable for the judgement-based
findings that dominate the non-mechanical remainder of the corpus.

### Option C: one umbrella "test hygiene" lint

Bundling the Tier 1 enforcement paths into a single lint would conflate
unrelated policies, duplicate Clippy, and prevent consumers from adopting one
without the others. The residual Whitaker candidates stay separate.

## Open questions

- Should `test_helper_must_return_result` extend to helpers reachable only
  from step functions (call-graph analysis), or stay attribute-scoped in its
  first iteration?
- After enabling `missing_assert_message`, how many corpus findings remain for
  `assertion_context_incomplete`, and do they share a reliable expected/actual
  binding shape?
- Can scoped `allow(clippy::disallowed_methods)` attributes document all
  sanctioned environment boundaries without excessive repetition, or is a
  central `allowed_paths` model still warranted?

## Recommendation

Schedule `test_helper_must_return_result` after the shared helpers in roadmap
8.1. Configure and trial Clippy's `missing_assert_message` and
`disallowed_methods` before scheduling either `assertion_context_incomplete` or
`no_std_env_operations`; any Whitaker implementation must cover only the
residual semantic or boundary-aware behaviour. Take `error_context_discarded`
and `no_drop_to_silence_unused` as Tier 2 follow-ups once the Tier 1 work is
complete and its noise characteristics are understood. Record the two deferred
candidates against their blocking infrastructure (roadmap §9 for
`fallible_gap_before_guard`, roadmap §7 and §8 experience for
`parameterizable_duplicate_tests`) and revisit when that work lands.
