Feature: Candidate selection for missing_docs_in_test_functions

  Scenario: An undocumented module function in a test harness is diagnosed
    Given a module function in a selected source file
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is diagnose

  Scenario: A doc attribute with an include_str value suppresses the diagnostic
    Given a module function in a selected source file
    And the compilation is a test harness
    And the function has a doc attribute with an include_str value
    When the candidate predicate is evaluated
    Then the verdict is skip because the function is documented

  Scenario: Methods in trait implementations are out of scope
    Given an associated function inside a trait implementation
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is skip because the function context is out of scope