Feature: Context detection

  Scenario: Recognise rstest decorated functions
    Given a function annotated with rstest
    When I check whether the function is test-like
    Then the function is recognised as test-like
    And its context is marked as test-like

  Scenario: Ignore plain functions
    Given a function without test attributes
    When I check whether the function is test-like
    Then the function is recognised as not test-like
    And its context is not marked as test-like

  Scenario: Recognise configured custom test attribute
    Given the lint recognises custom::test as a test attribute
    And a function annotated with the custom test attribute custom::test
    When I check whether the function is test-like
    Then the function is recognised as test-like
    And its context is marked as test-like
