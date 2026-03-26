Feature: Decomposition cosine threshold
  The decomposition similarity threshold accepts sufficiently similar methods
  and safely rejects zero-norm edge cases.

  Scenario: Strongly overlapping methods satisfy the threshold
    Given a left method named parse_tokens
    And the left method accesses fields grammar
    And a right method named parse_nodes
    And the right method accesses fields grammar
    When the cosine threshold is evaluated
    Then the methods are considered similar

  Scenario: Shared low-weight keywords remain below the threshold
    Given a left method named render_state
    And the left method accesses fields grammar
    And a right method named build_state
    And the right method uses external domains std::fs
    When the cosine threshold is evaluated
    Then the methods are not considered similar

  Scenario: Empty feature vectors do not satisfy the threshold
    Given a left method named build_render
    And a right method named parse_tokens
    And the right method accesses fields grammar
    When the cosine threshold is evaluated
    Then the methods are not considered similar
