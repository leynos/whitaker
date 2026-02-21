Feature: Installer metrics recording

  The installer records local aggregate metrics for successful install
  outcomes, tracking download-versus-build rates and total installation time.

  Scenario: Record a successful download install
    Given an empty install metrics store
    When a download install of 1200 milliseconds is recorded
    Then total installs is 1
    And download installs is 1
    And build installs is 0
    And download rate is 1.0
    And build rate is 0.0
    And total installation time is 1200 milliseconds

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

  Scenario: Recover from a corrupt metrics file
    Given a corrupt install metrics store
    When a build install of 500 milliseconds is recorded
    Then metrics recovery from corrupt file is true
    And total installs is 1
    And build installs is 1

  Scenario: Report write failures as errors
    Given a blocked install metrics path
    When a download install of 300 milliseconds is recorded
    Then metrics recording fails

  Scenario: Zero-state rates are zero
    Given an in-memory zero metrics aggregate
    When download and build rates are calculated
    Then download rate is 0.0
    And build rate is 0.0
