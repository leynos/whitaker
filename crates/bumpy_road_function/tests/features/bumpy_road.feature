Feature: Detect bumpy road intervals

  Scenario: Two separated bumps are detected
    Given a smoothed signal with two bumps
    And the threshold is 3.0
    And the minimum bump length is 2
    When I detect bumps
    Then 2 bumps are reported

  Scenario: A single bump does not trigger multiple intervals
    Given a smoothed signal with one bump
    And the threshold is 3.0
    And the minimum bump length is 2
    When I detect bumps
    Then 1 bumps are reported

  Scenario: Short spikes are ignored
    Given a smoothed signal with a short spike
    And the threshold is 3.0
    And the minimum bump length is 2
    When I detect bumps
    Then 0 bumps are reported

  Scenario: Even smoothing windows fall back to defaults
    Given default settings
    When the smoothing window is set to 2
    And I normalise the settings
    Then the window becomes 3

  Scenario: Negative thresholds fall back to defaults
    Given default settings
    When the threshold is set to -1.0
    And I normalise the settings
    Then the threshold becomes 3.0

