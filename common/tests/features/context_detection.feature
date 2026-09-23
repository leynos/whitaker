Feature: Context detection

  Scenario: Recognize rstest decorated functions
    Given a function annotated with rstest
    When I check whether the function is test-like
    Then the function is recognized as test-like
    And its context is marked as test-like

  Scenario: Recognize tokio::test decorated functions
    Given a function annotated with tokio::test
    When I check whether the function is test-like
    Then the function is recognized as test-like
    And its context is marked as test-like

  Scenario: Ignore plain functions
    Given a function without test attributes
    When I check whether the function is test-like
    Then the function is recognized as not test-like
    And its context is not marked as test-like

  Scenario: Recognize configured custom test attribute
    Given the lint recognizes custom::test as a test attribute
    And a function annotated with the custom test attribute custom::test
    When I check whether the function is test-like
    Then the function is recognized as test-like
    And its context is marked as test-like
