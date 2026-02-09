Feature: Artefact naming, manifest schema, and verification policy
  Prebuilt lint library artefacts follow the naming, manifest, and
  verification rules defined in ADR-001.

  Scenario: Construct artefact name from valid components
    Given a git SHA "abc1234"
    And a toolchain channel "nightly-2025-09-18"
    And a target triple "x86_64-unknown-linux-gnu"
    When an artefact name is constructed
    Then the filename is "whitaker-lints-abc1234-nightly-2025-09-18-x86_64-unknown-linux-gnu.tar.zst"

  Scenario: Reject unsupported target triple
    Given an invalid target triple "wasm32-unknown-unknown"
    Then the target triple is rejected

  Scenario: Accept all five supported target triples
    Given all supported target triples
    Then every triple is accepted

  Scenario: Reject invalid git SHA
    Given an invalid git SHA "XYZ"
    Then the git SHA is rejected

  Scenario: Reject empty toolchain channel
    Given an empty toolchain channel
    Then the toolchain channel is rejected

  Scenario: Construct manifest with all fields
    Given a complete set of manifest fields
    When a manifest is constructed
    Then all manifest fields are accessible

  Scenario: Default verification policy requires checksum
    Given the default verification policy
    Then checksum verification is required

  Scenario: Verification failure triggers fallback
    Given the default failure action
    Then the action is fallback with warning
