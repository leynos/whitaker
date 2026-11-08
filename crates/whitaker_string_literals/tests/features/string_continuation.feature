Feature: Rust string-literal continuation analysis

  The pure domain crate classifies source continuations and builds validated
  replacements for joins.

  Scenario: An ordinary source-line join prefers concat
    Given a cooked string expression with an interior continuation
    And the literal is in a general expression context
    When the continuation is classified
    Then the disposition is PreferConcat

  Scenario: An implicit format capture requires the direct literal
    Given a source-authored format string with an interior continuation
    And rustc classified one argument as Captured
    When the continuation is classified
    Then the disposition is RequireContinuation with reason ImplicitFormatCapture

  Scenario: A leading continuation expresses source layout
    Given a cooked string whose first token after the opening quote is a continuation
    When the continuation is classified
    Then the disposition is RequireContinuation with reason LeadingOrTrailingLayoutTrim
