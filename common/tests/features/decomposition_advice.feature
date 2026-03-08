Feature: Decomposition advice analysis
  Community detection groups related methods into reusable decomposition
  suggestions for brain type and brain trait diagnostics.

  Scenario: Type methods split into parsing, serialisation, and filesystem groups
    Given decomposition analysis for a type named Foo
    And a method named parse_tokens
    And method parse_tokens accesses fields grammar,tokens
    And method parse_tokens uses signature types TokenStream
    And a method named parse_nodes
    And method parse_nodes accesses fields grammar,ast
    And method parse_nodes uses local types ParseState
    And a method named encode_json
    And method encode_json uses external domains serde::json
    And method encode_json uses signature types Serializer
    And a method named decode_json
    And method decode_json uses external domains serde::json
    And method decode_json uses signature types Deserializer
    And a method named load_from_disk
    And method load_from_disk uses external domains std::fs
    And method load_from_disk uses local types PathBuf
    And a method named save_to_disk
    And method save_to_disk uses external domains std::fs
    And method save_to_disk uses local types PathBuf
    When decomposition suggestions are generated
    Then suggestion count is 3
    And there is a helper struct suggestion labelled grammar containing methods parse_nodes,parse_tokens
    And there is a module suggestion labelled serde::json containing methods decode_json,encode_json
    And there is a module suggestion labelled std::fs containing methods load_from_disk,save_to_disk

  Scenario: Trait methods suggest focused sub-traits
    Given decomposition analysis for a trait named Transport
    And a method named encode_request
    And method encode_request uses external domains serde::json
    And a method named decode_request
    And method decode_request uses external domains serde::json
    And a method named read_frame
    And method read_frame uses external domains std::io
    And method read_frame uses signature types IoBuffer
    And a method named write_frame
    And method write_frame uses external domains std::io
    And method write_frame uses signature types IoBuffer
    When decomposition suggestions are generated
    Then suggestion count is 2
    And there is a sub trait suggestion labelled serde::json containing methods decode_request,encode_request
    And there is a sub trait suggestion labelled std::io containing methods read_frame,write_frame

  Scenario: Weakly related methods produce no suggestions
    Given decomposition analysis for a type named SmallThing
    And a method named alpha
    And method alpha uses external domains serde::json
    And a method named beta
    And method beta uses external domains std::fs
    When decomposition suggestions are generated
    Then suggestion count is 0

  Scenario: Singleton noise methods are excluded from suggestions
    Given decomposition analysis for a type named Foo
    And a method named parse_tokens
    And method parse_tokens accesses fields grammar,tokens
    And a method named parse_nodes
    And method parse_nodes accesses fields grammar,ast
    And a method named encode_json
    And method encode_json uses external domains serde::json
    And a method named decode_json
    And method decode_json uses external domains serde::json
    And a method named run
    When decomposition suggestions are generated
    Then suggestion count is 2
    And there is no suggestion labelled run

  Scenario: Local-type communities stay stable when added out of order
    Given decomposition analysis for a type named Reporter
    And a method named render_summary
    And method render_summary uses local types SummaryState
    And a method named build_report
    And method build_report uses local types ReportState
    And a method named render_report
    And method render_report uses local types ReportState
    And a method named build_summary
    And method build_summary uses local types SummaryState
    When decomposition suggestions are generated
    Then suggestion count is 2
    And there is a helper struct suggestion labelled report containing methods build_report,render_report
    And there is a helper struct suggestion labelled summary containing methods build_summary,render_summary
