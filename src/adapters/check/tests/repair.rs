//! What a lazy repair preserves, and what it refuses to guess.

use whitaker_installer::artefact::suite_ref::SuiteRef;

use super::{
    super::repair::{recorded_suite_pin, repair_arguments},
    fixtures::{record, temp_dirs},
};
use crate::{diagnostics::DiagnosticSettings, ports::CheckRepairError};

#[test]
fn a_repair_replays_the_suite_pin_the_installation_recorded() {
    let (_root, dirs) = temp_dirs();
    record(Some("v0.2.7"))
        .write(&dirs.layout().metadata_path())
        .expect("the record can be written");

    let pin = recorded_suite_pin(&dirs).expect("a readable record is resolved");

    assert_eq!(
        pin.as_ref().map(SuiteRef::as_str),
        Some("v0.2.7"),
        "a repair must rebuild the suite the installation recorded"
    );
}

#[test]
fn an_unpinned_installation_has_no_pin_to_replay() {
    let (_root, dirs) = temp_dirs();
    record(None)
        .write(&dirs.layout().metadata_path())
        .expect("the record can be written");

    let pin = recorded_suite_pin(&dirs).expect("a readable record is resolved");

    assert_eq!(
        pin, None,
        "an install that recorded no pin leaves the repair free to use the tip"
    );
}

#[test]
fn a_machine_with_no_record_has_no_pin_to_replay() {
    let (_root, dirs) = temp_dirs();

    let pin = recorded_suite_pin(&dirs).expect("an absent record is not a failure");

    assert_eq!(
        pin, None,
        "the repair that writes the first record has no provenance to preserve"
    );
}

#[test]
fn a_record_the_installer_cannot_use_has_no_pin_to_replay() {
    let (_root, dirs) = temp_dirs();
    let path = dirs.layout().metadata_path();
    std::fs::create_dir_all(path.parent().expect("the record path has a parent"))
        .expect("the bootstrap directory can be created");
    std::fs::write(&path, b"{ this is not a record }").expect("the damaged record can be written");

    let pin = recorded_suite_pin(&dirs).expect("a record the installer will replace is not fatal");

    assert_eq!(
        pin, None,
        "an unusable record names no pin, and the repair replaces it anyway"
    );
}

#[test]
fn a_recorded_pin_that_is_not_a_reference_is_refused() {
    let (_root, dirs) = temp_dirs();
    record(Some("--upload-pack=evil"))
        .write(&dirs.layout().metadata_path())
        .expect("the record can be written");

    let error = recorded_suite_pin(&dirs).expect_err("a damaged pin must not be replayed");

    assert!(
        matches!(error, CheckRepairError::Install { .. }),
        "a pin that no longer validates needs an explicit install, got {error:?}"
    );
    assert!(
        error.to_string().contains("--upload-pack=evil"),
        "the refusal must name the pin it could not use: {error}"
    );
}

#[test]
fn a_record_that_cannot_be_read_is_a_host_fault() {
    let (_root, dirs) = temp_dirs();
    let path = dirs.layout().metadata_path();
    std::fs::create_dir_all(path.parent().expect("the record path has a parent"))
        .expect("the bootstrap directory can be created");
    // A directory where the record belongs makes the read fail with something
    // other than "not found", which is the case under test.
    std::fs::create_dir(&path).expect("a directory can stand in for the record");

    let error = recorded_suite_pin(&dirs).expect_err("an unreadable record is not a missing pin");

    assert!(
        matches!(error, CheckRepairError::Host { .. }),
        "installing again cannot mend an unreadable data directory, got {error:?}"
    );
}

#[test]
fn a_repair_keeps_the_clone_it_has_and_replays_the_pin() {
    let pin: SuiteRef = "v0.2.7".try_into().expect("a valid reference");

    let args = repair_arguments(
        Some(&pin),
        DiagnosticSettings {
            quiet: false,
            verbosity: 2,
        },
    );

    assert_eq!(
        args.suite_version.as_ref().map(SuiteRef::as_str),
        Some("v0.2.7"),
        "the repair must ask for the same suite the installation recorded"
    );
    assert!(
        args.skip.no_update,
        "a repair must fill a gap, not upgrade the managed clone"
    );
    assert_eq!(
        args.verbosity, 2,
        "the root command's -v must reach the installer"
    );
    assert!(
        !args.quiet,
        "a repair must not silence an invocation that did not ask for it"
    );
}

#[test]
fn an_unpinned_repair_provisions_everything_it_lacks() {
    let args = repair_arguments(None, DiagnosticSettings::default());

    assert_eq!(
        args.suite_version, None,
        "there is no pin to hold an unpinned repair to"
    );
    assert!(
        args.skip.no_update,
        "a repair must fill a gap, not upgrade the managed clone"
    );
    assert!(
        !args.skip.skip_deps,
        "a repair must be able to install the Dylint tools it is missing"
    );
}

#[test]
fn a_quiet_repair_stays_quiet() {
    let args = repair_arguments(
        None,
        DiagnosticSettings {
            quiet: true,
            verbosity: 0,
        },
    );

    assert!(args.quiet, "the root command's -q must reach the installer");
}
