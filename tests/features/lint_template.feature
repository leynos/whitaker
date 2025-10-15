Feature: Lint crate template
  Whitaker exposes helpers to generate new lint crates with consistent
  manifests and UI harness boilerplate so authors can focus on lint logic.

  Scenario: Rendering a template for a documented lint crate
    Given the lint crate name is function_attrs_follow_docs
    And the UI tests directory is ui
    When I render the lint crate template
    Then the manifest declares a cdylib crate type
    And the manifest reuses shared dependencies
    And the library includes UI test harness boilerplate for directory ui
    And the lint constant is named FUNCTION_ATTRS_FOLLOW_DOCS

  Scenario: Rendering a template with nested UI directories
    Given the lint crate name is lint_nested_ui
    And the UI tests directory is ui/lints/cases
    When I render the lint crate template
    Then the manifest reuses shared dependencies
    And the library includes UI test harness boilerplate for directory ui/lints/cases

  Scenario: Rendering a template with Windows separators in the UI directory
    Given the lint crate name is lint_windows_path
    And the UI tests directory is ui\windows\cases
    When I render the lint crate template
    Then the manifest reuses shared dependencies
    And the library includes UI test harness boilerplate for directory ui/windows/cases

  Scenario: Rejecting blank crate names
    Given the lint crate name is blank
    When I render the lint crate template
    Then template creation fails with an empty crate name error

  Scenario: Rejecting crate names starting with digits
    Given the lint crate name is 1foo
    When I render the lint crate template
    Then template creation fails with a crate name starting with a non-letter

  Scenario: Rejecting crate names with trailing separators
    Given the lint crate name is lint-
    When I render the lint crate template
    Then template creation fails due to a trailing separator -

  Scenario: Rejecting absolute UI directories
    Given the lint crate name is module_max_400_lines
    And the UI tests directory is /tmp/ui
    When I render the lint crate template
    Then template creation fails with an absolute UI directory error pointing to /tmp/ui

  Scenario: Rejecting absolute Windows UI directories
    Given the lint crate name is module_max_400_lines
    And the UI tests directory is C:\\temp\\ui
    When I render the lint crate template
    Then template creation fails with an absolute UI directory error pointing to C:\\temp\\ui

  Scenario: Rejecting UNC absolute UI directories
    Given the lint crate name is function_attrs_follow_docs
    And the UI tests directory is //server/share/ui
    When I render the lint crate template
    Then template creation fails with an absolute UI directory error pointing to //server/share/ui

  Scenario: Rejecting drive-relative Windows UI directories
    Given the lint crate name is module_max_400_lines
    And the UI tests directory is C:ui
    When I render the lint crate template
    Then template creation fails with an absolute UI directory error pointing to C:ui

  Scenario: Rejecting parent directory segments in the UI directory
    Given the lint crate name is module_max_400_lines
    And the UI tests directory is ui/../secrets
    When I render the lint crate template
    Then template creation fails because the UI directory traverses upwards

  Scenario: Rejecting invalid crate name characters
    Given the lint crate name is noUnwrapOrElsePanic
    When I render the lint crate template
    Then template creation fails with an invalid crate name character U

  Scenario: Rejecting blank UI test directories
    Given the lint crate name is module_max_400_lines
    And the UI tests directory is blank
    When I render the lint crate template
    Then template creation fails with an empty UI directory error

  Scenario: Rejecting crate names with trailing underscores
    Given the lint crate name is lint_
    When I render the lint crate template
    Then template creation fails due to a trailing separator _
