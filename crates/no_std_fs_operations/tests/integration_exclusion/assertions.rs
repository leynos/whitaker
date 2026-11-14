//! Fixture assertions and snapshot coverage for exclusion integration tests.

use cargo_metadata::diagnostic::DiagnosticLevel;

use super::*;

/// Assert that a run reported a configuration warning naming every needle.
///
/// Configuration warnings are plain rustc warnings rather than lint
/// diagnostics, so they carry no diagnostic code and are matched by their
/// rendered message instead.
///
/// # Errors
///
/// Return an error when the output cannot be parsed, no configuration warning
/// was emitted, or a needle is missing from every warning.
pub(super) fn assert_config_warning(
    result: &CargoDylintResult,
    needles: &[&str],
) -> anyhow::Result<()> {
    let warnings = warning_messages(&result.stdout)?;
    let config_warnings: Vec<&String> = warnings
        .iter()
        .filter(|message| message.contains("[no_std_fs_operations]"))
        .collect();

    anyhow::ensure!(
        !config_warnings.is_empty(),
        "expected a [no_std_fs_operations] configuration warning\nstderr:\n{}",
        result.stderr
    );

    for needle in needles {
        anyhow::ensure!(
            config_warnings
                .iter()
                .any(|message| message.contains(needle)),
            concat!(
                "expected a configuration warning containing ",
                "{needle:?}\nwarnings:\n{config_warnings:#?}\nstderr:\n{stderr}",
            ),
            needle = needle,
            config_warnings = config_warnings,
            stderr = result.stderr
        );
    }

    Ok(())
}

/// Collect the rendered messages of every compiler warning in cargo output.
///
/// # Errors
///
/// Return an error when the JSON message stream cannot be parsed.
fn warning_messages(output: &[u8]) -> anyhow::Result<Vec<String>> {
    let messages = Message::parse_stream(Cursor::new(output))
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "failed to parse cargo JSON messages:\n{}",
                String::from_utf8_lossy(output)
            )
        })?;

    Ok(messages
        .into_iter()
        .filter_map(|message| match message {
            Message::CompilerMessage(compiler_message)
                if matches!(compiler_message.message.level, DiagnosticLevel::Warning) =>
            {
                Some(compiler_message.message.message)
            }
            _ => None,
        })
        .collect())
}

/// Run `cargo dylint` against the fixture and count its diagnostics.
///
/// # Errors
///
/// Return an error if the command fails to run or its output cannot be parsed.
fn evaluate_fixture(
    fixture_dir: &Path,
    lint_library_path: &Path,
    crate_name: &str,
) -> anyhow::Result<(bool, usize)> {
    let result = run_cargo_dylint(fixture_dir, lint_library_path)?;
    let count = diagnostic_count(&result.stdout).with_context(|| {
        format!(
            "crate `{crate_name}` produced malformed cargo output\nstderr:\n{}",
            result.stderr
        )
    })?;
    Ok((result.is_success, count))
}

/// Assert that a fixture's exit status and diagnostic count match expectations.
///
/// # Errors
///
/// Return an error when the command fails, its output is malformed, or either
/// expectation is not met.
pub(super) fn assert_fixture_behaviour(
    fixture_dir: &Path,
    lint_library_path: &Path,
    crate_name: &str,
    expectation: Expectation,
) -> anyhow::Result<()> {
    let (is_success, count) = evaluate_fixture(fixture_dir, lint_library_path, crate_name)?;

    anyhow::ensure!(
        is_success == expectation.should_succeed,
        "crate `{crate_name}` should return success={}",
        expectation.should_succeed
    );

    if expectation.should_emit_diagnostics {
        anyhow::ensure!(
            count > 0,
            "crate `{crate_name}` should emit `no_std_fs_operations` diagnostics"
        );
    } else {
        anyhow::ensure!(
            count == 0,
            "crate `{crate_name}` should emit zero `no_std_fs_operations` diagnostics"
        );
    }

    Ok(())
}

/// Snapshot test: verifies the structured JSON diagnostic output emitted by
/// `cargo dylint` for a non-excluded crate.
///
/// Non-deterministic fields (absolute fixture paths) are redacted to
/// `[FIXTURE_ROOT]` before the snapshot is taken.
#[test]
#[ignore = "requires cargo-dylint and built lint library"]
#[serial]
fn non_excluded_crate_diagnostics_match_snapshot() -> anyhow::Result<()> {
    let lint_library_path = lint_library_path().context("failed to build lint library")?;
    let fixture = create_fixture_project(
        "non_excluded_crate_snap",
        FixtureKind::CrateExclusion,
        false,
    )
    .context("failed to create fixture project")?;

    let result = run_cargo_dylint(fixture.root(), &lint_library_path)
        .context("failed to run cargo dylint")?;

    let messages = Message::parse_stream(Cursor::new(&result.stdout))
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "non_excluded_crate_diagnostics_match_snapshot produced malformed cargo \
                 output\nstderr:\n{}",
                result.stderr
            )
        })?;

    let diagnostics: Vec<serde_json::Value> = messages
        .into_iter()
        .filter_map(|message| match message {
            Message::CompilerMessage(compiler_message)
                if compiler_message
                    .message
                    .code
                    .as_ref()
                    .is_some_and(|code| code.code == LINT_CRATE_NAME) =>
            {
                Some(
                    serde_json::to_value(compiler_message.message)
                        .context("failed to serialize diagnostic for snapshot"),
                )
            }
            _ => None,
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let prefix = fixture
        .root()
        .to_str()
        .context("fixture root should be valid UTF-8")?;

    let redacted: Vec<serde_json::Value> = diagnostics
        .into_iter()
        .map(|value| redact_path_prefix(value, prefix))
        .collect();

    assert_json_snapshot!("non_excluded_crate_diagnostics", redacted);
    Ok(())
}
