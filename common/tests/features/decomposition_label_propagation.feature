Feature: Decomposition label propagation
  Scenario: Disconnected pairs settle to shared labels
    Given methods named beta, alpha, delta, charlie are tracked
    And an edge from 0 to 1 with weight 5
    And an edge from 2 to 3 with weight 5
    And the maximum iteration count is 3
    When label propagation is run
    Then the labels are 1, 1, 3, 3
    And all propagated labels are in bounds

  Scenario: Isolated nodes keep their own labels
    Given methods named alpha, beta, gamma are tracked
    And the maximum iteration count is 4
    When label propagation is run
    Then the labels are 0, 1, 2
    And the graph has no active nodes
    And the propagation uses 0 iterations

  Scenario: Zero iteration bound keeps initial labels
    Given methods named alpha, beta, gamma are tracked
    And an edge from 0 to 1 with weight 5
    And an edge from 1 to 2 with weight 5
    And the maximum iteration count is 0
    When label propagation is run
    Then the labels are 0, 1, 2
    And the propagation uses 0 iterations

  Scenario: Equal scores break ties lexically
    Given methods named gamma, alpha, beta are tracked
    And an edge from 0 to 1 with weight 5
    And an edge from 0 to 2 with weight 5
    And the maximum iteration count is 1
    When label propagation is run
    Then the labels are 1, 1, 1
    And all propagated labels are in bounds

  Scenario: Invalid edge input is rejected
    Given methods named alpha, beta are tracked
    And an edge from 1 to 0 with weight 5
    And the maximum iteration count is 1
    When label propagation is run
    Then the propagation input is rejected
