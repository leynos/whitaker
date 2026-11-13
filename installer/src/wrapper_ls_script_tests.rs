//! Tests for the generated lister scripts.
//!
//! Both templates filter `cargo dylint list` to the Whitaker suite. They must
//! pass only the subcommand the list command supports, and the PowerShell
//! filter must match the suite name as a whole word with real regex classes.

use super::super::{SUITE_CRATE, unix_whitaker_ls_script, windows_ls_script};

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
        r#"$_ -match ("^\s*" + [regex]::Escape($suite) + "(\s|$)")"#
    );
    assert!(script.contains(&format!("$suite = \"{SUITE_CRATE}\"")));
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
