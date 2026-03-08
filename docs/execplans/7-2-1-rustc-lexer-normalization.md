# Implement `rustc_lexer` normalization, k-shingling, winnowing, and Rabin-Karp hashing (roadmap 7.2.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 7.2.1 delivers the first executable part of the clone detector
pipeline described in `docs/whitaker-clone-detector-design.md` §Pass A: token
engine (`rustc_lexer`). After this change, Whitaker has a pure Rust library
that can:

1. Normalize Rust source into deterministic Type-1 (`T1`) and Type-2 (`T2`)
   token streams.
2. Convert normalized tokens into `k`-shingles with byte-accurate source
   regions.
3. Compute deterministic Rabin-Karp rolling hashes for those shingles.
4. Apply winnowing to retain stable representative fingerprints for later
   MinHash and locality-sensitive hashing (LSH) work in roadmap item 7.2.2.

This stage must stop at fingerprint production. It must not implement candidate
generation, SARIF emission, command-line interface (CLI) wiring, or Dylint
integration; those belong to later roadmap items.

Observable outcome:

1. A new crate `crates/whitaker_clones_core/` exports a documented token-pass
   API.
2. Unit tests cover happy paths, unhappy paths, and edge cases for
   normalization, shingling, rolling hashing, and winnowing.
3. Behaviour tests using `rstest-bdd` v0.5.0 exercise end-to-end token-pass
   workflows from source text to retained fingerprints.
4. `docs/whitaker-clone-detector-design.md` records the final 7.2.1 design
   decisions under a new implementation-decisions subsection.
5. `docs/roadmap.md` marks 7.2.1 done only after all quality gates pass.
6. `make check-fmt`, `make lint`, and `make test` pass, and the documentation
   gates required by `AGENTS.md` for Markdown changes also pass.

## Constraints

- Scope only roadmap item 7.2.1. Do not implement MinHash, LSH, candidate
  pairing, Jaccard scoring, SARIF emission, AST refinement, caching, CLI
  commands, or lint integration.
- Create `crates/whitaker_clones_core/` because the design document assigns the
  token engine to that crate and no such crate exists in the current tree.
- Keep the new crate free of `rustc_private` dependencies. It must be usable as
  an ordinary Rust library by later crates.
- Introduce `rustc_lexer` as the tokenization dependency using a caret version
  requirement in `[workspace.dependencies]`, in line with repository policy.
- Prefer existing workspace dependencies (`camino`, `thiserror`, `rstest`,
  `rstest-bdd`, `rstest-bdd-macros`) over adding new third-party crates.
- Keep every Rust source file below 400 lines. Split the token engine into
  sibling modules before approaching the limit.
- Every module must begin with a `//!` module-level doc comment.
- Every public API must have Rustdoc with examples that compile under
  `cargo test --doc` as part of `make test`.
- Use en-GB-oxendict spelling in comments and documentation.
- Design the API so invalid `k` and winnowing-window values are rejected
  explicitly; do not silently coerce zero or empty values.
- Preserve deterministic behaviour across runs. The same input and parameters
  must yield the same normalized stream, hashes, and selected fingerprints.
- Record the final implementation decisions in
  `docs/whitaker-clone-detector-design.md`.
- Completion must include the roadmap checkbox update for 7.2.1.

## Tolerances

- Scope tolerance: if implementation begins to require 7.2.2 concepts
  (MinHash, bands, rows, candidate buckets) or 7.2.3 concepts (SARIF runs,
  result builders, stable fingerprints for reporting), stop and escalate.
- Dependency tolerance: if any new third-party dependency beyond `rustc_lexer`
  and already-pinned workspace crates appears necessary, stop and escalate with
  the concrete reason.
- API tolerance: if the token-pass API cannot be kept pure-library and starts
  needing filesystem traversal, Cargo metadata, or `rustc_private`, stop and
  escalate.
- Size tolerance: if the change exceeds 18 touched files or 1500 net new lines
  of code, stop and escalate.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` still
  fail after 3 targeted fix iterations, stop and escalate with the captured log
  paths.

## Risks

- Token classification risk: the design sketch uses `TokenKind as u32`, but the
  implementation needs a stable and explicit mapping rather than relying on raw
  discriminants. Mitigation: define an internal normalized token symbol type
  and map `rustc_lexer::TokenKind` into it deliberately.
- Range-accounting risk: byte ranges must survive trivia stripping,
  canonicalization, and shingle aggregation without off-by-one errors.
  Mitigation: store original byte spans per normalized token and assert region
  boundaries in unit and behaviour tests.
- Winnowing determinism risk: tie handling is underspecified in the design
  text. Mitigation: choose one rule explicitly (for example, rightmost minimum
  in each window), test it, and record it in the design document.
- Small-input risk: inputs shorter than `k`, or fingerprint lists shorter than
  the winnowing window, can produce surprising empty outputs. Mitigation:
  define the exact behaviour up front, test it, and document it in the public
  API.
- Test ergonomics risk: `rstest` and `rstest-bdd` parameters count toward the
  workspace Clippy `too_many_arguments` threshold. Mitigation: use small helper
  structs, tuple-style `#[case]` inputs, and BDD steps with at most `world` + 3
  parsed values.
- Integration-test lint risk: `expect()` and `unwrap()` are denied in
  integration tests. Mitigation: use `Result`-returning step functions or the
  established `match`/`panic!` helper pattern used elsewhere in the repository.

## Progress

- [x] Stage A: Gather context and draft this ExecPlan.
- [ ] Stage B: Scaffold `crates/whitaker_clones_core/` and workspace wiring.
- [ ] Stage C: Add failing unit tests for normalization, parameter validation,
  shingling, Rabin-Karp hashing, and winnowing.
- [ ] Stage D: Implement core token-pass domain types and validation helpers.
- [ ] Stage E: Implement `rustc_lexer` normalization for `T1` and `T2`.
- [ ] Stage F: Implement `k`-shingling and Rabin-Karp rolling hash support.
- [ ] Stage G: Implement deterministic winnowing over hashed shingles.
- [ ] Stage H: Add `rstest-bdd` behaviour coverage for end-to-end token-pass
  scenarios.
- [ ] Stage I: Update `docs/whitaker-clone-detector-design.md` with final 7.2.1
  implementation decisions.
- [ ] Stage J: Mark roadmap item 7.2.1 done in `docs/roadmap.md`.
- [ ] Stage K: Run documentation and code quality gates successfully.
- [ ] Stage L: Finalize living sections and outcomes in this ExecPlan.

## Surprises & Discoveries

- The repository already contains `crates/whitaker_sarif/` from roadmap item
  7.1.1, but there is currently no `whitaker_clones_core` crate in the
  workspace. This change therefore starts with crate creation, not extension.
- The clone-detector design already reserves `whitaker_clones_core::token` for
  this work, so creating the crate is consistent with the published design.
- Existing successful ExecPlans for roadmap items 6.3.1 and 7.1.1 follow a
  pattern of pure-library implementation first, then behaviour tests, then
  design-doc and roadmap updates.
- The current workspace already pins `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = "0.5.0"`, so no testing-version negotiation is needed.
- Existing behaviour tests confirm two local constraints that matter here:
  integration tests must avoid `expect()` and `unwrap()`, and BDD step
  functions should parse no more than three values to stay under Clippy's
  argument threshold.

## Decision Log

- Decision: implement roadmap 7.2.1 in a new crate
  `crates/whitaker_clones_core/`. Rationale: the design document already
  allocates token and AST engines to that crate, and no such crate exists
  today. Date/Author: 2026-03-08 / Codex.
- Decision: keep 7.2.1 strictly at fingerprint production. Rationale:
  candidate generation and similarity scoring are explicitly split into 7.2.2
  and 7.2.3 on the roadmap. Date/Author: 2026-03-08 / Codex.
- Decision: use explicit domain types for normalization profile, shingle size,
  winnowing window, normalized token, and retained fingerprint rather than raw
  tuples passed throughout the API. Rationale: this improves readability,
  supports validation, and avoids primitive obsession. Date/Author: 2026-03-08
  / Codex.
- Decision: encode invalid-parameter unhappy paths as typed validation errors
  instead of silent coercions. Rationale: the feature request requires unhappy
  path coverage, and zero-valued `k` or window sizes are genuine domain errors.
  Date/Author: 2026-03-08 / Codex.
- Decision: the final implementation must append a distinct
  `## Implementation decisions (7.2.1)` subsection to
  `docs/whitaker-clone-detector-design.md` rather than editing the completed
  7.1.1 subsection in place. Rationale: preserves decision history by roadmap
  item. Date/Author: 2026-03-08 / Codex.

## Outcomes & Retrospective

Not started. On completion, this section must summarise:

1. The final public API exported by `whitaker_clones_core`.
2. The test inventory (unit and BDD scenario counts).
3. The design decisions recorded in
   `docs/whitaker-clone-detector-design.md`.
4. The exact validation commands run and their log paths.
5. Any scope cuts or follow-on work intentionally left for 7.2.2 and later.

## Context and orientation

The repository root is `/home/user/project`. The current workspace members in
`Cargo.toml` are `common`, `crates/*`, `installer`, and `suite`, so a new crate
under `crates/` is picked up automatically. `whitaker_sarif` already exists as
the shared SARIF model crate from roadmap item 7.1.1, but the token engine
crate defined in the clone-detector design does not exist yet.

Relevant reference points for the implementer:

- `docs/roadmap.md` — roadmap item 7.2.1 under “Clone detector pipeline”.
- `docs/whitaker-clone-detector-design.md` — token-pass design, especially
  §Pass A: token engine (`rustc_lexer`), §Safety, scale, and performance notes,
  and the minimal code skeletons near the end of the file.
- `docs/execplans/7-1-1-whitaker-sarif-crate.md` — recent clone-detector
  ExecPlan showing the local format and completion workflow.
- `common/tests/brain_trait_metrics_behaviour.rs` and
  `common/tests/features/brain_trait_metrics.feature` — canonical BDD structure
  in this repository.
- `docs/rstest-bdd-users-guide.md` — step-definition and fixture guidance for
  `rstest-bdd` v0.5.0.
- `docs/rust-doctest-dry-guide.md` — Rustdoc expectations for public examples.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — refactoring
  guidance to keep the token engine split into small, comprehensible units.

The design document currently specifies this sequence for Pass A:

1. Normalize `rustc_lexer` output into `T1` and `T2` profiles.
2. Build `k`-shingles (default `k = 25`) over normalized token symbols.
3. Compute 64-bit Rabin-Karp rolling hashes.
4. Winnow over a fixed window (default `w = 16`) to keep representative
   fingerprints.

Later steps such as MinHash, LSH, Jaccard scoring, SARIF emission, and CLI
commands are intentionally out of scope for this roadmap item.

## Plan of work

### Stage B: Scaffold `whitaker_clones_core`

Create a new library crate at `crates/whitaker_clones_core/` with:

- `Cargo.toml`
- `src/lib.rs`
- `src/token/mod.rs`
- small sibling modules such as `normalize.rs`, `shingle.rs`, `winnow.rs`,
  `types.rs`, and `error.rs` as needed

Add the crate to `[workspace.dependencies]` in the root `Cargo.toml`. Add
`rustc_lexer` to `[workspace.dependencies]` with a caret requirement. Keep all
other dependencies on existing workspace pins unless a concrete reason to
escalate appears.

The initial public surface should be small and composable. One acceptable shape
is:

```rust
pub enum NormProfile {
    T1,
    T2,
}

pub struct NormalizedToken {
    pub symbol: NormalizedTokenSymbol,
    pub range: std::ops::Range<usize>,
}

pub struct ShingleSize(/* validated non-zero usize */);

pub struct WinnowWindow(/* validated non-zero usize */);

pub struct Fingerprint {
    pub hash: u64,
    pub range: std::ops::Range<usize>,
}

pub fn normalize(source: &str, profile: NormProfile) -> Vec<NormalizedToken>;

pub fn hash_shingles(
    tokens: &[NormalizedToken],
    k: ShingleSize,
) -> Result<Vec<Fingerprint>, TokenPassError>;

pub fn winnow(
    fingerprints: &[Fingerprint],
    window: WinnowWindow,
) -> Result<Vec<Fingerprint>, TokenPassError>;
```

The exact names may change during implementation, but the API must preserve
three properties:

1. normalization is deterministic and byte-range-aware;
2. invalid parameters are rejected with typed errors;
3. later stages can consume retained fingerprints without knowing about the
   lexer internals.

### Stage C: Write failing tests first

Add unit tests before the implementation is complete. Keep them close to the
token modules unless an integration-style test is more readable.

Planned unit test matrix:

1. `T1` strips whitespace, line comments, and block comments while preserving
   keyword, punctuation, and delimiter tokens.
2. `T2` canonicalizes identifiers and literals so renamed variables and
   changed literal values normalize to the same symbol stream.
3. Byte ranges on retained normalized tokens still point to the original
   source spans after trivia stripping.
4. `ShingleSize::try_from(0)` and `WinnowWindow::try_from(0)` fail with a
   typed error.
5. Inputs shorter than `k` yield zero hashed shingles.
6. Inputs of exactly `k` normalized tokens yield exactly one hashed shingle.
7. The rolling hash implementation matches a naive recomputation for overlapping
   shingles across several fixtures.
8. Winnowing returns stable minima for a known fingerprint sequence, including
   the chosen tie-breaking rule.
9. Winnowing with fewer fingerprints than the window uses the documented small-
   input behaviour.
10. Re-running the same pipeline on the same source yields byte-for-byte equal
    results.

Use `rstest` to parameterize coverage, but keep each test function under the
workspace argument threshold by bundling expectations into a single case value
where needed.

### Stage D: Implement domain types and validation

Implement the foundational types before the algorithm bodies become complex.
Keep these in focused modules:

- `token::types` for `NormProfile`, validated `ShingleSize`,
  validated `WinnowWindow`, `NormalizedToken`, `NormalizedTokenSymbol`, and
  `Fingerprint`
- `token::error` for typed validation errors

Use newtypes or validated constructors so illegal `k` and window sizes are not
ordinary `usize` values moving through the code. This is the main unhappy-path
surface for 7.2.1 and should stay explicit.

### Stage E: Implement `rustc_lexer` normalization

Build `token::normalize` on `rustc_lexer::tokenize`.

Implementation requirements:

1. Strip whitespace and both comment kinds for all profiles.
2. Preserve byte ranges from the original source for every retained token.
3. For `T1`, preserve the lexical category of non-trivia tokens.
4. For `T2`, canonicalize identifiers and literals while leaving keywords,
   punctuation, and delimiters distinct.
5. Use an explicit internal symbol mapping rather than `TokenKind as u32`.
6. Keep the function small by extracting helpers for trivia detection,
   canonicalization, and symbol mapping.

Document any profile-specific choices that are not explicit in the design
document, such as how raw identifiers are treated under `T2`.

### Stage F: Implement `k`-shingling and Rabin-Karp hashing

Add a focused module for shingle generation and rolling hash support.

Requirements:

1. Generate contiguous token windows of length `k` over the normalized token
   stream.
2. Derive each shingle's byte range from the first token start to the last
   token end.
3. Implement Rabin-Karp rolling hash with a documented constant base, matching
   the design's intent of deterministic 64-bit hashing.
4. Prefer a simple, testable implementation over premature micro-optimisation.
   A naive helper is acceptable for cross-checking in tests.
5. Keep all arithmetic in integer space to satisfy the workspace Clippy policy.

If the implementation needs a `Shingle` type separate from `Fingerprint`, add
it only if it improves readability for later MinHash work in 7.2.2.

### Stage G: Implement deterministic winnowing

Add winnowing as a separate module or sibling functions inside the shingling
module, whichever keeps files shorter and clearer.

Requirements:

1. Accept the hashed shingle sequence and a validated window size.
2. Return retained fingerprints with their original byte ranges intact.
3. Choose and document one deterministic tie-breaking strategy.
4. Define the behaviour for short inputs explicitly:
   either emit the global minimum once or return the whole list; whichever is
   chosen must be consistent with the tests and design-note update.
5. Deduplicate repeated minima only when required by the chosen algorithm; do
   not accidentally discard distinct regions that share a hash value.

This stage should produce the final 7.2.1 output that 7.2.2 will later feed
into MinHash and LSH.

### Stage H: Add behaviour tests with `rstest-bdd` v0.5.0

Create behaviour coverage under the new crate:

- `crates/whitaker_clones_core/tests/token_pass_behaviour.rs`
- `crates/whitaker_clones_core/tests/features/token_pass.feature`

Use a small mutable or interior-mutable world struct, following the existing
repository pattern. Avoid `expect()` and `unwrap()` in the integration test.

Planned scenarios:

1. `T1` normalization removes comments and whitespace from a small function.
2. `T2` normalization causes two renamed code snippets to produce the same
   normalized symbol stream.
3. Hashing a token stream with exactly `k` tokens yields one retained
   fingerprint with the expected byte span.
4. Winnowing over a known hash sequence keeps the documented minima.
5. Invalid `k` or window values are rejected with a readable error.
6. A source shorter than `k` produces no fingerprints without panicking.

Keep feature steps small enough to avoid the Clippy argument-count trap. If a
step would otherwise need too many placeholders, split it into multiple `And`
steps.

### Stage I: Update the design document

Append a new subsection to `docs/whitaker-clone-detector-design.md`:

- `## Implementation decisions (7.2.1)`

Record the final decisions taken during implementation, at minimum:

1. the public crate and module layout;
2. the explicit token-symbol mapping strategy;
3. the invalid-parameter error model;
4. the winnowing tie-breaking rule;
5. the small-input behaviour for `k`-shingling and winnowing.

These notes must describe what was actually shipped, not what was merely
planned.

### Stage J: Mark the roadmap item done

Update `docs/roadmap.md` only after the implementation, documentation updates,
and all quality gates succeed:

- change `- [ ] 7.2.1 ...` to `- [x] 7.2.1 ...`

Do not mark 7.2.2 or later items done.

### Stage K: Run validation and capture evidence

Because the change touches Markdown and Rust code, run both the documentation
gates and the required code gates. Use `tee` and `set -o pipefail` so the logs
survive truncation and preserve the real exit code.

Run:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/7-2-1-fmt.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/7-2-1-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/7-2-1-nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-1-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/7-2-1-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/7-2-1-test.log
```

Expected result:

1. every command exits with status 0;
2. `make test` includes the new unit tests, doctests, and BDD scenarios for
   `whitaker_clones_core`;
3. the Outcomes section of this ExecPlan records the final log paths and any
   useful summary lines.

## Approval gate

This document is the draft-phase output required by the `execplans` skill.
Implementation must not begin until the user explicitly approves the plan or
requests revisions.
