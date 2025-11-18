Feature: Localised diagnostics for std::fs usage

  Scenario: English messaging encourages cap-std
    Given the locale "en-GB" is selected
    And the operation is "std::fs::read_to_string"
    When I localise the std::fs diagnostic
    Then the primary mentions "std::fs::read_to_string"
    And the note references "cap_std::fs::Dir"
    And the help references "camino::Utf8Path"

  Scenario: Welsh messaging reflects the capability note
    Given the locale "cy" is selected
    And the operation is "std::fs::remove_file"
    When I localise the std::fs diagnostic
    Then the primary mentions "std::fs::remove_file"
    And the note references "cyfeiriadur"

  Scenario: Scottish Gaelic messaging mirrors the policy
    Given the locale "gd" is selected
    And the operation is "std::fs::metadata"
    When I localise the std::fs diagnostic
    Then the primary mentions "std::fs::metadata"
    And the note references "neach-gairm"

  Scenario: Unsupported locale falls back to English
    Given the locale "zz" is selected
    And the operation is "std::fs::read_dir"
    When I localise the std::fs diagnostic
    Then the primary mentions "std::fs::read_dir"
    And the help references "cap_std::fs::Dir"

  Scenario: Localisation failure surfaces the missing key
    Given localisation fails
    And the operation is "std::fs::canonicalize"
    When I localise the std::fs diagnostic
    Then localisation fails for "no_std_fs_operations"
