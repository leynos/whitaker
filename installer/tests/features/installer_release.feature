Feature: Installer release archive packaging
  The whitaker-installer binary is packaged into binstall-compatible
  archives for distribution via GitHub Releases. Unix targets use
  .tgz archives; Windows uses .zip.

  Scenario: Archive filename uses tgz for Linux target
    Given version "0.2.1" and target "x86_64-unknown-linux-gnu"
    When the archive filename is computed
    Then the archive filename is "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1.tgz"

  Scenario: Archive filename uses zip for Windows target
    Given version "0.2.1" and target "x86_64-pc-windows-msvc"
    When the archive filename is computed
    Then the archive filename is "whitaker-installer-x86_64-pc-windows-msvc-v0.2.1.zip"

  Scenario: Archive contains correct directory structure for Unix
    Given version "0.2.1" and target "x86_64-unknown-linux-gnu"
    And a fake installer binary exists
    When the installer is packaged
    Then the archive contains "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.1/whitaker-installer"

  Scenario: Windows archive contains exe binary
    Given version "0.2.1" and target "x86_64-pc-windows-msvc"
    And a fake installer binary exists
    When the installer is packaged
    Then the archive contains "whitaker-installer-x86_64-pc-windows-msvc-v0.2.1/whitaker-installer.exe"

  Scenario: Archive filename matches binstall pkg-url template
    Given version "0.2.1" and target "aarch64-apple-darwin"
    When the archive filename is computed
    Then the binstall pkg-url ends with the archive filename

  Scenario: Packaging rejects missing binary
    Given version "0.2.1" and target "x86_64-unknown-linux-gnu"
    And the binary path does not exist
    When packaging is attempted
    Then a packaging error is returned
