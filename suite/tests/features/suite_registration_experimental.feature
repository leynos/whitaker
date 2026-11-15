Feature: Register the experimental string continuation style lint

  Scenario: Registering the early pass after an opt-in
    Given an empty lint store
    And the repository opts in to string_continuation_style
    When I register the suite lints
    Then the early pass count is 1
    And the early pass is the string continuation style pass
    And every selected early lint descriptor has a registered early pass
    And the late pass count is 1
