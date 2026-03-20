Feature: Token-pass normalization and fingerprinting
  The token pass strips trivia, canonicalizes Type-2 fragments, hashes
  contiguous token shingles, and retains deterministic fingerprints.

  Scenario: Type-1 normalization removes trivia
    Given the source snippet commented_function
    And the profile is T1
    When the source is normalized
    Then the normalized labels are fn demo ( x : i32 ) { x + 1 }

  Scenario: Type-2 normalization matches renamed functions
    Given the source snippet renamed_function_a
    And the comparison source snippet renamed_function_b
    And the profile is T2
    When both sources are normalized
    Then the normalized labels match exactly
    Then the normalized labels are fn <ID_0> ( <ID_1> : <ID_2> ) { <ID_1> + <NUM> }

  Scenario: Exact k tokens produce one fingerprint with a stable span
    Given the source snippet short_function
    And the profile is T2
    And shingle size 6
    When fingerprints are generated
    Then the fingerprint count is 1
    And the first fingerprint spans 0 to 12

  Scenario: Winnowing keeps the rightmost minimum
    Given the known fingerprint sequence rightmost_minimum
    And winnow window 3
    When fingerprints are winnowed
    Then the retained hashes are 4

  Scenario: Invalid shingle size is rejected
    Given shingle size 0
    Then the error is shingle size must be greater than zero

  Scenario: Unterminated literals fail normalization
    Given the source snippet unterminated_string
    And the profile is T1
    When the source is normalized
    Then the error is unterminated string literal at byte range 12..17
