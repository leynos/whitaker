Feature: Predicate branch evaluation
  Conditional limits guard complex boolean expressions.

  Scenario: Branches remain within the configured limit
    Given the branch limit is 2
    And the predicate declares 2 branches
    When I evaluate the predicate complexity
    Then the predicate is accepted

  Scenario: Branches equal the limit
    Given the branch limit is 3
    And the predicate declares 3 branches
    When I evaluate the predicate complexity
    Then the predicate is accepted

  Scenario: Branches exceed the limit
    Given the branch limit is 2
    And the predicate declares 3 branches
    When I evaluate the predicate complexity
    Then the predicate is rejected

  Scenario: Branch limit tightened to a single branch
    Given the branch limit is 1
    And the predicate declares 2 branches
    When I evaluate the predicate complexity
    Then the predicate is rejected
