Feature: Shared rstest fingerprint models

  Scenario: Equivalent helper-call arguments share a fingerprint
    Given helper-call arguments for fixture db and literal 42
    And matching helper-call arguments for fixture db and literal 42
    When I compare the argument fingerprints
    Then the argument fingerprints match

  Scenario: Renamed setup paragraphs share a fingerprint
    Given a setup paragraph using locals user and cache
    And a matching setup paragraph using locals account and store
    When I compare the paragraph fingerprints
    Then the paragraph fingerprints match

  Scenario: Unsupported arguments remain explicit
    Given helper-call arguments containing an unsupported argument
    When I inspect the argument fingerprint
    Then the unsupported argument is still present

  Scenario: Structurally different setup paragraphs diverge
    Given a setup paragraph with a one-argument constructor
    And a matching setup paragraph with a two-argument constructor
    When I compare the paragraph fingerprints
    Then the paragraph fingerprints differ

  Scenario: Local slots follow first appearance order
    Given a setup paragraph using locals zeta and alpha
    When I inspect the paragraph fingerprint
    Then zeta has slot 0 and alpha has slot 1
