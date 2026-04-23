Feature: Shared rstest span recovery

  Scenario: A direct user-editable span is kept
    Given a direct user-editable span at line 10
    When I recover the user-editable span
    Then the recovery result keeps the direct span at line 10

  Scenario: A nested macro chain recovers the invocation site
    Given a macro frame at line 2
    And a macro frame at line 3
    And a user-editable frame at line 12
    When I recover the user-editable span
    Then the recovery result uses a recovered span at line 12

  Scenario: Macro-only glue is skipped
    Given a macro frame at line 4
    And a macro frame at line 5
    When I recover the user-editable span
    Then the recovery result is macro-only

  Scenario: The first non-expansion frame wins even when later frames also qualify
    Given a macro frame at line 6
    And a user-editable frame at line 20
    And a user-editable frame at line 30
    When I recover the user-editable span
    Then the recovery result uses a recovered span at line 20
