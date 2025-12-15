Feature: Consumer guidance documentation
  Documentation provides valid examples for workspace configuration and
  installer usage. These scenarios validate that documented TOML examples
  parse correctly.

  Scenario: Suite-only workspace metadata is valid TOML
    Given a workspace metadata example for suite-only
    When the TOML is parsed
    Then parsing succeeds
    And the libraries pattern is "suite"

  Scenario: Individual crates workspace metadata is valid TOML
    Given a workspace metadata example for individual crates
    When the TOML is parsed
    Then parsing succeeds
    And the libraries pattern is "crates/*"

  Scenario: Version-pinned workspace metadata with tag is valid TOML
    Given a workspace metadata example with tag pinning
    When the TOML is parsed
    Then parsing succeeds
    And the tag field is present

  Scenario: Version-pinned workspace metadata with revision is valid TOML
    Given a workspace metadata example with revision pinning
    When the TOML is parsed
    Then parsing succeeds
    And the revision field is present

  Scenario: Pre-built library path workspace metadata is valid TOML
    Given a workspace metadata example with pre-built path
    When the TOML is parsed
    Then parsing succeeds
    And the path field is present

  Scenario: dylint.toml lint configuration is valid TOML
    Given a dylint.toml example with lint configuration
    When the TOML is parsed
    Then parsing succeeds
    And module_max_lines configuration is present
    And conditional_max_n_branches configuration is present
