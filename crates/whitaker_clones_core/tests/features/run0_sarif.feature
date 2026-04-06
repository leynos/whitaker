Feature: Token-pass Run 0 SARIF emission
  Accepted token-pass pairs should emit deterministic SARIF Run 0 results for
  Type-1 and Type-2 clones, while rejected or malformed inputs should stay
  explicit.

  Scenario: Accepted Type-1 pair emits one WHK001 result
    Given token fragment alpha_t1 is loaded
    And token fragment beta_t1 is loaded
    And candidate pair alpha_t1 and beta_t1 is queued
    When Run 0 is emitted
    Then exactly 1 result is emitted
    And the emitted rule is WHK001
    And the result has 1 primary location and 1 related location

  Scenario: Accepted Type-2 pair emits Whitaker Type-2 properties
    Given token fragment alpha_t2 is loaded
    And token fragment beta_t2 is loaded
    And candidate pair alpha_t2 and beta_t2 is queued
    When Run 0 is emitted
    Then exactly 1 result is emitted
    And the emitted rule is WHK002
    And the Whitaker profile is T2
    And the Whitaker k is 25
    And the Whitaker window is 16

  Scenario: Below-threshold pair emits no results
    Given token fragment alpha_t2 is loaded
    And token fragment beta_t2_partial is loaded
    And candidate pair alpha_t2 and beta_t2_partial is queued
    When Run 0 is emitted
    Then no results are emitted

  Scenario: Empty retained fingerprints fail before emission
    Given token fragment alpha_empty is loaded
    And token fragment beta_t1 is loaded
    And candidate pair alpha_empty and beta_t1 is queued
    When Run 0 is emitted
    Then the emission error is token fragment `alpha_empty` must retain at least one fingerprint

  Scenario: Multi-line ranges become 1-based SARIF regions
    Given token fragment alpha_multiline is loaded
    And token fragment beta_multiline is loaded
    And candidate pair alpha_multiline and beta_multiline is queued
    When Run 0 is emitted
    Then the primary region is 2:1-3:1

  Scenario: Reversed pair input still emits one deterministic result
    Given token fragment alpha_t1 is loaded
    And token fragment beta_t1 is loaded
    And candidate pair beta_t1 and alpha_t1 is queued
    When Run 0 is emitted
    Then exactly 1 result is emitted
    And the primary file is src/a.rs
