//! UI harness for the `missing_docs_in_test_functions` lint.
//!
//! Three batches, because the gate under test differs in each. `ui/` compiles
//! with `--test` and turns the lint into a hard error, so a fixture that should
//! be quiet proves the policy skipped it rather than merely warning quietly.
//! `ui-no-harness/` compiles undocumented sources without `--test`, so the
//! absence of diagnostics proves the harness gate. The example target is
//! separate because it needs the `rstest` and `rstest-bdd` dev-dependencies,
//! which `Test::example` supplies and `Test::src_base` cannot.

use dylint_testing::ui::Test;
use serial_test::serial;
use whitaker_lint_core::testing::ui::HarnessError;

/// Lint name turned into a hard error, so silence in a fixture is meaningful.
const DENY: &str = "-Dmissing_docs_in_test_functions";

/// The edition a fixture is compiled with.
///
/// Compiletest passes no edition by default, so a fixture that uses `async fn`
/// would fail to parse rather than exercise the lint. Example targets are the
/// exception: `Test::example` already passes an edition, and repeating it is a
/// hard error rather than an override.
const EDITION: &str = "--edition=2024";

/// Runs the `--test` fixture directory.
#[test]
#[serial]
fn ui() {
    run_directory("ui", &["--test", DENY, EDITION]).expect("the `--test` UI fixtures should match");
}

/// Runs the same undocumented sources without a test harness.
#[test]
#[serial]
fn ui_no_harness() {
    run_directory("ui-no-harness", &[DENY, EDITION])
        .expect("the non-harness UI fixtures should produce no diagnostics");
}

/// Runs the example that needs the macro dev-dependencies.
#[test]
#[serial]
fn ui_proc_macro_example() {
    whitaker_lint_core::testing::ui::run_with_runner(
        env!("CARGO_PKG_NAME"),
        "examples",
        |crate_name, _directory| {
            whitaker_common::test_support::run_test_runner("proc_macro_functions", || {
                let mut test = Test::example(crate_name, "proc_macro_functions");
                test.rustc_flags(["--test", DENY]);
                test.run();
            })
        },
    )
    .expect("the procedural-macro example should match its expected output");
}

/// Runs one fixture directory through the shared UI runner.
///
/// `Test::src_base` walks the directory itself, so a fixture that declares an
/// out-of-line module finds it under `auxiliary/`, which compiletest excludes
/// from test collection — the same layout the other lint crates use.
fn run_directory(directory: &str, flags: &[&str]) -> Result<(), HarnessError> {
    whitaker_lint_core::testing::ui::run_with_runner(
        env!("CARGO_PKG_NAME"),
        directory,
        |crate_name, fixture_directory| {
            let mut test = Test::src_base(crate_name, fixture_directory);
            test.rustc_flags(flags.iter().copied());
            test.run();
            Ok(())
        },
    )
}
