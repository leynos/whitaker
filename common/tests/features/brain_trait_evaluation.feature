Feature: Brain trait threshold evaluation
  Threshold evaluation determines whether a trait qualifies as a brain
  trait based on total method count and default method complexity. The
  warn rule requires both conditions to hold simultaneously (AND-based).
  The deny rule fires on method count alone (OR-based). Deny supersedes
  warn.

  Scenario: Trait within all limits passes
    Given a trait called Foo with 5 required and 5 default methods
    And default method CC sum of 20
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    Then the disposition is pass

  Scenario: All warn conditions trigger a warning
    Given a trait called Foo with 12 required and 8 default methods
    And default method CC sum of 40
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    Then the disposition is warn

  Scenario: Many methods alone does not trigger warn
    Given a trait called Foo with 15 required and 5 default methods
    And default method CC sum of 10
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    Then the disposition is pass

  Scenario: High CC alone does not trigger warn
    Given a trait called Foo with 5 required and 5 default methods
    And default method CC sum of 60
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    Then the disposition is pass

  Scenario: Method count at deny threshold triggers deny
    Given a trait called Foo with 20 required and 10 default methods
    And default method CC sum of 0
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    Then the disposition is deny

  Scenario: Deny supersedes warn
    Given a trait called Foo with 20 required and 10 default methods
    And default method CC sum of 50
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    Then the disposition is deny

  Scenario: Associated items do not count as methods
    Given a trait called Foo with 19 required and 0 default methods
    And 5 associated types and 5 associated consts
    And default method CC sum of 0
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    Then the disposition is pass

  Scenario: Diagnostic surfaces measured values
    Given a trait called BigParser with 15 required and 10 default methods
    And default method CC sum of 52
    And the default brain trait thresholds
    When brain trait thresholds are evaluated
    And the diagnostic message is formatted
    Then the primary message contains 25 methods
    And the primary message contains CC=52
    And the primary message contains BigParser
