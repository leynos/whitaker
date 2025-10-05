Feature: Shared configuration loading

  Background:
    Given no configuration state has been prepared

  Scenario: load defaults when configuration is absent
    Given no workspace configuration overrides are provided
    When the shared configuration is loaded
    Then the module max line limit is 400

  Scenario: override module max line limit
    Given the workspace config sets the module max line limit to 120
    When the shared configuration is loaded
    Then the module max line limit is 120

  Scenario: report configuration errors
    Given the workspace config sets the module max line limit to an invalid value
    When the shared configuration is loaded
    Then a configuration error is reported
