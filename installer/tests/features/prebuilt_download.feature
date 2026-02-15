Feature: Prebuilt artefact download and verification

  The installer attempts to download prebuilt lint libraries before
  building locally, per ADR-001. Any failure causes a graceful fallback
  to local compilation without aborting.

  Scenario: Successful prebuilt download and verification
    Given a valid manifest for target "x86_64-unknown-linux-gnu"
    And a matching archive with correct checksum
    When prebuilt download is attempted
    Then the prebuilt result is success
    And the staging path contains the expected toolchain directory

  Scenario: Checksum mismatch triggers fallback
    Given a valid manifest for target "x86_64-unknown-linux-gnu"
    And an archive with mismatched checksum
    When prebuilt download is attempted
    Then the prebuilt result is fallback
    And the fallback reason mentions "checksum"

  Scenario: Network failure triggers fallback
    Given a manifest download that fails with a network error
    When prebuilt download is attempted
    Then the prebuilt result is fallback
    And the fallback reason mentions "download"

  Scenario: Missing artefact triggers fallback
    Given a manifest download that returns not found
    When prebuilt download is attempted
    Then the prebuilt result is fallback
    And the fallback reason mentions "not found"

  Scenario: Toolchain mismatch triggers fallback
    Given a valid manifest with toolchain "nightly-2025-01-01"
    And the expected toolchain is "nightly-2025-09-18"
    When prebuilt download is attempted
    Then the prebuilt result is fallback
    And the fallback reason mentions "toolchain mismatch"

  Scenario: Build-only flag skips prebuilt
    Given the build-only flag is set
    When the install configuration is checked
    Then no prebuilt download is attempted
