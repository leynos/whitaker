Feature: Policy evaluation for no_unwrap_or_else_panic
  The lint should fire when a panicking unwrap_or_else fallback appears in
  production code or tests and stay quiet for safe, doctest, or
  test-with-interpolation contexts.

  Scenario: Panicking fallback outside tests
    Given a panicking unwrap_or_else fallback outside tests
    When the lint policy is evaluated
    Then the lint triggers

  Scenario: Panicking fallback inside a test
    Given a panicking unwrap_or_else fallback
    And code runs inside a test
    When the lint policy is evaluated
    Then the lint triggers

  Scenario: Panicking fallback inside main when allowed
    Given a panicking unwrap_or_else fallback outside tests
    And code runs inside main
    And allow in main is enabled
    When the lint policy is evaluated
    Then the lint is skipped

  Scenario: Safe fallback outside tests
    Given the fallback is safe
    When the lint policy is evaluated
    Then the lint is skipped

  Scenario: Panicking fallback during doctest execution
    Given a panicking unwrap_or_else fallback
    And a doctest harness is active
    When the lint policy is evaluated
    Then the lint is skipped

  Scenario: Interpolated panic inside a test is permitted
    Given a panicking unwrap_or_else fallback
    And the panic message interpolates a value
    And code runs inside a test
    When the lint policy is evaluated
    Then the lint is skipped

  Scenario: Plain panic inside a test is still flagged
    Given a panicking unwrap_or_else fallback
    And code runs inside a test
    When the lint policy is evaluated
    Then the lint triggers
