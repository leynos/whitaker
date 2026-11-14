Feature: Opt in to macro-aware complexity
  Scenario: Stable installation omits the experimental lint
    Given an installation request without experimental lints
    When the installer resolves lint crates
    Then macro_aware_cognitive_complexity is absent

  Scenario: Experimental installation includes the lint
    Given an installation request enabling experimental lints
    When the installer resolves lint crates
    Then macro_aware_cognitive_complexity is selected exactly once