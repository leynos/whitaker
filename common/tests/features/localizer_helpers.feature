Feature: Localiser helpers
  Lint helpers should resolve consistent locales and provide deterministic
  fallbacks when localisation data is missing.

  Scenario: Fallback to the bundled locale
    Given DYLINT_LOCALE is not set
    And no configuration locale is provided
    When I request the localizer for "function_attrs_follow_docs"
    Then the resolved locale is "en-GB"

  Scenario: Environment locale is honoured
    Given DYLINT_LOCALE is "cy"
    And no configuration locale is provided
    When I request the localizer for "function_attrs_follow_docs"
    Then the resolved locale is "cy"

  Scenario: Localisation fallback captures missing keys
    Given DYLINT_LOCALE is not set
    And no configuration locale is provided
    And I have requested the localizer for "no_expect_outside_tests"
    And fallback messages are defined
    And a missing message key "missing-key" is requested
    When I resolve the diagnostic message set
    Then the fallback primary message contains "Fallback primary"
    And a delayed bug is recorded mentioning "missing-key"

  Scenario: Localisation succeeds without falling back
    Given DYLINT_LOCALE is not set
    And no configuration locale is provided
    And I have requested the localizer for "function_attrs_follow_docs"
    And I prepare arguments for the doc attribute diagnostic
    And a message key "function_attrs_follow_docs" is requested
    When I resolve the diagnostic message set
    Then the resolved primary message contains "Doc comments"
    And no delayed bug is recorded

  Scenario: Message interpolation failures fall back deterministically
    Given DYLINT_LOCALE is not set
    And no configuration locale is provided
    And I have requested the localizer for "function_attrs_follow_docs"
    And fallback messages are defined
    And I do not prepare arguments for the doc attribute diagnostic
    And a message key "function_attrs_follow_docs" is requested
    When I resolve the diagnostic message set
    Then the fallback primary message contains "Fallback primary"
    And a delayed bug is recorded mentioning "function_attrs_follow_docs"
