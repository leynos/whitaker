Feature: Decomposition adjacency construction
  The adjacency builder preserves valid similarity edges, keeps neighbour
  indices in bounds, and produces symmetric adjacency lists.

  Scenario: Valid edges produce symmetric neighbour lists
    Given a graph with 4 nodes
    And an edge from 0 to 1 with weight 5
    And an edge from 2 to 3 with weight 9
    When adjacency is built
    Then the adjacency is symmetric
    And all neighbour indices are in bounds

  Scenario: Edges not in canonical order (left < right) are rejected
    Given a graph with 3 nodes
    And an edge from 2 to 1 with weight 5
    When adjacency is built
    Then the build is rejected

  Scenario: Isolated nodes have empty neighbour lists
    Given a graph with 4 nodes
    And an edge from 0 to 2 with weight 7
    When adjacency is built
    Then node 1 has no neighbours
    And node 3 has no neighbours

  Scenario: Multiple neighbours appear in sorted order
    Given a graph with 4 nodes
    And an edge from 0 to 3 with weight 3
    And an edge from 0 to 1 with weight 5
    And an edge from 0 to 2 with weight 8
    When adjacency is built
    Then the neighbours of node 0 are sorted
