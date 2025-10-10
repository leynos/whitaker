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

  Scenario: Rejecting blank crate names
    Given the lint crate name is blank
    When I render the lint crate template
    Then template creation fails with an empty crate name error

  Scenario: Rejecting absolute UI directories
    Given the lint crate name is module_max_400_lines
    And the UI tests directory is /tmp/ui
    When I render the lint crate template
    Then template creation fails with an absolute UI directory error pointing to /tmp/ui

  Scenario: Rejecting invalid crate name characters
    Given the lint crate name is noUnwrapOrElsePanic
    When I render the lint crate template
    Then template creation fails with an invalid crate name character U
