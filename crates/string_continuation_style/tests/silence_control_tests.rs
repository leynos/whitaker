//! Enforces that every silence fixture names a committed diagnostic control.
//!
//! A `pass_*` fixture on its own proves nothing about the gate it exercises: a
//! lint that never fires passes all of them. What makes a silence fixture
//! meaningful is a paired control — a committed `fail_*` fixture that reports
//! the same construct with the silencing factor removed. If the control ever
//! stops reporting, the gate is not live and the silence fixture is vacuous.
//!
//! That pairing used to be prose. `pass_shadowed_println.rs` said its control
//! was "recorded in the `ExecPlan`", which is a document no gate reads, so the
//! claim could rot silently. This test makes the pairing a checked property:
//! each `pass_*` fixture must declare `Control: <fixture>.rs`, the named file
//! must exist beside it, and it must carry a `.stderr` golden proving it still
//! emits a diagnostic.

use camino::{Utf8Path, Utf8PathBuf};

/// Collects fixture file names in `directory` with the given prefix and suffix.
fn fixture_names(directory: &Utf8Path, prefix: &str, suffix: &str) -> Vec<String> {
    let Ok(entries) = directory.read_dir_utf8() else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_owned())
        .filter(|name: &String| name.starts_with(prefix) && name.ends_with(suffix))
        .collect()
}

/// Returns the control fixture named by a `pass_*` source, if it declares one.
fn declared_control(source: &str) -> Option<&str> {
    source
        .lines()
        .find_map(|line| line.trim().strip_prefix("//! Control: "))
        .map(str::trim)
}

#[test]
fn every_silence_fixture_names_a_committed_control() {
    let directory = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ui")
        .canonicalize_utf8()
        .expect("the ui fixture directory should exist");
    let mut silence_fixtures = fixture_names(&directory, "pass_", ".rs");
    silence_fixtures.sort();
    assert!(
        !silence_fixtures.is_empty(),
        "no silence fixtures found in {directory}"
    );

    let mut failures = Vec::new();
    for fixture in &silence_fixtures {
        let path = directory.join(fixture);
        let Ok(source) = std::fs::read_to_string(&path) else {
            failures.push(format!("{fixture}: unreadable"));
            continue;
        };
        let Some(control) = declared_control(&source) else {
            failures.push(format!("{fixture}: no `//! Control: <fixture>.rs` line"));
            continue;
        };
        if !directory.join(control).exists() {
            failures.push(format!("{fixture}: control `{control}` does not exist"));
            continue;
        }
        let golden = directory.join(control.replace(".rs", ".stderr"));
        if !golden.exists() {
            failures.push(format!(
                "{fixture}: control `{control}` has no `.stderr`, so it proves no diagnostic"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "silence fixtures must each name a committed control that reports:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn the_pairing_check_rejects_a_fixture_with_no_control() {
    assert_eq!(
        declared_control("//! Silence is deliberate.\nfn main() {}"),
        None
    );
    assert_eq!(
        declared_control("//! Control: fail_plain_join.rs\n"),
        Some("fail_plain_join.rs")
    );
}
