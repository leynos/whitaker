# Documentation contents

- [Documentation contents](contents.md) explains how the Whitaker documentation
  set is organized and helps readers find the right document quickly.

## Guides

- [User's guide](users-guide.md) explains how to install, configure, and use
  Whitaker and its lints.
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
- [Whitaker command-line interface (CLI) design](whitaker-cli-design.md)
  describes the command-line surface and design choices for installer and
  operator workflows.
- [Whitaker clone detector design](whitaker-clone-detector-design.md)
  documents the clone detector architecture, data flow, and supporting
  reasoning.
- [Brain trust lints design](brain-trust-lints-design.md) describes the lints
  that analyse trait and type structure to surface decomposition guidance.

## User-facing and technical reference material

- Rstest Behaviour-Driven Development (BDD) user's guide
  <rstest-bdd-users-guide.md> explains how to use `rstest-bdd` effectively in
  Whitaker-oriented Rust test suites.
- [Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md)
  explains fixture-oriented testing patterns used throughout the repository.
- Reliable testing in Rust via dependency injection
  <reliable-testing-in-rust-via-dependency-injection.md> explains the
  repository's preferred approach to testable Rust design.
- [Rust doctest dry guide](rust-doctest-dry-guide.md) explains how to keep
  Rust doctests accurate without unnecessary execution.
- Complexity antipatterns and refactoring strategies
  <complexity-antipatterns-and-refactoring-strategies.md> records the
  complexity patterns Whitaker flags and the refactorings they are intended to
  encourage.
- Lints for rstest fixtures and test hygiene
  <lints-for-rstest-fixtures-and-test-hygiene.md> explains the lint rationale
  and coding guidance around fixture-heavy tests.
- Local validation of GitHub Actions with act and pytest
  <local-validation-of-github-actions-with-act-and-pytest.md> explains how to
  exercise workflow logic locally before pushing changes.

## Decision records

- Architectural decision record (ADR) 001: prebuilt Dylint libraries
  <adr-001-prebuilt-dylint-libraries.md> records the decision to ship prebuilt
  Dylint libraries and the constraints that follow from that release model.
- Architectural decision record (ADR) 002: Dylint `expect` attribute macro
  <adr-002-dylint-expect-attribute-macro.md> records the decision and migration
  guidance for the `#[expect(...)]` attribute-macro approach.
- Architectural decision record (ADR) 003: formal proof strategy for clone
  detector pipeline
  <adr-003-formal-proof-strategy-for-clone-detector-pipeline.md> records the
  formal verification direction for the clone detector pipeline and its proof
  boundaries.

## Planning material

- [docs/execplans/](execplans/) stores execution plans for branch-scoped
  implementation work and larger follow-up tasks.
