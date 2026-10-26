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
- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  records the complexity patterns Whitaker flags and the refactorings they are
  intended to encourage.
- [Lints for rstest fixtures and test hygiene](lints-for-rstest-fixtures-and-test-hygiene.md)
  explains the lint rationale and coding guidance around fixture-heavy tests.
- [Local validation of GitHub Actions with act and pytest](local-validation-of-github-actions-with-act-and-pytest.md)
  explains how to exercise workflow logic locally before pushing changes.

## Decision records

- [Architectural decision record (ADR) 001: prebuilt Dylint
  libraries](adr-001-prebuilt-dylint-libraries.md) records the decision to ship
  prebuilt Dylint libraries and the constraints that follow from that release
  model.
- [Architectural decision record (ADR) 002: Dylint `expect` attribute
  macro](adr-002-dylint-expect-attribute-macro.md) records the support-macro
  decision, migration phases, and known limitations for conditional
  `#[expect(...)]` usage.
- [Architectural decision record (ADR) 003: formal proof strategy for clone
  detector
  pipeline](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md)
  records the formal verification direction for the clone detector pipeline and
  its proof boundaries.
- [Architectural decision record (ADR) 004: pin-only mutation-testing
  contract](adr-004-pin-only-mutation-testing-contract.md) records why the
  shared mutation workflow tests its declared configuration without claiming
  parity with the continuous integration test baseline.
- [Architectural decision record (ADR) 005: adopt the shared CV-005 contract
  library](adr-005-adopt-the-shared-cv005-contract-library.md) records the move
  from a local copy of the CV-005 contract to the pinned shared check.
- [Architectural decision record (ADR) 006: directory-based module
  layout](adr-006-directory-based-module-layout.md) records the `mod.rs` module
  convention and the extraction of oversized test modules into sibling files,
  and states plainly that the 400-line budget is not yet machine-enforced here.
- [Architectural decision record (ADR) 007: Rust source rewrite
  substrate](adr-007-rust-rewrite-materialization-substrate.md) defines a
  bounded comparative spike between ast-grep and `ra_ap_syntax::SyntaxEditor`,
  with source-fidelity gates and an evidence-based backend selection rule.

## Requests for comments

- [RFC 0001: lint candidates derived from CodeRabbit review
  findings](rfcs/0001-coderabbit-derived-lint-candidates.md) proposes new
  Whitaker lints distilled from recurring CodeRabbit review findings in Rust
  code.
- [RFC 0002: `string_continuation_style`](rfcs/0002-string-continuation-style.md)
  proposes a context-sensitive lint that prefers `concat!()` only when it can
  preserve a cooked string literal's semantics and surrounding grammar.
- [RFC 0003: `missing_docs_in_test_functions`](rfcs/0003-missing-docs-in-test-functions.md)
  proposes a pre-expansion lint that requires documentation for source-authored
  functions in configured Rust test-source paths without inspecting generated
  test code.
- [RFC 0006: nested `cargo` lints for tests](rfcs/0006-nested-cargo-lints.md)
  proposes three late-pass lints for tests that spawn `cargo` or `rustc` with a
  private target directory, directly, or without a time bound.
- [RFC 0007: compiler-validated rewrite
  checking](rfcs/0007-compiler-validated-rewrite-checking.md) proposes isolated
  overlay compilation that classifies materialized rewrites under equivalent
  NLL and Polonius configurations.
- [RFC 0008: borrow-workaround lint
  family](rfcs/0008-borrow-workaround-lint-family.md) proposes rewrite-backed
  rules for repeated lookups, remove-and-reinsert cycles, staged identifiers,
  snapshots, lexical fences, and other indirect borrowing shapes.
- [RFC 0009: validated ownership and borrow
  rewriter](rfcs/0009-validated-ownership-and-borrow-rewriter.md) proposes
  recipe materialization, alternative ranking, deterministic previews,
  transactional application, and byte-exact rollback.
- [RFC 0010: macro-aware cognitive complexity](rfcs/0010-macro-aware-cognitive-complexity.md)
  proposes a cognitive complexity lint that charges only author-written
  constructs, and a shared authored-versus-generated classifier for every
  complexity-sensitive Whitaker lint.
- [RFC 0011: weak representations at domain
  boundaries](rfcs/0011-weak-domain-boundaries.md) proposes the `DOMAIN` lint
  family and rules for discarded parser witnesses, validation without
  refinement, and repeated boundary parsing.
- [RFC 0012: fragile types and invalid-state
  encodings](rfcs/0012-fragile-types.md) proposes rules for manual sum types,
  mixed presence constraints, finite Boolean and sentinel state, redundant
  dimensions, primitive-to-domain projections, invariant surfaces, and local
  terminal protocols.
- [RFC 0012 amendment: state-machine ADT and protocol
  coverage](rfcs/0012-state-machine-adt-amendment.md) normatively broadens the
  RFC 0012 rule contracts and establishes mdtablefix issues 443–449 as the
  acceptance corpus.
- [Phase 17 DOMAIN roadmap amendment](roadmap-phase-17-domain-amendment.md)
  supersedes conflicting Phase 17 state-space tasks with the expanded RFC 0012
  rule catalogue and the mdtablefix 443–449 acceptance corpus.

## Planning material

- [docs/execplans/](execplans/) stores execution plans for branch-scoped
  implementation work and larger follow-up tasks.
- [Issue #523 OrthoConfig adoption plan](execplans/issue-523-adopt-ortho-config-v0-10-0-for-the-whitaker-cli.md)
  records the proposed scope, baseline, implementation sequence, and gates.
