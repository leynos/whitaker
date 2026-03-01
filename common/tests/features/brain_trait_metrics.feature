Feature: Brain trait metric collection
  Trait metric collection tracks interface size, required-method burden,
  and default method cognitive complexity for brain trait analysis.

  Scenario: Mixed trait items aggregate all signals
    Given a trait named Parser
    And a required method parse
    And a required method validate
    And a default method render with CC 12
    And an associated type Output
    And an associated const VERSION
    When trait metrics are built
    Then total trait items is 5
    And required method count is 2
    And default method count is 1
    And default method CC sum is 12
    And implementor burden is 2

  Scenario: Traits without default methods keep default complexity at zero
    Given a trait named Reader
    And a required method read
    And an associated type Output
    When trait metrics are built
    Then total trait items is 2
    And required method count is 1
    And default method count is 0
    And default method CC sum is 0
    And implementor burden is 1

  Scenario: Empty traits produce zeroed metrics
    Given a trait named EmptyTrait
    When trait metrics are built
    Then total trait items is 0
    And required method count is 0
    And default method count is 0
    And default method CC sum is 0
    And implementor burden is 0

  Scenario: Macro-expanded default methods are filtered
    Given a trait named FilteredTrait
    And a required method parse
    And a default method generated_helper with CC 30 from expansion
    And a default method render with CC 12
    When trait metrics are built
    Then total trait items is 2
    And required method count is 1
    And default method count is 1
    And default method CC sum is 12
    And implementor burden is 1

  Scenario: Implementor burden only tracks required methods
    Given a trait named Transformer
    And a required method parse
    And a required method validate
    And a default method normalise with CC 7
    And an associated const VERSION
    When trait metrics are built
    Then total trait items is 4
    And required method count is 2
    And default method count is 1
    And default method CC sum is 7
    And implementor burden is 2

  Scenario: Traits with only default methods have zero burden
    Given a trait named Convenience
    And a default method render with CC 5
    And a default method validate with CC 9
    When trait metrics are built
    Then total trait items is 2
    And required method count is 0
    And default method count is 2
    And default method CC sum is 14
    And implementor burden is 0
