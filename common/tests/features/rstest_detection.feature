Feature: Strict rstest detection

  Scenario: Detect an rstest test from a direct attribute
    Given a function annotated with rstest
    When I check whether the function is an rstest test
    Then the function is recognised as an rstest test

  Scenario: Detect an rstest fixture from a direct attribute
    Given a function annotated with rstest::fixture
    When I check whether the function is an rstest fixture
    Then the function is recognised as an rstest fixture

  Scenario: Classify a plain identifier parameter as fixture-local
    Given a parameter named db
    When I classify the parameter
    Then the parameter is classified as fixture-local
    And the fixture-local names contain db

  Scenario: Classify a case parameter as provider-driven
    Given a parameter named case_input annotated with case
    When I classify the parameter
    Then the parameter is classified as provider-driven

  Scenario: Leave unsupported parameter bindings unsupported
    Given a destructured parameter binding
    When I classify the parameter
    Then the parameter is classified as unsupported

  Scenario: Ignore expansion traces while fallback is disabled
    Given the expansion trace contains rstest
    When I check whether the function is an rstest test
    Then the function is recognised as not being an rstest test

  Scenario: Use expansion traces when fallback is enabled
    Given the expansion trace contains rstest
    And expansion fallback is enabled
    When I check whether the function is an rstest test
    Then the function is recognised as an rstest test
