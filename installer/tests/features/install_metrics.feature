Feature: Installer metrics recording

  The installer records local aggregate metrics for successful install
  outcomes, tracking download-versus-build rates and total installation time.

  Scenario: Record a successful prebuilt-download install
    Given an empty install metrics store
    When a download install of 1200 milliseconds is recorded
    Then total installs is 1
    And download installs is 1
    And build installs is 0
    And download rate is 1.0
    And build rate is 0.0
    And total installation time is 1200 milliseconds
    And summary line contains "download 1/1 (100.0%)"
    And summary line contains "build 0/1 (0.0%)"

  Scenario: Record a successful build-only install
    Given an empty install metrics store
    When a build install of 900 milliseconds is recorded
    Then total installs is 1
    And download installs is 0
    And build installs is 1
    And download rate is 0.0
    And build rate is 1.0
    And total installation time is 900 milliseconds
    And summary line contains "download 0/1 (0.0%)"
    And summary line contains "build 1/1 (100.0%)"

  Scenario: Record download and build installs
    Given an empty install metrics store
    And a download install of 1000 milliseconds is recorded
    When a build install of 2000 milliseconds is recorded
    Then total installs is 2
    And download installs is 1
    And build installs is 1
    And download rate is 0.5
    And build rate is 0.5
    And total installation time is 3000 milliseconds
    And summary line contains "download 1/2 (50.0%)"
    And summary line contains "build 1/2 (50.0%)"

  Scenario: Recover from a corrupt metrics file
    Given a corrupt install metrics store
    When a build install of 500 milliseconds is recorded
    Then metrics recovery from corrupt file is true
    And total installs is 1
    And build installs is 1
    And summary line contains "build 1/1 (100.0%)"

  Scenario: Report write failures as warning text
    Given a blocked install metrics path
    When a download install of 300 milliseconds is recorded
    Then metrics recording fails
    And warning text contains "could not record install metrics"

  Scenario: Zero-state rates are zero
    Given an in-memory zero metrics aggregate
    When download and build rates are calculated
    Then download rate is 0.0
    And build rate is 0.0
