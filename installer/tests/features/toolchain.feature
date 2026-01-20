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

  # Auto-install failure scenarios - these exercise the auto-install code path
  # by using a non-existent toolchain to trigger installation attempts

  Scenario: Auto-install failure reports error with toolchain name
    Given the installer is invoked with a non-existent toolchain
    When the installer CLI is run
    Then the CLI exits with an error
    And the error mentions toolchain installation failure
    And the error includes the toolchain name

  Scenario: Auto-install failure in quiet mode produces minimal output
    Given the installer is invoked with a non-existent toolchain in quiet mode
    When the installer CLI is run
    Then the CLI exits with an error
    And the error mentions toolchain installation failure
    And the error output is minimal
