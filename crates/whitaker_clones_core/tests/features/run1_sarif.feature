Feature: SARIF Run 1 Type-3 refinement
  The AST pass refines token-pass results and adds near-miss clones.

  Scenario: A near-miss pair missed by the token pass is added as Type-3
    Given the Rust fragments near_miss_left and near_miss_right
    And the token pass does not accept the pair
    When the AST pass refines the candidates
    Then Run 1 contains a WHK003 result with baseline state new
    And the result records a cosine of at least 0.85
    And the result carries an astHash partial fingerprint

  Scenario: A rename-equivalent Type-2 pair is carried unchanged
    Given the Rust fragments renamed_left and renamed_right
    And the token pass accepts the pair as WHK002
    When the AST pass refines the candidates
    Then Run 1 contains a WHK002 result with baseline state unchanged
    And the result records a cosine of 1.000000

  Scenario: A Type-2 pair with differing structure is reclassified
    Given the Rust fragments reshaped_left and reshaped_right
    And the token pass accepts the pair as WHK002
    When the AST pass refines the candidates
    Then Run 1 contains a WHK003 result with baseline state updated
    And the result keeps its Run 0 whitakerFragment fingerprint
    And the result records token rule WHK002

  Scenario: A near-exact Type-1 pair with differing structure is reclassified
    Given the Rust fragments near_exact_left and near_exact_right
    And the token pass accepts the pair as WHK001
    When the AST pass refines the candidates
    Then Run 1 contains a WHK003 result with baseline state updated
    And the result keeps its Run 0 whitakerFragment fingerprint
    And the result records token rule WHK001

  Scenario: A renamed pair under both profiles is reported once
    Given the Rust fragments renamed_left and renamed_right under both profiles
    And the token pass accepts only the T2 pair
    When the AST pass refines the candidates
    Then Run 1 contains exactly 1 result

  Scenario: Structurally unrelated candidates are not reported
    Given the Rust fragments unrelated_left and unrelated_right
    And the token pass does not accept the pair
    When the AST pass refines the candidates
    Then Run 1 contains no result for the pair

  Scenario: An unparsable fragment keeps its token verdict
    Given the Rust fragments renamed_left and broken_right
    And lowering broken_right reports an unparsable span
    And the token pass accepts the pair as WHK002
    When the AST pass refines the candidates
    Then Run 1 contains a WHK002 result with baseline state unchanged
    And the result has no cosine and no astHash

  Scenario: Misordered thresholds are rejected
    Given a type3 threshold of 0.95 and a type2 threshold of 0.90
    When the AST pass refines the candidates
    Then refinement fails with a threshold order error