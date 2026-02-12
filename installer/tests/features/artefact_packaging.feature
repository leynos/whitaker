Feature: Artefact packaging for rolling release
  Prebuilt lint library archives are packaged according to
  ADR-001 for distribution via the rolling GitHub Release.

  Scenario: Package a single library file into a tar.zst archive
    Given a library file "libwhitaker_suite.so"
    And a git SHA "abc1234"
    And a toolchain channel "nightly-2025-09-18"
    And a target triple "x86_64-unknown-linux-gnu"
    When the artefact is packaged
    Then the archive exists with the expected ADR-001 filename
    And the archive contains the library file
    And the archive contains a manifest.json

  Scenario: Manifest JSON contains all required fields
    Given a packaged artefact
    When the manifest is extracted
    Then the manifest contains field "git_sha"
    And the manifest contains field "schema_version"
    And the manifest contains field "toolchain"
    And the manifest contains field "target"
    And the manifest contains field "generated_at"
    And the manifest contains field "files"
    And the manifest contains field "sha256"

  Scenario: Archive SHA-256 is a valid digest
    Given a packaged artefact
    When the archive SHA-256 is computed
    Then it is a valid 64-character hex string

  Scenario: Packaging rejects an empty file list
    Given no library files
    When packaging is attempted
    Then a packaging error is returned

  Scenario: Archive filename matches ArtefactName convention
    Given a packaged artefact with known components
    When the archive filename is inspected
    Then it matches the ArtefactName string representation

  Scenario: Archive contains multiple library files
    Given library files "libfoo.so" and "libbar.so" and "libbaz.so"
    And a git SHA "abc1234"
    And a toolchain channel "nightly-2025-09-18"
    And a target triple "x86_64-unknown-linux-gnu"
    When the artefact is packaged
    Then the archive contains 3 library files
    And the archive contains a manifest.json

  Scenario: Manifest files field lists all library basenames
    Given library files "libfoo.so" and "libbar.so"
    And a git SHA "abc1234"
    And a toolchain channel "nightly-2025-09-18"
    And a target triple "x86_64-unknown-linux-gnu"
    When the artefact is packaged
    And the manifest is extracted
    Then the manifest files field contains "libfoo.so"
    And the manifest files field contains "libbar.so"
