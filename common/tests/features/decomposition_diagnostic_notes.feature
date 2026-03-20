Feature: Decomposition diagnostic notes
  Brain type and brain trait diagnostics render concise decomposition notes
  from clustered method communities.

  Scenario: Type note renders grammar, serde, and filesystem areas
    Given note rendering for a type named Foo
    And the parser, serde, and filesystem methods are tracked
    When the decomposition diagnostic note is rendered
    Then the note is present
    And the note contains line "- [grammar] helper struct for `parse_nodes`, `parse_tokens`"
    And the note contains line "- [serde::json] module for `decode_json`, `encode_json`"
    And the note contains line "- [std::fs] module for `load_from_disk`, `save_to_disk`"

  Scenario: Trait note renders focused sub-traits
    Given note rendering for a trait named Transport
    And the transport serde and io methods are tracked
    When the decomposition diagnostic note is rendered
    Then the note is present
    And the note contains line "- [serde::json] sub-trait for `decode_request`, `encode_request`"
    And the note contains line "- [std::io] sub-trait for `read_frame`, `write_frame`"

  Scenario: No suggestions yields no note
    Given note rendering for a type named SmallThing
    And a method named alpha
    And method alpha uses external domains serde::json
    And a method named beta
    And method beta uses external domains std::fs
    When the decomposition diagnostic note is rendered
    Then there is no note

  Scenario: Large subjects cap the number of rendered areas
    Given note rendering for a type named Coordinator
    And a method named grammar_alpha
    And method grammar_alpha accesses fields grammar
    And a method named grammar_beta
    And method grammar_beta accesses fields grammar
    And a method named serde_alpha
    And method serde_alpha uses external domains serde::json
    And a method named serde_beta
    And method serde_beta uses external domains serde::json
    And a method named io_alpha
    And method io_alpha uses external domains std::io
    And a method named io_beta
    And method io_beta uses external domains std::io
    And a method named fs_alpha
    And method fs_alpha uses external domains std::fs
    And a method named fs_beta
    And method fs_beta uses external domains std::fs
    When the decomposition diagnostic note is rendered
    Then the note is present
    And the note contains line "1 more area omitted"
    And the note does not contain "[std::io]"

  Scenario: Large communities cap rendered method names
    Given note rendering for a type named Reporter
    And a method named report_alpha
    And method report_alpha accesses fields report
    And a method named report_beta
    And method report_beta accesses fields report
    And a method named report_delta
    And method report_delta accesses fields report
    And a method named report_epsilon
    And method report_epsilon accesses fields report
    And a method named report_gamma
    And method report_gamma accesses fields report
    And a method named io_alpha
    And method io_alpha uses external domains std::io
    And a method named io_beta
    And method io_beta uses external domains std::io
    When the decomposition diagnostic note is rendered
    Then the note is present
    And the note contains line "- [report] helper struct for `report_alpha`, `report_beta`, `report_delta`, +2 more methods"
