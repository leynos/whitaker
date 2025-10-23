Feature: Localisation loader
  Whitaker lints require localised diagnostics with predictable fallbacks.

  Scenario: Falling back to the bundled locale when no preference is provided
    Given no locale preference
    When I request the message for function_attrs_follow_docs
    Then the resolved locale is en-GB
    And the loader reports fallback usage
    And the message contains Doc comments

  Scenario: Resolving an alternate locale with attribute lookups
    Given the locale preference cy
    When I request the attribute note on function_attrs_follow_docs
    Then the resolved locale is cy
    And the message contains Mae’r briodoledd

  Scenario: Resolving Scottish Gaelic plural forms
    Given the locale preference gd
    When I request the attribute note on conditional_max_two_branches with branches 3
    Then the resolved locale is gd
    And the message contains meuran

  Scenario: Welsh lint count handles zero
    Given the locale preference cy
    When I request the attribute note on common-lint-count with lint count 0
    Then the message contains dim lint

  Scenario: Welsh lint count handles one
    Given the locale preference cy
    When I request the attribute note on common-lint-count with lint count 1
    Then the message contains 1 lint

  Scenario: Welsh lint count handles two
    Given the locale preference cy
    When I request the attribute note on common-lint-count with lint count 2
    Then the message contains dau lint

  Scenario: Welsh lint count handles three
    Given the locale preference cy
    When I request the attribute note on common-lint-count with lint count 3
    Then the message contains 3 lint

  Scenario: Welsh lint count handles six
    Given the locale preference cy
    When I request the attribute note on common-lint-count with lint count 6
    Then the message contains 6 lint

  Scenario: Welsh lint count handles eleven
    Given the locale preference cy
    When I request the attribute note on common-lint-count with lint count 11
    Then the message contains 11 lint

  Scenario: Falling back to English for untranslated attributes
    Given the locale preference cy
    When I request the attribute fallback-note on common-lint-count
    Then the message contains Fallback diagnostics default to English

  Scenario: Surfacing a missing message error for unknown keys
    Given the locale preference en-GB
    When I request the message for imaginary.lint
    Then localisation fails with a missing message error
