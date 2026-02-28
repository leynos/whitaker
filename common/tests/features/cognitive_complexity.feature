Feature: Cognitive complexity with macro-expansion filtering
  The cognitive complexity builder computes SonarSource-style CC
  incrementally, excluding macro-expanded nodes to prevent inflated
  scores from generated code.

  Scenario: Empty function has zero complexity
    Given a new complexity builder
    When the complexity is finalised
    Then the complexity score is 0

  Scenario: Single if adds one structural increment
    Given a new complexity builder
    And a structural increment not from expansion
    And a nesting increment not from expansion
    When the complexity is finalised
    Then the complexity score is 1

  Scenario: Nested if adds nesting-depth penalty
    Given a new complexity builder
    And a structural increment not from expansion
    And a nesting increment not from expansion
    And nesting is pushed not from expansion
    And a structural increment not from expansion
    And a nesting increment not from expansion
    And nesting is popped
    When the complexity is finalised
    Then the complexity score is 3

  Scenario: Macro-expanded structural increment is excluded
    Given a new complexity builder
    And a structural increment from expansion
    When the complexity is finalised
    Then the complexity score is 0

  Scenario: Macro-expanded nesting does not inflate depth
    Given a new complexity builder
    And nesting is pushed from expansion
    And a structural increment not from expansion
    And a nesting increment not from expansion
    And nesting is popped
    When the complexity is finalised
    Then the complexity score is 1

  Scenario: Boolean operators add fundamental increments
    Given a new complexity builder
    And a structural increment not from expansion
    And a fundamental increment not from expansion
    And a fundamental increment not from expansion
    When the complexity is finalised
    Then the complexity score is 3

  Scenario: Mixed real and expansion increments
    Given a new complexity builder
    And a structural increment not from expansion
    And nesting is pushed not from expansion
    And a structural increment from expansion
    And a structural increment not from expansion
    And a nesting increment not from expansion
    And nesting is popped
    When the complexity is finalised
    Then the complexity score is 3

  Scenario: Fundamental increment from expansion is excluded
    Given a new complexity builder
    And a fundamental increment from expansion
    When the complexity is finalised
    Then the complexity score is 0
