Feature: LCOM4 cohesion analysis
  LCOM4 counts connected components in a method relationship graph to
  measure type cohesion. A result of 1 indicates high cohesion; 2 or
  more indicates the type bundles unrelated responsibilities.

  Scenario: Single method is always cohesive
    Given a method called process accessing fields data
    When I compute LCOM4
    Then the component count is 1

  Scenario: Two methods sharing a field are cohesive
    Given a method called read accessing fields buffer
    And a method called write accessing fields buffer
    When I compute LCOM4
    Then the component count is 1

  Scenario: Two methods with a direct call are cohesive
    Given a method called validate accessing no fields
    And a method called process accessing no fields calling validate
    When I compute LCOM4
    Then the component count is 1

  Scenario: Two disjoint methods indicate low cohesion
    Given a method called parse accessing fields input
    And a method called render accessing fields output
    When I compute LCOM4
    Then the component count is 2

  Scenario: Transitive field sharing connects a chain
    Given a method called a accessing fields x
    And a method called b accessing fields x, y
    And a method called c accessing fields y
    When I compute LCOM4
    Then the component count is 1

  Scenario: Empty type has zero components
    When I compute LCOM4
    Then the component count is 0

  Scenario: Methods with no fields and no calls are isolated
    Given a method called alpha accessing no fields
    And a method called beta accessing no fields
    And a method called gamma accessing no fields
    When I compute LCOM4
    Then the component count is 3
