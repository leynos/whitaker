Feature: Locale resolution
  Locale selection honours explicit overrides, the DYLINT_LOCALE environment
  variable, and workspace configuration before falling back to the bundled
  locale.

  Background:
    Given no explicit locale override is provided
    And DYLINT_LOCALE is not set
    And no configuration locale is provided

  Scenario: Use the fallback locale when no overrides exist
    When the locale is resolved
    Then the locale source is fallback
    And the resolved locale is en-GB
    And the fallback locale is used
    And no locale rejections are recorded

  Scenario: Use the environment locale when available
    Given DYLINT_LOCALE is gd
    And the configuration locale is cy
    When the locale is resolved
    Then the locale source is environment
    And the resolved locale is gd
    And the fallback locale is not used
    And no locale rejections are recorded

  Scenario: Prefer configuration after rejecting the environment
    Given DYLINT_LOCALE is zz
    And the configuration locale is cy
    When the locale is resolved
    Then the locale source is configuration
    And the resolved locale is cy
    And the fallback locale is not used
    And the locale rejections include environment zz

  Scenario: Prefer the explicit override over other sources
    Given the explicit locale override is gd
    And DYLINT_LOCALE is cy
    And the configuration locale is en-GB
    When the locale is resolved
    Then the locale source is explicit
    And the resolved locale is gd
    And the fallback locale is not used
    And no locale rejections are recorded

  Scenario: Ignore explicit whitespace and fall back to configuration
    Given the explicit locale override is "  "
    And the configuration locale is gd
    When the locale is resolved
    Then the locale source is configuration
    And the resolved locale is gd
    And the fallback locale is not used
    And no locale rejections are recorded
