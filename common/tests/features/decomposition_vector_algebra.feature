Feature: Decomposition vector algebra
  The decomposition vector helpers preserve the algebraic properties that the
  cosine-threshold proof relies on.

  Scenario: Shared features preserve dot-product commutativity
    Given a left method named parse_tokens
    And the left method accesses fields grammar
    And a right method named parse_nodes
    And the right method accesses fields grammar
    When the vector algebra is evaluated
    Then the dot product is commutative
    And the left squared norm is 44
    And the right squared norm is 44

  Scenario: Empty feature vectors still have non-negative squared norms
    Given a left method named build_render
    And a right method named parse_nodes
    And the right method accesses fields grammar
    When the vector algebra is evaluated
    Then the left squared norm is 0
    And the right squared norm is 44

  Scenario: Methods without overlapping positive features have zero dot product
    Given a left method named parse_tokens
    And the left method accesses fields grammar
    And a right method named save_to_disk
    And the right method uses external domains std::fs
    When the vector algebra is evaluated
    Then the dot product is zero
