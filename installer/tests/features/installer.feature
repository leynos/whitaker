Feature: Whitaker lint library installer
  The installer builds, links, and stages Dylint lint libraries for local use.
  It detects the pinned toolchain, builds crates with the required features,
  and copies them to a target directory with toolchain-specific naming.

  Scenario: Resolve suite only by default
    Given no specific lints are requested
    When the crate list is resolved
    Then only the suite crate is included

  Scenario: Resolve individual lints
    Given individual lints mode is enabled
    When the crate list is resolved
    Then all lint crates are included
    And the suite crate is not included

  Scenario: Resolve specific lints
    Given specific lints are requested
    When the crate list is resolved
    Then only the requested lints are included

  Scenario: Validate known crate names
    Given a list of valid crate names
    When the names are validated
    Then validation succeeds

  Scenario: Reject unknown crate names
    Given a list containing an unknown crate name
    When the names are validated
    Then validation fails with a lint not found error

  Scenario: Parse standard toolchain format
    Given a rust-toolchain.toml with standard format
    When the toolchain is detected
    Then the channel is extracted correctly

  Scenario: Parse top-level channel format
    Given a rust-toolchain.toml with top-level channel
    When the toolchain is detected
    Then the channel is extracted correctly

  Scenario: Reject missing channel in toolchain file
    Given a rust-toolchain.toml without a channel
    When the toolchain is detected
    Then detection fails with an invalid file error

  Scenario: Generate shell snippets for all shells
    Given a target library path
    When shell snippets are generated
    Then bash snippet uses export syntax
    And fish snippet uses set -gx syntax
    And PowerShell snippet uses $env syntax

  Scenario: Stage library with toolchain suffix
    Given a built library
    And a staging directory
    When the library is staged
    Then the staged filename includes the toolchain

  Scenario: Reject staging to non-writable directory
    Given a non-writable staging directory
    When the staging directory is prepared
    Then staging fails with a target not writable error

  Scenario: Dry-run outputs configuration
    Given the installer is invoked with dry-run and a target directory
    When the installer CLI is run
    Then the CLI exits successfully
    And dry-run output is shown

  Scenario: Dry-run rejects unknown lint
    Given the installer is invoked with dry-run and an unknown lint
    When the installer CLI is run
    Then the CLI exits with an error
    And an unknown lint message is shown

  Scenario: Install suite to a temporary directory
    Given the installer is invoked to a temporary directory
    When the installer CLI is run
    Then installation succeeds or is skipped
    And the suite library is staged
