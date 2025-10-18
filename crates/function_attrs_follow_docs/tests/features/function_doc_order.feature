Feature: Function doc comments must precede attributes

  Scenario: Accept doc comments before attributes
    Given a doc comment before other attributes
    When I evaluate the attribute order
    Then the order is accepted

  Scenario: Reject doc comments after attributes
    Given a doc comment after an attribute
    When I evaluate the attribute order
    Then the order is rejected

  Scenario: Allow functions without doc comments
    Given attributes without doc comments
    When I evaluate the attribute order
    Then the order is accepted

  Scenario: Ignore inner attributes when ordering docs
    Given a doc comment after an inner attribute
    When I evaluate the attribute order
    Then the order is accepted
