Feature: Lazy repair in whitaker check

  Scenario: Missing components are repaired once before linting
    Given preflight reports "NotInstalled" and then "Ready"
    And the repair succeeds
    When check runs with default options
    Then the repair was requested once
    And Dylint ran once
    And the outcome is Linted
    And standard output is empty
    And standard error contains the installer marker

  Scenario: A ready installation is not repaired
    Given preflight reports "Ready"
    When check runs with default options
    Then the repair was not requested

  Scenario: --no-install refuses a repairable installation
    Given preflight reports "LibrariesMissing"
    When check runs with "--no-install"
    Then the repair was not requested
    And the exit code is 4 with reason key "provision.refused"

  Scenario: A user-managed library path is never repaired
    Given a user-managed library path
    And preflight reports "DylintVersion"
    When check runs with default options
    Then the repair was not requested
    And the exit code is 4 with reason key "provision.refused"

  Scenario: A repair that does not converge is reported
    Given preflight reports "NotInstalled" and then "Snapshot"
    And the repair succeeds
    When check runs with default options
    Then Dylint was not run
    And the exit code is 4 with reason key "provision.not-converged"

  Scenario: A failed repair is an operational error
    Given preflight reports "NotInstalled"
    And the repair fails
    When check runs with default options
    Then Dylint was not run
    And the exit code is 4 with reason key "provision.repair-failed"