Feature: Detect example patterns in test documentation

  Scenario: Detect an examples heading
    Given documentation line "# Examples"
    When I evaluate the documentation
    Then the violation is examples heading

  Scenario: Detect a fenced code block
    Given documentation line "```rust"
    And documentation line "assert!(true);"
    And documentation line "```"
    When I evaluate the documentation
    Then the violation is code fence

  Scenario: Ignore inline backticks
    Given documentation line "Use `ticks` inline only."
    When I evaluate the documentation
    Then there is no violation

  Scenario: Ignore plain prose
    Given documentation line "No examples are documented here."
    When I evaluate the documentation
    Then there is no violation

  Scenario: Preserve source-order precedence
    Given documentation line "```"
    And documentation line "let value = 1;"
    And documentation line "```"
    And documentation line "# Examples"
    When I evaluate the documentation
    Then the violation is code fence
