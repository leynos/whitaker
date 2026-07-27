Feature: Module-path scoped suppression for std::fs usage

  Scenario: A usage inside an excluded module is suppressed
    Given the module path "my_app::legacy_io" is excluded
    When a std::fs usage is found in item "my_app::legacy_io::reader"
    Then the usage is suppressed

  Scenario: A usage in the excluded module itself is suppressed
    Given the module path "my_app::legacy_io" is excluded
    When a std::fs usage is found in item "my_app::legacy_io"
    Then the usage is suppressed

  Scenario: A usage in a sibling module sharing a name prefix is reported
    Given the module path "my_app::legacy_io" is excluded
    When a std::fs usage is found in item "my_app::legacy_io_utils::reader"
    Then the usage is reported

  Scenario: A usage in an unrelated module is reported
    Given the module path "my_app::legacy_io" is excluded
    When a std::fs usage is found in item "my_app::network::client"
    Then the usage is reported

  Scenario: A crate-root exclusion suppresses every module
    Given the module path "my_app" is excluded
    When a std::fs usage is found in item "my_app::network::client"
    Then the usage is suppressed

  Scenario: With no exclusions configured every usage is reported
    Given no module paths are excluded
    When a std::fs usage is found in item "my_app::legacy_io::reader"
    Then the usage is reported
