Feature: Brain type threshold evaluation
  Threshold evaluation determines whether a type qualifies as a brain
  type based on WMC, brain method count, and LCOM4. The warn rule
  requires all conditions to hold simultaneously (AND-based). The deny
  rule fires when any single condition holds (OR-based). Deny
  supersedes warn.

  Scenario: Type within all limits passes
    Given a type called Foo with WMC 30 and 0 brain methods
    And the type has LCOM4 1
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is pass

  Scenario: All warn conditions trigger a warning
    Given a type called Foo with WMC 60 and 1 brain methods
    And the type has LCOM4 2
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is warn

  Scenario: High WMC alone does not trigger a warning
    Given a type called Foo with WMC 80 and 0 brain methods
    And the type has LCOM4 1
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is pass

  Scenario: Brain method without high WMC does not trigger a warning
    Given a type called Foo with WMC 30 and 1 brain methods
    And the type has LCOM4 2
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is pass

  Scenario: WMC at deny threshold triggers deny
    Given a type called Foo with WMC 100 and 0 brain methods
    And the type has LCOM4 1
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is deny

  Scenario: Multiple brain methods trigger deny
    Given a type called Foo with WMC 60 and 2 brain methods
    And the type has LCOM4 1
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is deny

  Scenario: High LCOM4 triggers deny
    Given a type called Foo with WMC 30 and 0 brain methods
    And the type has LCOM4 3
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is deny

  Scenario: Deny supersedes warn
    Given a type called Foo with WMC 100 and 2 brain methods
    And the type has LCOM4 3
    And the default brain type thresholds
    When brain type thresholds are evaluated
    Then the disposition is deny

  Scenario: Diagnostic surfaces measured values
    Given a type called Foo with WMC 118 and 1 brain methods
    And the type has LCOM4 3
    And a brain method called parse_all with CC 31 and LOC 140
    And the default brain type thresholds
    When brain type thresholds are evaluated
    And the diagnostic message is formatted
    Then the primary message contains WMC=118
    And the primary message contains LCOM4=3
    And the primary message contains parse_all
