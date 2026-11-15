Feature: Register the experimental macro-aware cognitive complexity lint

  Scenario: Registering the late pass
    Given an empty lint store
    When I register the suite lints
    Then the late pass count is 1
    And the late pass declares the macro-aware cognitive complexity lint