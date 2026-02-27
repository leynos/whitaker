Feature: Cargo-binstall metadata for installer release artefacts
  The whitaker-installer crate includes cargo-binstall metadata so
  that users can install pre-built binaries via cargo binstall without
  compiling from source.

  Scenario: Binstall metadata section exists in Cargo.toml
    Given the installer Cargo.toml is loaded
    When the binstall metadata section is inspected
    Then the pkg-url template is present
    And the bin-dir template is present
    And the default pkg-fmt is "tgz"

  Scenario: Windows override uses zip format
    Given the installer Cargo.toml is loaded
    When the binstall overrides are inspected
    Then the x86_64-pc-windows-msvc override has pkg-fmt "zip"

  Scenario: URL template expands correctly for Linux
    Given target "x86_64-unknown-linux-gnu" and version "0.2.0"
    When the pkg-url template is expanded
    Then the URL ends with ".tgz"
    And the URL contains the target triple

  Scenario: URL template expands correctly for Windows
    Given target "x86_64-pc-windows-msvc" and version "0.2.0"
    When the pkg-url template is expanded
    Then the URL ends with ".zip"
    And the URL contains the target triple

  Scenario: Binary directory expands correctly for Unix
    Given target "x86_64-unknown-linux-gnu" and version "0.2.0"
    When the bin-dir template is expanded
    Then the path ends with "whitaker-installer"

  Scenario: Binary directory expands correctly for Windows
    Given target "x86_64-pc-windows-msvc" and version "0.2.0"
    When the bin-dir template is expanded
    Then the path ends with "whitaker-installer.exe"

  Scenario: No invalid placeholders in templates
    Given the installer Cargo.toml is loaded
    When the binstall metadata section is inspected
    Then no templates contain the placeholder "{repo}"
    And no templates contain the placeholder "{crate}"
