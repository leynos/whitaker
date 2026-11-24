//! Tests for the generated lister scripts.
//!
//! Both templates filter `cargo dylint list` to the Whitaker suite. They must
//! pass only the subcommand the list command supports, and the PowerShell
//! filter must match the suite name as a whole word with real regex classes.

#[cfg(unix)]
use std::path::Path;

use super::super::{SUITE_CRATE, unix_whitaker_ls_script, windows_ls_script};
#[cfg(unix)]
use super::{STUB_LS_INSTALLER, run, scripts, write_executable};
#[cfg(windows)]
use super::{ls_stub_installer_dir, run_powershell, stdout_lines, windows_scripts};

/// The lister succeeds when the dispatcher does, which is the case the issue
/// reported as broken: the wrapper exited non-zero and printed nothing.
#[cfg(unix)]
#[test]
fn whitaker_ls_succeeds_and_prints_the_suite_line() {
    let (temp, _, whitaker_ls) = scripts();
    write_executable(&temp.path().join("whitaker-installer"), STUB_LS_INSTALLER);

    let output = run(&whitaker_ls, &[], None::<&Path>, 0);

    assert_eq!(
        output.status.code(),
        Some(0),
        "the lister succeeds when the dispatcher does: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "whitaker_suite    nightly    /lib\n",
        "the success path prints the suite line and drops the unrelated names"
    );
}

/// The Windows lister succeeds when the dispatcher does, matching the Unix
/// success-path test so neither platform can regress alone.
#[cfg(windows)]
#[test]
fn windows_whitaker_ls_succeeds_and_prints_the_suite_line() {
    let (_temp, _, whitaker_ls) = windows_scripts();
    let stub = ls_stub_installer_dir();

    let output = run_powershell(&whitaker_ls, &[], Some(stub.path()), 0);

    assert_eq!(
        output.status.code(),
        Some(0),
        "the lister succeeds when the dispatcher does: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        stdout_lines(&output),
        ["whitaker_suite    nightly    /lib"],
        "the Windows success path prints the suite line: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Regression: the template once doubled its backslashes inside a raw string,
/// so PowerShell matched a literal `\s` and `whitaker-ls` printed nothing.
#[test]
fn windows_ls_pattern_uses_regex_whitespace_classes() {
    let script = windows_ls_script();
    let pattern_line = script
        .lines()
        .find(|line| line.contains("-match"))
        .expect("the script filters with -match");

    assert_eq!(
        pattern_line.trim(),
        r#"$_ -match ("^\s*" + [regex]::Escape($suite) + "(\s|$)")"#,
        "the filter must use PowerShell regex whitespace classes: {script}"
    );
    assert!(
        script.contains(&format!("$suite = \"{SUITE_CRATE}\"")),
        "the script must bind the suite name before matching it: {script}"
    );
}

/// Regression: the lister once passed `--color never`, which `cargo dylint
/// list` rejects, so every invocation failed. Both templates must pass only
/// the subcommand.
#[test]
fn whitaker_ls_scripts_pass_only_the_list_subcommand() {
    for script in [unix_whitaker_ls_script(), windows_ls_script()] {
        assert!(
            !script.contains("--color"),
            "the lister must not pass a colour flag: {script}"
        );
        assert!(
            script.contains("run -- list"),
            "the lister must pass `run -- list`: {script}"
        );
    }
}
