Feature: SARIF model construction and merge
  SARIF 2.1.0 document construction, rule definition, and run
  merging for the Whitaker clone detection pipeline.

  Scenario: Build a minimal SARIF log with one run
    Given a run for tool whitaker_clones_cli version 0.2.1
    When the SARIF log is built with that run
    Then the log version is 2.1.0
    And the log has 1 run
    And the run tool name is whitaker_clones_cli

  Scenario: Build a result with a WHK001 rule
    Given a result for rule WHK001
    And the result message is Type-1 clone detected
    And a location at file src/main.rs line 10
    When the result is built
    Then the result rule ID is WHK001
    And the result level is warning
    And the result has 1 location

  Scenario: Attach Whitaker properties to a result
    Given Whitaker properties with profile T2
    And k value 25
    And window value 16
    When properties are converted to JSON
    Then the JSON contains whitaker profile T2
    And the JSON contains whitaker k 25

  Scenario: Merge two runs deduplicates identical results
    Given a run containing 2 unique results
    And another run containing 1 duplicate result
    When the runs are merged
    Then the merged run has 2 results

  Scenario: Serialize and deserialize a SARIF log round-trip
    Given a SARIF log with one run and two results
    When the log is serialized to JSON
    And the JSON is deserialized back
    Then the deserialized log equals the original

  Scenario: Empty log with no runs is valid SARIF
    Given a SARIF log with no runs
    When the log is serialized to JSON
    Then the JSON contains version 2.1.0

  Scenario: All three Whitaker rules are defined
    When all Whitaker rules are retrieved
    Then there are 3 rules
    And rule WHK001 exists
    And rule WHK002 exists
    And rule WHK003 exists

  Scenario: Path helpers produce stable file locations
    Given a target directory at /tmp/project/target
    When the token pass path is requested
    Then the path ends with whitaker/clones.token.sarif
