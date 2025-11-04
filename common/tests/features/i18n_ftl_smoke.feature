Feature: Fluent resources remain parseable
  The localisation bundles must parse cleanly so the loader never poisons
  its shared cache. These scenarios cover file discovery and representative
  parser outcomes so contributors can diagnose failures quickly.

  Scenario: Collect all localisation bundles
    When I collect all Fluent files
    Then each Fluent path is unique
    And the collection includes locales/en-GB/common.ftl

  Scenario: Parse a valid Fluent resource
    Given the Fluent resource fixture valid
    When I parse the Fluent resource
    Then the parse succeeds

  Scenario: Reject a malformed Fluent resource
    Given the Fluent resource fixture invalid
    When I parse the Fluent resource
    Then the parse fails with 1 errors

  Scenario: Reject duplicate message identifiers
    Given the Fluent resource fixture duplicate
    When I parse the Fluent resource
    Then the parse fails with 1 errors
