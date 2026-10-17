Feature: The root whitaker command-line interface

  Scenario: The root command lists its subcommands
    Given the whitaker binary is available
    When I run whitaker with "--help"
    Then the command succeeds
    And the output lists the subcommand "install"
    And the output lists the subcommand "ls"

  Scenario: Requesting help exits successfully
    Given the whitaker binary is available
    When I run whitaker with "--help"
    Then the exit code is 0

  Scenario: Listing staged lints as JSON
    Given a staging directory containing a staged suite library
    When I run whitaker with "ls --json"
    Then the command succeeds
    And the output is valid JSON
    And stdout contains no diagnostic text

  Scenario: Listing staged lints as text
    Given a staging directory containing a staged suite library
    When I run whitaker with "ls"
    Then the command succeeds
    And the output names the staged suite

  Scenario: A dry-run install reports configuration without writing stdout
    Given a Whitaker workspace checkout
    When I run whitaker with "install --dry-run"
    Then the command succeeds
    And no lint library is staged
    And stdout is empty
    And stderr contains install diagnostics
    And no wrapper-generation message is reported

  Scenario: Conflicting lint selection flags are rejected
    Given the whitaker binary is available
    When I run whitaker with "install --lint module_max_lines --individual-lints"
    Then the command fails
    And the exit code is 2
    And the error names both conflicting options

  Scenario: Legacy lint invocations are forwarded to cargo dylint
    Given the whitaker binary is available
    And a fake cargo dylint command returning exit code 23
    When I run whitaker with "--all -- -p whitaker-common --all-targets"
    Then the invocation reaches cargo dylint
    And the exit code is 23
