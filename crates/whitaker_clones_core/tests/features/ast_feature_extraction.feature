Feature: AST feature extraction
  The AST pass maps candidate spans to parser nodes and extracts stable
  feature inputs for Type-3 refinement.

  Scenario: Smallest covering expression is selected
    Given the source snippet add_function
    And the candidate span snippet add_expression
    When the candidate span is lowered
    Then the lowered root kind is BIN_EXPR

  Scenario: Identifier-renamed fragments share an AST hash
    Given the left source snippet renamed_function_a
    And the right source snippet renamed_function_b
    When both whole sources are lowered and hashed
    Then the AST hashes match

  Scenario: Structurally different fragments have different AST hashes
    Given the left source snippet renamed_function_a
    And the right source snippet different_structure
    When both whole sources are lowered and hashed
    Then the AST hashes differ
