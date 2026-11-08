//! Configuration validation and domain-normalization scenarios.

use std::sync::Arc;

use ortho_config::MapEnv;
use serde_json::json;
use tempfile::TempDir;
use whitaker_cli::{load_with_sources, BumpyRoad, RstestHelper};

use crate::support::{ensure, ensure_eq, non_unicode_value, root, sources, write};

/// Reject malformed values and unknown canonical keys.
#[rstest::rstest]
fn rejects_invalid_values_and_unknown_keys(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(
        &root,
        "whitaker.toml",
        "[module_max_lines]\nmax_lines = 'large'\n",
    )?;
    ensure(
        load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err(),
        "a wrongly typed value should be rejected",
    )?;
    write(&root, "whitaker.toml", "[not_a_setting]\nvalue = 1\n")?;
    ensure(
        load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err(),
        "an unknown canonical key should be rejected",
    )?;
    Ok(())
}

/// Assert that loading under `env` rejects the configuration in `root`.
///
/// # Errors
///
/// Propagates the fixture's temporary-directory error, and reports `message`
/// when the injected environment is accepted instead of rejected.
fn assert_rejected_by_environment(
    root_res: std::io::Result<TempDir>,
    env: MapEnv,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    ensure(
        load_with_sources(&sources(&root, Arc::new(env), json!({}))).is_err(),
        message,
    )
}

/// Reject invalid presentation settings before returning application values.
#[rstest::rstest]
fn rejects_unknown_output_policy(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_rejected_by_environment(
        root_res,
        MapEnv::new().with_var("WHITAKER_OUTPUT_COLOUR", "sometimes"),
        "an unknown colour policy should be rejected",
    )
}

/// Reject misspelled command namespaces instead of silently discarding them.
#[rstest::rstest]
fn rejects_unknown_command_namespace(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(
        &root,
        "whitaker.toml",
        "[cmds.not_a_command.diagnostics]\nlocale = 'cy'\n",
    )?;
    ensure(
        load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err(),
        "a misspelled command namespace should be rejected",
    )?;
    Ok(())
}

/// Assert bumpy-road settings match the lint's normalization policy.
fn assert_bumpy_road_normalized(bumpy: &BumpyRoad) {
    let defaults = BumpyRoad::default();
    assert_eq!(
        bumpy.threshold.total_cmp(&defaults.threshold),
        std::cmp::Ordering::Equal,
        "threshold should fall back to {:?}, got {:?}",
        defaults.threshold,
        bumpy.threshold,
    );
    assert_eq!(
        bumpy.window, defaults.window,
        "window should fall back to the default",
    );
    assert_eq!(bumpy.min_bump_lines, 1, "min_bump_lines should clamp to 1");
    assert_eq!(
        bumpy.weights.depth.total_cmp(&defaults.weights.depth),
        std::cmp::Ordering::Equal,
        "weights.depth should fall back to {:?}, got {:?}",
        defaults.weights.depth,
        bumpy.weights.depth,
    );
}

/// Normalize domain-invalid tuning values supplied by a configuration file.
#[rstest::rstest]
fn normalizes_invalid_file_tuning(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(
        &root,
        "whitaker.toml",
        "[bumpy_road_function]\nthreshold = -1.0\nwindow = 2\nmin_bump_lines = \
         0\n[bumpy_road_function.weights]\ndepth = -1.0\n",
    )?;
    let loaded = load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    assert_bumpy_road_normalized(&loaded.settings.bumpy_road_function);
    Ok(())
}

/// Normalize domain-invalid tuning values supplied by the environment.
#[rstest::rstest]
fn normalizes_invalid_environment_tuning(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    let env = Arc::new(
        MapEnv::new()
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_THRESHOLD", "-1.0")
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_WINDOW", "2")
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_MIN_BUMP_LINES", "0")
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_WEIGHTS_DEPTH", "-1.0"),
    );
    let loaded = load_with_sources(&sources(&root, env, json!({})))?;
    assert_bumpy_road_normalized(&loaded.settings.bumpy_road_function);
    Ok(())
}

/// Normalize domain-invalid tuning values supplied explicitly by the CLI.
#[rstest::rstest]
fn normalizes_invalid_cli_tuning(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    let cli = json!({
        "bumpy_road_function": {
            "threshold": -1.0,
            "window": 2,
            "min_bump_lines": 0,
            "weights": {"depth": -1.0},
        },
    });
    let loaded = load_with_sources(&sources(&root, Arc::new(MapEnv::new()), cli))?;
    assert_bumpy_road_normalized(&loaded.settings.bumpy_road_function);
    Ok(())
}

/// Normalize branch and fixture tuning values from a configuration file.
#[rstest::rstest]
fn normalizes_other_domain_tuning(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(
        &root,
        "whitaker.toml",
        "[conditional_max_n_branches]\nmax_branches = \
         0\n[rstest_helper_should_be_fixture]\nmin_calls = 0\nmin_distinct_tests = \
         1\nprovider_param_attributes = [' rstest::case ', 'case', '', 'values']\n",
    )?;
    let loaded = load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    ensure_eq(
        &loaded.settings.conditional_max_n_branches.max_branches,
        &1,
        "the branch limit is raised to the minimum",
    )?;
    let helper = &loaded.settings.rstest_helper_should_be_fixture;
    ensure_eq(&helper.min_calls, &2, "the call minimum is raised")?;
    ensure_eq(
        &helper.min_distinct_tests,
        &2,
        "the distinct-test minimum is raised",
    )?;
    ensure_eq(
        &helper.provider_param_attributes,
        &["case".to_owned(), "values".to_owned()],
        "attribute names are trimmed, deduplicated, and sorted",
    )?;
    ensure_eq(
        &RstestHelper::default().provider_param_attributes,
        &["case", "values", "files", "future", "context"],
        "the default attribute list is unchanged",
    )?;
    Ok(())
}

/// Reject non-Unicode values for recognized canonical environment variables.
#[rstest::rstest]
#[case("WHITAKER_DIAGNOSTICS_LOCALE")]
#[case("WHITAKER_LINT_SELECT")]
#[case("WHITAKER_MODULE_MAX_LINES_MAX_LINES")]
fn rejects_non_unicode_canonical_values(
    #[from(root)] root_res: std::io::Result<TempDir>,
    #[case] variable: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_rejected_by_environment(
        root_res,
        MapEnv::new().with_var(variable, non_unicode_value()),
        "a non-Unicode value should be rejected",
    )
}
