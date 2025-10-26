Feature: Localised diagnostics for function attribute ordering
  Scenario: English fallback locale
    Given the locale "en-GB" is selected
    And the subject kind is "function"
    And the attribute label is "#[inline]"
    When I localise the diagnostic
    Then the primary message contains "Doc comments"
    And the note mentions "#[inline]"
    And the help mentions "#[inline]"

  Scenario: Welsh locale messaging
    Given the locale "cy" is selected
    And the subject kind is "method"
    And the attribute label is "#[allow(clippy::bool_comparison)]"
    When I localise the diagnostic
    Then the primary message contains "sylwadau doc"
    And the note mentions "#[allow(clippy::bool_comparison)]"

  Scenario: Attribute snippet fallback uses the translated label
    Given the locale "en-GB" is selected
    And the subject kind is "trait method"
    And the attribute snippet cannot be retrieved
    When I localise the diagnostic
    Then the note mentions "the preceding attribute"
    And the help mentions "the preceding attribute"

  Scenario: Unsupported locale falls back to English
    Given the locale "zz" is selected
    And the subject kind is "trait method"
    And the attribute label is "#[allow(dead_code)]"
    When I localise the diagnostic
    Then the primary message contains "Doc comments"

  Scenario: Localisation failure reports missing message
    Given localisation fails
    And the subject kind is "function"
    And the attribute label is "#[cfg(test)]"
    When I localise the diagnostic
    Then localisation fails for "function_attrs_follow_docs"
