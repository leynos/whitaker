Feature: Register the experimental string continuation style lint

  Scenario: Registering the early pass
    Given an empty lint store
    When I register the suite lints
    Then the early pass count is 1
    And the early pass is the string continuation style pass
    And every early lint descriptor has a registered early pass
