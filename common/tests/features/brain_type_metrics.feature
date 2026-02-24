Feature: Brain type metric collection
  Metric collection for brain type detection computes WMC (the sum of
  cognitive complexity across methods), identifies brain methods (those
  exceeding both CC and LOC thresholds), and aggregates type-level
  metrics including LCOM4 and foreign reach.

  Scenario: WMC is the sum of all method complexities
    Given a method called parse with CC 30 and LOC 100
    And a method called validate with CC 15 and LOC 40
    When I compute WMC
    Then the WMC is 45

  Scenario: A method qualifies as a brain method
    Given a method called parse with CC 30 and LOC 100
    And the brain method CC threshold is 25
    And the brain method LOC threshold is 80
    When I identify brain methods
    Then parse is a brain method

  Scenario: A method below both thresholds is not a brain method
    Given a method called helper with CC 5 and LOC 20
    And the brain method CC threshold is 25
    And the brain method LOC threshold is 80
    When I identify brain methods
    Then there are no brain methods

  Scenario: A method meeting only the CC threshold is not a brain method
    Given a method called complex_but_short with CC 30 and LOC 40
    And the brain method CC threshold is 25
    And the brain method LOC threshold is 80
    When I identify brain methods
    Then there are no brain methods

  Scenario: A method meeting only the LOC threshold is not a brain method
    Given a method called simple_long with CC 10 and LOC 100
    And the brain method CC threshold is 25
    And the brain method LOC threshold is 80
    When I identify brain methods
    Then there are no brain methods

  Scenario: Empty type has zero WMC
    When I compute WMC
    Then the WMC is 0

  Scenario: Type metrics aggregate all signals
    Given a method called parse with CC 30 and LOC 100
    And a method called validate with CC 15 and LOC 40
    And the brain method CC threshold is 25
    And the brain method LOC threshold is 80
    And the LCOM4 value is 2
    And the foreign reach count is 5
    When I build type metrics for MyType
    Then the type WMC is 45
    And the type has 1 brain method
    And the type LCOM4 is 2
    And the type foreign reach is 5

  Scenario: Foreign references are deduplicated
    Given a foreign reference to std::collections
    And a foreign reference to std::collections
    And a foreign reference to serde::Deserialize
    When I compute foreign reach
    Then the foreign reach is 2

  Scenario: Macro-expanded foreign references are filtered
    Given a foreign reference to std::fmt from expansion
    And a foreign reference to serde::Serialize not from expansion
    When I compute foreign reach
    Then the foreign reach is 1
