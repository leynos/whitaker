Feature: Localised diagnostics for expect usage
  Scenario: English fallback locale
    Given the locale "en-GB" is selected
    And the receiver type is "Result<T, E>"
    And the function context is "handler"
    When I localise the expect diagnostic
    Then the diagnostic mentions "expect on `Result<T, E>`"
    And the note references "function `handler`"
    And the help references "`Result<T, E>`"
    And the help references "`Err` variant"

  Scenario: Welsh messaging
    Given the locale "cy" is selected
    And the receiver type is "Option<String>"
    And the function context is ""
    When I localise the expect diagnostic
    Then the diagnostic mentions "Peidiwch"
    And the note references "Daw’r galwad"

  Scenario: Unsupported locale falls back to English
    Given the locale "zz" is selected
    And the receiver type is "Result<i32, i32>"
    And the call occurs outside any function
    When I localise the expect diagnostic
    Then the diagnostic mentions "expect on `Result<i32, i32>`"
    And the fallback help mentions "Result<i32, i32>"
    And the fallback help mentions "`Err` variant"

  Scenario: Receiver type is empty
    Given the locale "en-GB" is selected
    And the receiver type is empty
    And the function context is ""
    When I localise the expect diagnostic
    Then the fallback and localisation logic should handle the receiver type robustly

  Scenario: Receiver type is malformed
    Given the locale "en-GB" is selected
    And the receiver type is malformed
    And the function context is "worker"
    When I localise the expect diagnostic
    Then the fallback and localisation logic should handle the receiver type robustly

  Scenario: Receiver type is unexpected
    Given the locale "en-GB" is selected
    And the receiver type is unexpected
    And the function context is "handler"
    When I localise the expect diagnostic
    Then the fallback and localisation logic should handle the receiver type robustly

  Scenario: Localisation failure surfaces missing message
    Given localisation fails
    And the receiver type is "Result<(), ()>"
    And the function context is "worker"
    When I localise the expect diagnostic
    Then localisation fails for "no_expect_outside_tests"
