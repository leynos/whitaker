Feature: Register the experimental late lints

  Scenario: Registering the macro-aware late pass after an opt-in
    Given an empty lint store
    And the repository opts in to macro_aware_cognitive_complexity
    When I register the suite lints
    Then the late pass count is 2
    And the early pass count is 0
    And the macro-aware cognitive complexity lint is declared

  Scenario: Registering the rstest helper late pass after an opt-in
    Given an empty lint store
    And the repository opts in to rstest_helper_should_be_fixture
    When I register the suite lints
    Then the late pass count is 2
    And the early pass count is 0

  Scenario: Registering every experimental pass when all are enabled
    Given an empty lint store
    And every experimental lint is enabled
    When I register the suite lints
    Then the late pass count is 3
    And the early pass count is 1
    And the store has the suite lints registered
