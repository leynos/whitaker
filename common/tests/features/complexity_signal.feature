Feature: Per-line complexity signal construction
  The Bumpy Road feasibility study models complexity as a per-line signal and
  uses moving-average smoothing to highlight sustained peaks.

  Scenario: Overlapping segments accumulate their contributions
    Given a function spanning lines 10 to 14
    And a segment from line 10 to 12 with value 1.0
    And a segment from line 12 to 14 with value 2.0
    When I build the per-line signal
    Then the built signal equals 1.0, 1.0, 3.0, 2.0, 2.0

  Scenario: Segments outside the function range are rejected
    Given a function spanning lines 11 to 14
    And a segment from line 9 to 10 with value 1.0
    When I build the per-line signal
    Then signal building fails

  Scenario: Smoothing averages neighbouring samples
    Given the raw signal is 0.0, 0.0, 3.0, 0.0, 0.0
    And the smoothing window is 3
    When I smooth the signal
    Then the smoothed signal equals 0.0, 1.0, 1.0, 1.0, 0.0

  Scenario: Smoothing rejects an even window size
    Given the raw signal is 1.0, 2.0
    And the smoothing window is 2
    When I smooth the signal
    Then smoothing fails

  Scenario: Smoothing rejects a zero-sized window
    Given the raw signal is 1.0, 2.0
    And the smoothing window is 0
    When I smooth the signal
    Then smoothing fails

