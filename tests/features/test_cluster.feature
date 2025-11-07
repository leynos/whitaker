Feature: Zero-config test cluster
  The RAII fixture should provide a ready connection URI without bespoke
  setup rituals.

  Scenario: Building with defaults
    Given a fresh cluster builder
    When the cluster is built
    Then building succeeds with database whitaker_test

  Scenario: Rejecting invalid database names
    Given a fresh cluster builder
    And the database name is ""
    When the cluster is built
    Then building fails with an invalid database name error

  Scenario: Rejecting reserved ports
    Given a fresh cluster builder
    And the cluster port is 42
    When the cluster is built
    Then building fails with an invalid port error

  Scenario: Recording bootstrap statements
    Given a fresh cluster builder
    And the database name is demo_db
    And a bootstrap statement "CREATE EXTENSION citext" is queued
    When the cluster is built
    Then building succeeds with database demo_db
    And the applied statements include "CREATE EXTENSION citext"

  Scenario: Blocking destructive bootstrap statements
    Given a fresh cluster builder
    And a bootstrap statement "DROP DATABASE prod" is queued
    When the cluster is built
    Then building fails with an unsafe statement error
