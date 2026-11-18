Feature: The whitaker check command

  Scenario: A ready installation is linted through the Whitaker suite
    Given a ready Whitaker installation
    When I run whitaker check with "-- -p whitaker-common"
    Then the exit code is 23
    And cargo dylint received the Whitaker suite options and the user arguments

  Scenario: An uninstalled machine is refused without running Dylint
    Given no Whitaker installation
    When I run whitaker check with "--no-install"
    Then the exit code is 4
    And stderr reports the reason key "provision.refused"

  Scenario: A user-managed library path without the suite is refused
    Given a Dylint library path the user set with no Whitaker suite
    And a cargo command that records its invocation
    When I run whitaker check
    Then the exit code is 4
    And stderr reports the reason key "provision.user-path"
    And cargo dylint was never started

  Scenario: A user-managed library path overrides the recorded libraries
    Given a ready Whitaker installation
    And a Dylint library path the user set
    When I run whitaker check
    Then the exit code is 23
    And cargo dylint was given the library path the user set