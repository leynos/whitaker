Feature: Whitaker UI test harness
  The shared harness should prepare Dylint UI tests consistently so lint crates
  only need to declare which directory contains their fixtures.

  Scenario: Running UI tests with a relative directory
    Given the harness is prepared for crate demo
    And the UI directory is ui
    When the harness is executed
    Then the runner is invoked with crate demo and directory ui
    And the harness succeeds

  Scenario: Rejecting an empty crate name
    Given the harness has no crate name
    When the harness is executed
    Then the harness reports an empty crate name error

  Scenario: Rejecting an absolute UI directory
    Given the harness is prepared for crate lint
    And the UI directory is /tmp/ui
    When the harness is executed
    Then the harness reports an absolute directory error containing /tmp/ui

  Scenario: Propagating runner failures
    Given the harness is prepared for crate lint
    And the UI directory is ui
    And the runner will fail with message diff mismatch
    When the harness is executed
    Then the runner is invoked with crate lint and directory ui
    And the harness reports a runner failure mentioning diff mismatch

  Scenario: Rejecting a Windows UNC UI directory
    Given the harness is prepared for crate lint
    And the UI directory is \\server\share\ui
    When the harness is executed
    Then the harness reports an absolute directory error containing \\server\share\ui

  Scenario: Rejecting a drive-relative Windows UI directory
    Given the harness is prepared for crate lint
    And the UI directory is C:ui
    When the harness is executed
    Then the harness reports an absolute directory error containing C:ui
