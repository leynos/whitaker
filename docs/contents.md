# Documentation contents

- [Documentation contents](contents.md) explains how the Whitaker documentation
  set is organized and helps readers find the right document quickly.

## Guides

- [User's guide](users-guide.md) explains how to install, configure, and use
  Whitaker and its lints.
- [Whitaker 0.3.0 migration guide](v0-3-0-migration-guide.md) lists the
  source-breaking public API renames and upgrade mappings for library consumers.
- [Developer's guide](developers-guide.md) explains how to build, test, verify,
  and extend Whitaker as a maintainer or contributor.
- [Repository layout](repository-layout.md) maps the main directories, crates,
  and support files in the repository so contributors can orient themselves
  before editing code.
- [Documentation style guide](documentation-style-guide.md) defines the writing,
  formatting, and document-structure rules used across the documentation set.
- [Publishing guide](publishing.md) explains how to validate release artefacts
  and publish Whitaker packages.
- [Roadmap](roadmap.md) tracks planned work, implementation phases, and larger
  changes that have not yet landed.

## Primary design documents

- [Whitaker Dylint suite design](whitaker-dylint-suite-design.md) explains the
  architecture and rationale behind the core lint suite.
- [Ownership shape lints design](ownership-shape-lints-design.md) describes
  the clone-pressure and local shared-ownership lints planned for the
  ownership-shape phase.
- [Async-trait architecture hygiene Dylint suite design for Whitaker](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  defines the trait-family analysis, lint catalogue, and rollout model for
  async dyn-dispatch hygiene checks.
- [Technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  explains the split between lightweight Dylint detectors and the richer
  workspace analysis needed to inventory dead or over-suppressed test support
  code.
- [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  defines the architecture-boundary lints and the Cargo graph checker for
  compile-time hygiene policy.
- [Whitaker command-line interface (CLI) design](whitaker-cli-design.md)
  defines the unified `whitaker` binary, its configuration model, and the
  installation and diagnostic workflows behind it.
- [OrthoConfig v0.10.0 provenance](ortho-config-provenance.md) records the
  immutable upstream source and checksums for the imported OrthoConfig guides.
- [OrthoConfig v0.10.0 user's guide](ortho-config-users-guide.md) is the
  byte-preserved upstream guide used to design the CLI configuration spine.
- [OrthoConfig v0.10.0 migration guide](ortho-config-v0-10-0-migration-guide.md)
  is a compatibility entry point to the byte-preserved release guide, stored
  under `ortho-config/` so its relative links resolve to the correct guide.
- [OrthoConfig v0.10.0 legacy migration link target](v0-10-0-migration-guide.md)
  preserves the original relative link in the imported user's guide.
- [OrthoConfig ADR 0004 compatibility entry point](adrs/adr-0004-cargo-external-subcommand-entry-point.md)
  links to the upstream Cargo external-subcommand decision; it is not a
  Whitaker design decision.
- [Whitaker clone detector design](whitaker-clone-detector-design.md)
  documents the clone detector architecture, data flow, and supporting
  reasoning.
- [Brain trust lints design](brain-trust-lints-design.md) describes the lints
  that analyse trait and type structure to surface decomposition guidance.

## Release migration notes

- [Whitaker 0.2.8 migration](migrations/0.2.8.md) explains the released
  suite-version pinning option and the checkout behaviour that affects existing
  installer workflows.

## User-facing and technical reference material

- [Rstest Behaviour-Driven Development (BDD) user's guide](rstest-bdd-users-guide.md)
  explains how to use `rstest-bdd` effectively in Whitaker-oriented Rust test
  suites.
- [Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md)
  explains fixture-oriented testing patterns used throughout the repository.
- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  explains the repository's preferred approach to testable Rust design.
- [Rust doctest dry guide](rust-doctest-dry-guide.md) explains how to keep
  Rust doctests accurate without unnecessary execution.
- [OrthoConfig user's guide](ortho-config-users-guide.md) explains how to build
  layered command-line configuration with OrthoConfig.
- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  records the complexity patterns Whitaker flags and the refactorings they are
  intended to encourage.
- [Lints for rstest fixtures and test hygiene](lints-for-rstest-fixtures-and-test-hygiene.md)
  explains the lint rationale and coding guidance around fixture-heavy tests.
- [Local validation of GitHub Actions with act and pytest](local-validation-of-github-actions-with-act-and-pytest.md)
  explains how to exercise workflow logic locally before pushing changes.

## Decision records

The `docs/adrs/` directory holds architectural decision records and
compatibility entry points. Local ADR filenames follow
`adr-nnnn-title-slug.md`, as the
[documentation style guide](documentation-style-guide.md) prescribes.

- [Architectural decision record (ADR) 0001: prebuilt Dylint
  libraries](adrs/adr-0001-prebuilt-dylint-library-distribution.md) records the
  decision to ship prebuilt Dylint libraries and the constraints that follow
  from that release model.
- [Architectural decision record (ADR) 0002: Dylint `expect` attribute
  macro](adrs/adr-0002-attribute-macro-for-conditional-dylint-expect.md)
  records the support-macro decision, migration phases, and known limitations
  for conditional `#[expect(...)]` usage.
- [Architectural decision record (ADR) 0003: formal proof strategy for clone
  detector
  pipeline](adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md)
  records the formal verification direction for the clone detector pipeline
  and its proof boundaries.
- [Architectural decision record (ADR) 0004: pin-only mutation-testing
  contract](adrs/adr-0004-pin-only-mutation-testing-contract.md) records why
  the shared mutation workflow tests its declared configuration without
  claiming parity with the continuous integration test baseline.
- [Architectural decision record (ADR) 0005: adopt the shared CV-005 contract
  library](adrs/adr-0005-adopt-the-shared-cv-005-contract-library.md) records
  the move from a local copy of the CV-005 contract to the pinned shared check.
- [Architectural decision record (ADR) 0006: directory-based module
  layout](adrs/adr-0006-directory-based-module-layout.md) records the `mod.rs`
  module convention and the extraction of oversized test modules into sibling
  files, and states plainly that the 400-line budget is not yet
  machine-enforced here.
- [Architectural decision record (ADR) 0007: Rust source rewrite
  substrate](adrs/adr-0007-rust-rewrite-materialization-substrate.md) defines a
  bounded comparative spike between ast-grep and `ra_ap_syntax::SyntaxEditor`,
  with source-fidelity gates and an evidence-based backend selection rule.
- [Architectural decision record (ADR) 0008: OrthoConfig CLI boundary](adrs/adr-0008-ortho-config-cli-boundary.md)
  records the configuration sources, compatibility bridge, and application
  boundary for the planned CLI.
- [Architectural decision record (ADR) 0009: verified Clippy snapshot for the
  Dylint driver
  bootstrap](adrs/adr-0009-verified-clippy-snapshot-for-the-dylint-driver-bootstrap.md)
  records the move to Dylint 6.3.0, the commit-named Clippy bundle that driver
  builds read instead of public Clippy, first-use driver construction in an
  identity-keyed directory, and the `whitaker-installer run` dispatcher.
- [Architectural decision record (ADR) 010: prove the lint's position, not its
  macro's identity](adr-010-early-lint-pass-position-proof.md) records the
  post-expansion early pass, the wrapper-depth admission test that closes the
  `$l:literal` hole `is_root()` leaves open, the built-in macro set, the
  crate-wide shadow switch, and the separate `register_early_pass` wiring the
  suite needs.
- [Architectural decision record (ADR) 011: root Whitaker CLI boundary](adr-011-whitaker-cli-boundary.md)
  records the executable namespace, installer API boundary, Dylint forwarding,
  and staged publication decision for the unified CLI.

## Requests for comments

The `docs/rfcs/` directory holds requests for comments (RFCs). Each filename
follows `rfc-nnnn-title-slug.md`, as the
[documentation style guide](documentation-style-guide.md) prescribes.

- [RFC 0001: lint candidates derived from CodeRabbit review
  findings](rfcs/rfc-0001-lint-candidates-derived-from-coderabbit-review-findings.md)
  proposes new Whitaker lints distilled from recurring CodeRabbit review
  findings in Rust code.
- [RFC 0002: `string_continuation_style`](rfcs/rfc-0002-string-continuation-style.md)
  proposes a context-sensitive lint that prefers `concat!()` only when it can
  preserve a cooked string literal's semantics and surrounding grammar.
- [RFC 0003: `missing_docs_in_test_functions`](rfcs/rfc-0003-missing-docs-in-test-functions.md)
  proposes a pre-expansion lint that requires documentation for source-authored
  functions in configured Rust test-source paths without inspecting generated
  test code.
- [RFC 0006: nested `cargo` lints for tests](rfcs/rfc-0006-nested-cargo-lints-for-tests.md)
  proposes three late-pass lints for tests that spawn `cargo` or `rustc` with a
  private target directory, directly, or without a time bound.
- [RFC 0007: compiler-validated rewrite
  checking](rfcs/rfc-0007-compiler-validated-rewrite-checking.md) proposes
  isolated overlay compilation that classifies materialized rewrites under
  equivalent NLL and Polonius configurations.
- [RFC 0008: borrow-workaround lint
  family](rfcs/rfc-0008-borrow-workaround-lint-family.md) proposes
  rewrite-backed rules for repeated lookups, remove-and-reinsert cycles, staged
  identifiers, snapshots, lexical fences, and other indirect borrowing shapes.
- [RFC 0009: validated ownership and borrow
  rewriter](rfcs/rfc-0009-validated-ownership-and-borrow-rewriter.md) proposes
  recipe materialization, alternative ranking, deterministic previews,
  transactional application, and byte-exact rollback.
- [RFC 0010: macro-aware cognitive complexity](rfcs/rfc-0010-macro-aware-cognitive-complexity.md)
  proposes a cognitive complexity lint that charges only author-written
  constructs, and a shared authored-versus-generated classifier for every
  complexity-sensitive Whitaker lint.
- [RFC 0011: weak representations at domain
  boundaries](rfcs/rfc-0011-weak-representations-at-domain-boundaries.md)
  proposes the `DOMAIN` lint family and rules for discarded parser witnesses,
  validation without refinement, and repeated boundary parsing.
- [RFC 0012: fragile types and invalid-state
  encodings](rfcs/rfc-0012-fragile-types-and-invalid-state-encodings.md)
  proposes rules for manual sum types, mixed presence constraints, finite
  Boolean and sentinel state, redundant dimensions, primitive-to-domain
  projections, invariant surfaces, and local terminal protocols.
- [RFC 0012 amendment: state-machine ADT and protocol
  coverage](rfcs/rfc-0012-amendment-state-machine-adt-and-protocol-coverage.md)
  normatively broadens the RFC 0012 rule contracts and establishes mdtablefix
  issues 443–449 as the acceptance corpus.
- [Phase 17 DOMAIN roadmap amendment](roadmap-phase-17-domain-amendment.md)
  supersedes conflicting Phase 17 state-space tasks with the expanded RFC 0012
  rule catalogue and the mdtablefix 443–449 acceptance corpus.

## Planning material

- [docs/execplans/](execplans/) and [debugging plans](debugging/) record
  implementation plans and bounded investigations.
- [Issue #523 OrthoConfig adoption plan](execplans/issue-523-adopt-ortho-config-v0-10-0-for-the-whitaker-cli.md)
  records the proposed scope, baseline, implementation sequence, and gates.
- [Issue #514 debugging note](debugging/debugging-plan-20261024T200138Z.md)
  records the cause, baseline evidence, and approval gate.

## Verification evidence

- [Clone-detector proof run for PR 365 (2026-10-25)](
  verification/clone-detector/pr365-2026-10-25/README.md) records bounded
  exact-head Kani runs for candidates `bf9aa109` and `e100b422`, with harness
  results and limits on what the proofs establish.
- [Earlier clone-detector proof run for PR 365 (2026-10-19)](
  verification/clone-detector/pr365-2026-10-19/README.md) retains the earlier
  Kani transcripts, the unwind-5 negative control, and the related Verus
  record. The 2026-10-25 record supersedes it for later exact-head Kani
  candidate results; the earlier control and Verus evidence remain distinct
  records.
