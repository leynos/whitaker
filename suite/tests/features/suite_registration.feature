Feature: Register the Whitaker lint suite

  Scenario: Registering into an empty lint store
    Given an empty lint store
    When I register the suite lints
    Then registration succeeds
    And the store has the suite lints registered
    And the late pass count is 1
    And the lint names mirror the suite descriptors
    And the suite lint declarations align with the descriptors

  Scenario: Registering no experimental pass without an opt-in
    Given an empty lint store
    When I register the suite lints
    Then the early pass count is 0
    And the late pass count is 1
    And every selected early lint descriptor has a registered early pass
    And the macro-aware cognitive complexity lint is declared

  Scenario: Registering twice surfaces a duplicate lint error
    Given the suite lints are already registered
    When I register the suite lints
    Then registration fails with a duplicate lint error
