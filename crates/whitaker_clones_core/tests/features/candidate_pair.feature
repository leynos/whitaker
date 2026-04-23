Feature: Candidate pair canonicalization
  CandidatePair::new should suppress self-pairs and emit distinct pairs in
  canonical lexical order.

  Scenario: Distinct IDs already in canonical order stay unchanged
    Given input fragment IDs alpha and beta
    When the candidate pair constructor is called
    Then the canonical pair is alpha and beta

  Scenario: Reversed distinct IDs are canonicalized
    Given input fragment IDs beta and alpha
    When the candidate pair constructor is called
    Then the canonical pair is alpha and beta

  Scenario: Identical IDs are suppressed
    Given input fragment IDs alpha and alpha
    When the candidate pair constructor is called
    Then no candidate pair is returned

  Scenario: Similar IDs follow lexical order rather than numeric order
    Given input fragment IDs fragment-2 and fragment-10
    When the candidate pair constructor is called
    Then the canonical pair is fragment-10 and fragment-2
