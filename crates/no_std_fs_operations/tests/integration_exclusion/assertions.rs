//! Fixture assertions and snapshot coverage for exclusion integration tests.

use super::*;

/// Runs `cargo dylint` against the fixture and counts diagnostics.
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
