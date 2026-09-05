//! Subprocess coverage for installer diagnostics configuration.

use crate::diagnostics;
use std::process::Command;

const PROBE_ENV: &str = "WHITAKER_DIAGNOSTICS_PROBE";
const DEBUG_MARKER: &str = "diagnostics-debug-probe";
const WARN_MARKER: &str = "diagnostics-warn-probe";
const TARGET_MARKER: &str = "diagnostics_probe_target";

#[test]
#[ignore = "invoked by the subprocess-isolated diagnostics test"]
fn diagnostics_probe_child() {
    if std::env::var_os(PROBE_ENV).is_none() {
        return;
    }
    diagnostics::initialize();
    tracing::debug!(target: TARGET_MARKER, "{DEBUG_MARKER}");
    tracing::warn!(target: TARGET_MARKER, "{WARN_MARKER}");
}

#[test]
fn diagnostics_filter_and_formatter_are_configured_in_a_subprocess() {
    let debug_output = run_diagnostics_probe("debug");
    assert!(
        debug_output.contains(DEBUG_MARKER),
        "output: {debug_output}"
    );
    assert!(debug_output.contains(WARN_MARKER), "output: {debug_output}");
    assert!(
        !debug_output.contains(TARGET_MARKER),
        "formatter must suppress tracing targets: {debug_output}"
    );

    let fallback_output = run_diagnostics_probe("[");
    assert!(
        !fallback_output.contains(DEBUG_MARKER),
        "invalid RUST_LOG must fall back to warn filtering: {fallback_output}"
    );
    assert!(
        fallback_output.contains(WARN_MARKER),
        "warn fallback must retain warning diagnostics: {fallback_output}"
    );
}

fn run_diagnostics_probe(rust_log: &str) -> String {
    let executable = std::env::current_exe().expect("locate installer test executable");
    let output = Command::new(executable)
        .args([
            "--exact",
            "tests::diagnostics_tests::diagnostics_probe_child",
            "--ignored",
            "--nocapture",
        ])
        .env(PROBE_ENV, "1")
        .env("RUST_LOG", rust_log)
        .output()
        .expect("run diagnostics probe subprocess");
    assert!(
        output.status.success(),
        "diagnostics probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("{stdout}{stderr}")
}
