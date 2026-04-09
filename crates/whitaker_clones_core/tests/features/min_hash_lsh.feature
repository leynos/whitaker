Feature: MinHash and LSH candidate generation
  Candidate generation should stay deterministic and stop before token-level
  acceptance or reporting.

  Scenario: Identical retained fingerprints become a candidate pair
    Given LSH bands 1 and rows 128
    And fragment alpha retains hashes 11 22 33
    And fragment beta retains hashes 11 22 33
    When candidate pairs are generated
    Then candidate pair count is 1
    And the only candidate pair is alpha and beta

  Scenario: Different retained fingerprints do not collide
    Given LSH bands 1 and rows 128
    And fragment alpha retains hashes 11 22 33
    And fragment gamma retains hashes 44 55 66
    When candidate pairs are generated
    Then no candidate pairs are returned

  Scenario: Multiple colliding bands still produce one canonical pair
    Given LSH bands 32 and rows 4
    And fragment beta retains hashes 5 7 11 13
    And fragment alpha retains hashes 5 7 11 13
    When candidate pairs are generated
    Then candidate pair count is 1
    And the only candidate pair is alpha and beta

  Scenario: Invalid LSH settings surface a typed error
    Given LSH bands 0 and rows 4
    When candidate pairs are generated
    Then the candidate generation error is ZeroBands

  Scenario: Empty retained fingerprints are rejected explicitly
    Given LSH bands 1 and rows 128
    And fragment empty retains no hashes
    When candidate pairs are generated
    Then the candidate generation error is EmptyFingerprintSet

  Scenario: Zero LSH rows surface a typed error
    Given LSH bands 4 and rows 0
    When candidate pairs are generated
    Then the candidate generation error is ZeroRows

  Scenario: Non-zero invalid LSH products are rejected explicitly
    Given LSH bands 3 and rows 42
    When candidate pairs are generated
    Then the candidate generation error is InvalidBandRowProduct(3,42)
