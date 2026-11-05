Feature: Register the Whitaker lint suite

  Scenario: Registering into an empty lint store
    Given an empty lint store
    When I register the suite lints
    Then registration succeeds
    And the store has the suite lints registered
    And the late pass count is 1
    And the lint names mirror the suite descriptors
    And the suite lint declarations align with the descriptors

  Scenario: Registering the early pass
    Given an empty lint store
    When I register the suite lints
    Then the early pass count is 1
    And the early pass is the string continuation style pass
    And every early lint descriptor has a registered early pass

  Scenario: Registering twice surfaces a duplicate lint error
    Given the suite lints are already registered
    When I register the suite lints
    Then registration fails with a duplicate lint error
