Feature: rstest helper call-site collection

  Scenario: generated rstest cases are deduplicated by source call site
    Given two generated rstest cases share the same source helper call
    When the collector stores the call-site evidence
    Then one deduplicated record is retained

  Scenario: distinct helper calls remain distinct
    Given two helper calls use different source spans
    When the collector stores the call-site evidence
    Then both source records are retained
