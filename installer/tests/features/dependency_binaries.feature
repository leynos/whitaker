Feature: Dependency binary installation
  Repository-hosted dependency binaries are preferred before Cargo fallback.

  Scenario: Install cargo-dylint from the repository release
    Given the missing tool is "cargo-dylint"
    And the repository installer succeeds
    When dependency installation runs
    Then the install succeeds
    And stderr contains "Installed cargo-dylint from repository release."

  Scenario: Install dylint-link from the repository release
    Given the missing tool is "dylint-link"
    And the repository installer succeeds
    When dependency installation runs
    Then the install succeeds
    And stderr contains "Installed dylint-link from repository release."

  Scenario: Repository asset is unavailable and cargo install builds from source
    Given the missing tool is "cargo-dylint"
    And the repository installer fails with "not found"
    And cargo binstall is available
    When dependency installation runs
    Then the install succeeds
    And stderr contains "Installed cargo-dylint from source with cargo install."

  Scenario: Repository asset is unavailable and cargo install failure propagates
    Given the missing tool is "cargo-dylint"
    And the repository installer fails with "not found"
    And cargo binstall is available
    And cargo binstall fails with "binstall failed"
    And cargo install fails with "cargo install failed"
    When dependency installation runs
    Then the install fails for "cargo-dylint" with message containing "cargo install failed"

  Scenario: Repository asset and cargo binstall are unavailable and cargo install succeeds
    Given the missing tool is "cargo-dylint"
    And the repository installer fails with "not found"
    And cargo binstall is unavailable
    When dependency installation runs
    Then the install succeeds
    And stderr contains "Installed cargo-dylint from source with cargo install."

  Scenario: Repository asset and cargo binstall are unavailable and cargo install fails
    Given the missing tool is "cargo-dylint"
    And the repository installer fails with "not found"
    And cargo binstall is unavailable
    And cargo install fails with "cargo install failed"
    When dependency installation runs
    Then the install fails for "cargo-dylint" with message containing "cargo install failed"

  Scenario: Repository install fails verification and cargo binstall is used
    Given the missing tool is "cargo-dylint"
    And the repository installer succeeds but verification fails
    And cargo binstall is available
    When dependency installation runs
    Then the install succeeds
    And stderr contains "failed verification"
    And stderr contains "Installed cargo-dylint with cargo binstall."

  Scenario: Unsupported target skips the repository path and uses cargo binstall
    Given the missing tool is "cargo-dylint"
    And the target is unsupported
    And cargo binstall is available
    When dependency installation runs
    Then the install succeeds
    And stderr contains "Installed cargo-dylint with cargo binstall."

  Scenario: Repository install succeeds when cargo binstall is unavailable
    Given the missing tool is "cargo-dylint"
    And the repository installer succeeds
    And cargo binstall is unavailable
    When dependency installation runs
    Then the install succeeds
    And stderr contains "Installed cargo-dylint from repository release."

  Scenario: Provenance document lists both dependencies
    Given the dependency manifest is loaded
    When provenance markdown is rendered
    Then the provenance contains "https://github.com/trailofbits/dylint"
    And the provenance contains "cargo-dylint"
    And the provenance contains "dylint-link"
