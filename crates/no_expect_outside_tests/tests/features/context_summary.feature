Feature: Summarise traversal context for `.expect(..)` linting

  Scenario: Plain function without test attributes
    Given a non-test function named handler
    When I summarise the context
    Then the context is marked as production
    And the function name is handler

  Scenario: Function marked as a test
    Given a test function named works
    When I summarise the context
    Then the context is marked as test
    And the function name is works

  Scenario: Module guarded by cfg(test)
    Given a module with cfg(test)
    When I summarise the context
    Then the context is marked as test
    And no function name is recorded

  Scenario: Function recognised via configured attribute
    Given an additional test attribute custom::test is configured
    And a function annotated with the additional attribute custom::test
    When I summarise the context
    Then the context is marked as test
    And the function name is custom
