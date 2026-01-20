Feature: Toolchain auto-detection and installation
  The installer automatically detects the pinned toolchain from rust-toolchain.toml
  and ensures it is installed before proceeding with the build.

  Scenario: Auto-detect toolchain in dry-run mode
    Given the installer is invoked with auto-detect toolchain
    When the installer CLI is run
    Then the CLI exits successfully
    And dry-run output shows the detected toolchain

  Scenario: Auto-detect toolchain in quiet mode
    Given the installer is invoked with auto-detect toolchain in quiet mode
    When the installer CLI is run
    Then the CLI exits successfully
    And no toolchain installation message is shown

  Scenario: Auto-detect toolchain and install suite
    Given the installer is invoked with auto-detect toolchain to a temporary directory
    When the installer CLI is run
    Then installation succeeds or is skipped
    And the suite library is staged
