Feature: Method metadata extraction for LCOM4
  The extraction builder accumulates field accesses and method calls
  observed during a method body walk, filtering out macro-expanded
  spans to prevent generated code from inflating the cohesion graph.

  Scenario: Field access is recorded
    Given an extraction builder for method process
    And a field access to data not from expansion
    When I build the method info
    Then the accessed fields contain data
    And the called methods are empty

  Scenario: Method call is recorded
    Given an extraction builder for method dispatch
    And a method call to validate not from expansion
    When I build the method info
    Then the called methods contain validate
    And the accessed fields are empty

  Scenario: Macro-expanded field access is filtered
    Given an extraction builder for method render
    And a field access to canvas not from expansion
    And a field access to macro_field from expansion
    When I build the method info
    Then the accessed fields contain canvas
    And the accessed fields do not contain macro_field

  Scenario: Macro-expanded method call is filtered
    Given an extraction builder for method update
    And a method call to helper not from expansion
    And a method call to macro_call from expansion
    When I build the method info
    Then the called methods contain helper
    And the called methods do not contain macro_call

  Scenario: All entries from expansion yields empty sets
    Given an extraction builder for method generated
    And a field access to a from expansion
    And a method call to b from expansion
    When I build the method info
    Then the accessed fields are empty
    And the called methods are empty

  Scenario: Empty builder yields empty method info
    Given an extraction builder for method empty
    When I build the method info
    Then the accessed fields are empty
    And the called methods are empty

  Scenario: Multiple fields and calls accumulate correctly
    Given an extraction builder for method complex
    And a field access to alpha not from expansion
    And a field access to beta not from expansion
    And a method call to do_work not from expansion
    And a method call to validate not from expansion
    When I build the method info
    Then the accessed fields contain alpha
    And the accessed fields contain beta
    And the called methods contain do_work
    And the called methods contain validate
