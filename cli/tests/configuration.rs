//! Exercise configuration loading with isolated files and injected environments.

use std::sync::Arc;

use cap_std::{ambient_authority, fs::Dir};

use ortho_config::MapEnv;
use serde_json::json;
use tempfile::TempDir;
use whitaker_cli::{load_with_sources, BumpyRoad, RstestHelper, Sources};

/// Write a fixture relative to its capability-owned directory.
fn write(root: &TempDir, name: &str, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    Dir::open_ambient_dir(root.path(), ambient_authority())?.write(name, contents)
}

/// Allocate a private project root for each scenario.
#[rstest::fixture]
fn root() -> std::io::Result<TempDir> {
    TempDir::new()
}

/// Produce an operating-system string that cannot be decoded as Unicode.
#[cfg(unix)]
fn non_unicode_value() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::from_bytes(b"\xff").to_os_string()
}

/// Produce an operating-system string that cannot be decoded as Unicode.
#[cfg(windows)]
fn non_unicode_value() -> std::ffi::OsString {
    use std::os::windows::ffi::OsStringExt;
    std::ffi::OsString::from_wide(&[0xD800])
}

/// Read the effective locale from one injected source bundle.
fn locale(input: Sources) -> ortho_config::OrthoResult<String> {
    Ok(load_with_sources(input)?.settings.diagnostics.locale)
}

/// Construct sources rooted in a private test directory.
fn sources(root: &TempDir, env: Arc<MapEnv>, cli: serde_json::Value) -> Sources {
    Sources {
        project_root: root.path().to_path_buf(),
        config_path: None,
        discovery: Arc::new(
            env.as_ref()
                .clone()
                .with_var("XDG_CONFIG_DIRS", root.path().join("system"))
                .with_var("XDG_CONFIG_HOME", root.path().join("user")),
        ),
        environment: env,
        command: None,
        cli,
    }
}

/// Exercise default, file, environment and explicit CLI precedence.
#[rstest::rstest]
fn merges_all_precedence_tiers(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let empty = Arc::new(MapEnv::new());
    assert_eq!(locale(sources(&root, empty, json!({})))?, "en-GB");
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_DIAGNOSTICS_LOCALE", "fr"));
    assert_eq!(locale(sources(&root, env.clone(), json!({})))?, "fr");
    assert_eq!(
        load_with_sources(sources(
            &root,
            env,
            json!({"diagnostics": {"locale": "de"}})
        ))?
        .settings
        .diagnostics
        .locale,
        "de"
    );
    Ok(())
}

/// Translate both legacy locale sources and report their deprecation.
#[rstest::rstest]
fn reads_legacy_file_and_environment(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(&root, "dylint.toml", "locale = 'cy'\n")?;
    let env = Arc::new(MapEnv::new().with_var("DYLINT_LOCALE", "fr"));
    let loaded = load_with_sources(sources(&root, env, json!({})))?;
    assert_eq!(loaded.settings.diagnostics.locale, "fr");
    assert_eq!(
        loaded.notices,
        [
            "dylint.toml is deprecated; use whitaker.toml",
            "DYLINT_LOCALE is deprecated; use WHITAKER_DIAGNOSTICS_LOCALE",
        ]
    );
    Ok(())
}

/// Reject a missing explicit file and a malformed discovered file.
#[rstest::rstest]
fn rejects_unusable_files(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let env = Arc::new(MapEnv::new());
    let mut selected = sources(&root, env.clone(), json!({}));
    selected.config_path = Some(root.path().join("missing.toml"));
    assert!(load_with_sources(selected).is_err());
    write(&root, "whitaker.toml", "[diagnostics\n")?;
    assert!(load_with_sources(sources(&root, env, json!({}))).is_err());
    Ok(())
}

/// Keep command-specific values in the same source order as global values.
#[rstest::rstest]
#[case("check", "CHECK")]
#[case("install", "INSTALL")]
#[case("ls", "LS")]
#[case("doctor", "DOCTOR")]
#[case("rewrite", "REWRITE")]
#[case("rewrite_check", "REWRITE_CHECK")]
#[case("rewrite_diff", "REWRITE_DIFF")]
#[case("rewrite_apply", "REWRITE_APPLY")]
#[case("rewrite_explain", "REWRITE_EXPLAIN")]
fn command_precedence(
    root: std::io::Result<TempDir>,
    #[case] command: &str,
    #[case] variable_command: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let mut defaults = sources(&root, Arc::new(MapEnv::new()), json!({}));
    defaults.command = Some(command.to_owned());
    assert_eq!(locale(defaults)?, "en-GB");
    write(
        &root,
        "whitaker.toml",
        format!("[cmds.{command}.diagnostics]\nlocale = 'cy'\n"),
    )?;
    let mut file = sources(&root, Arc::new(MapEnv::new()), json!({}));
    file.command = Some(command.to_owned());
    assert_eq!(locale(file)?, "cy");
    let env = Arc::new(MapEnv::new().with_var(
        format!("WHITAKER_CMDS_{variable_command}_DIAGNOSTICS_LOCALE"),
        "fr",
    ));
    let mut args = sources(&root, env.clone(), json!({}));
    args.command = Some(command.to_owned());
    assert_eq!(locale(args)?, "fr");
    let mut args = sources(&root, env, json!({"diagnostics": {"locale": "de"}}));
    args.command = Some(command.to_owned());
    assert_eq!(locale(args)?, "de");
    Ok(())
}

/// Preserve known legacy lint settings while tolerating unrelated Dylint tables.
#[rstest::rstest]
fn bridges_legacy_lint_tables(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(&root, "dylint.toml", "[module_max_lines]\nmax_lines = 123\n[other_lint]\nsetting = true\n[bumpy_road_function.weights]\ndepth = 2.0\n")?;
    let loaded = load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    assert_eq!(loaded.settings.module_max_lines.max_lines, 123);
    assert_eq!(loaded.settings.bumpy_road_function.weights.depth, 2.0);
    Ok(())
}

/// Reject malformed values and unknown canonical keys.
#[rstest::rstest]
fn rejects_invalid_values_and_unknown_keys(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(
        &root,
        "whitaker.toml",
        "[module_max_lines]\nmax_lines = 'large'\n",
    )?;
    assert!(load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err());
    write(&root, "whitaker.toml", "[not_a_setting]\nvalue = 1\n")?;
    assert!(load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err());
    Ok(())
}

/// Prefer the canonical file and apply canonical environment over legacy controls.
#[rstest::rstest]
fn canonical_sources_win_over_legacy(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(
        &root,
        "whitaker.toml",
        "[install]\nallow_build_from_source = false\n",
    )?;
    write(&root, "dylint.toml", "locale = 'cy'\n")?;
    let env = Arc::new(
        MapEnv::new()
            .with_var("WHITAKER_NO_SOURCE_FALLBACK", "1")
            .with_var("WHITAKER_INSTALL_ALLOW_BUILD_FROM_SOURCE", "true"),
    );
    let loaded = load_with_sources(sources(&root, env, json!({})))?;
    assert!(loaded.settings.install.allow_build_from_source);
    assert_eq!(loaded.settings.diagnostics.locale, "en-GB");
    assert!(loaded.notices.is_empty());
    Ok(())
}

/// Replace selector lists when a higher-priority source supplies a value.
#[rstest::rstest]
fn selector_list_is_replaced(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(&root, "whitaker.toml", "[lint]\nselect = ['DOC']\n")?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_LINT_SELECT", "PAN"));
    let loaded = load_with_sources(sources(&root, env, json!({})))?;
    assert_eq!(loaded.settings.lint.select, ["PAN"]);
    Ok(())
}

/// Reject invalid presentation settings before returning application values.
#[rstest::rstest]
fn rejects_unknown_output_policy(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_OUTPUT_COLOUR", "sometimes"));
    assert!(load_with_sources(sources(&root, env, json!({}))).is_err());
    Ok(())
}

/// Prefer an explicit CLI file over an environment-selected file.
#[rstest::rstest]
fn explicit_selector_order(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(&root, "environment.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    write(&root, "explicit.toml", "[diagnostics]\nlocale = 'fr'\n")?;
    let env =
        Arc::new(MapEnv::new().with_var("WHITAKER_CONFIG", root.path().join("environment.toml")));
    assert_eq!(locale(sources(&root, env.clone(), json!({})))?, "cy");
    let mut input = sources(&root, env, json!({}));
    input.config_path = Some(root.path().join("explicit.toml"));
    assert_eq!(load_with_sources(input)?.settings.diagnostics.locale, "fr");
    Ok(())
}

/// Fail closed for an unreadable selection or a malformed legacy file.
#[rstest::rstest]
fn rejects_unreadable_and_malformed_legacy(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let env = Arc::new(MapEnv::new());
    let mut input = sources(&root, env.clone(), json!({}));
    input.config_path = Some(root.path().to_path_buf());
    assert!(load_with_sources(input).is_err());
    write(&root, "dylint.toml", "[broken\n")?;
    assert!(load_with_sources(sources(&root, env, json!({}))).is_err());
    Ok(())
}

/// Do not treat absent CLI DTO fields as explicit overrides.
#[rstest::rstest]
fn omitted_cli_values_do_not_mask_file(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    let loaded = load_with_sources(sources(
        &root,
        Arc::new(MapEnv::new()),
        json!({"diagnostics": {"locale": null}}),
    ))?;
    assert_eq!(loaded.settings.diagnostics.locale, "cy");
    Ok(())
}

/// Bridge a legacy file explicitly selected by the application.
#[rstest::rstest]
fn explicitly_selected_legacy_file_is_translated(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(&root, "dylint.toml", "locale = 'cy'\n")?;
    let mut input = sources(&root, Arc::new(MapEnv::new()), json!({}));
    input.config_path = Some(root.path().join("dylint.toml"));
    let loaded = load_with_sources(input)?;
    assert_eq!(loaded.settings.diagnostics.locale, "cy");
    assert_eq!(
        loaded.notices,
        ["dylint.toml is deprecated; use whitaker.toml"]
    );
    Ok(())
}

/// Stack system, user, and project files in increasing precedence order.
#[rstest::rstest]
fn stacks_discovery_scopes(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority())?;
    dir.create_dir_all("system/whitaker")?;
    dir.create_dir_all("user/whitaker")?;
    dir.write(
        "system/whitaker/whitaker.toml",
        "[diagnostics]\nlocale = 'cy'\n",
    )?;
    let env = Arc::new(MapEnv::new());
    assert_eq!(locale(sources(&root, env.clone(), json!({})))?, "cy");
    dir.write(
        "user/whitaker/whitaker.toml",
        "[diagnostics]\nlocale = 'fr'\n",
    )?;
    assert_eq!(locale(sources(&root, env.clone(), json!({})))?, "fr");
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'de'\n")?;
    assert_eq!(
        load_with_sources(sources(&root, env, json!({})))?
            .settings
            .diagnostics
            .locale,
        "de"
    );
    Ok(())
}

/// Surface a malformed lower-priority file even if a later file is valid.
#[rstest::rstest]
fn rejects_malformed_lower_scope(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority())?;
    dir.create_dir_all("system/whitaker")?;
    dir.write("system/whitaker/whitaker.toml", "[broken\n")?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    assert!(load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err());
    Ok(())
}

/// Load the hidden project file and prefer it when both project names exist.
#[rstest::rstest]
fn hidden_project_file_is_discovered(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(&root, ".whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    write(&root, "dylint.toml", "locale = 'fr'\n")?;
    let env = Arc::new(MapEnv::new());
    let loaded = load_with_sources(sources(&root, env.clone(), json!({})))?;
    assert_eq!(loaded.settings.diagnostics.locale, "cy");
    assert!(loaded.notices.is_empty());
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'de'\n")?;
    assert_eq!(
        load_with_sources(sources(&root, env, json!({})))?
            .settings
            .diagnostics
            .locale,
        "cy"
    );
    Ok(())
}

/// Merge a parent file before the extending project configuration.
#[rstest::rstest]
fn loads_toml_extends_chain(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(
        &root,
        "base.toml",
        "[diagnostics]\nlocale = 'cy'\n[output]\ncolour = 'never'\n",
    )?;
    write(
        &root,
        "whitaker.toml",
        "extends = 'base.toml'\n[diagnostics]\nlocale = 'fr'\n",
    )?;
    let loaded = load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    assert_eq!(loaded.settings.diagnostics.locale, "fr");
    assert_eq!(loaded.settings.output.colour, "never");
    Ok(())
}

/// Reject misspelled command namespaces instead of silently discarding them.
#[rstest::rstest]
fn rejects_unknown_command_namespace(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(
        &root,
        "whitaker.toml",
        "[cmds.not_a_command.diagnostics]\nlocale = 'cy'\n",
    )?;
    assert!(load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err());
    Ok(())
}

/// Assert bumpy-road settings match the lint's normalization policy.
fn assert_bumpy_road_normalized(bumpy: &BumpyRoad) {
    let defaults = BumpyRoad::default();
    assert_eq!(bumpy.threshold, defaults.threshold);
    assert_eq!(bumpy.window, defaults.window);
    assert_eq!(bumpy.min_bump_lines, 1);
    assert_eq!(bumpy.weights.depth, defaults.weights.depth);
}

/// Normalize domain-invalid tuning values supplied by a configuration file.
#[rstest::rstest]
fn normalizes_invalid_file_tuning(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(
        &root,
        "whitaker.toml",
        "[bumpy_road_function]\nthreshold = -1.0\nwindow = 2\nmin_bump_lines = 0\n\
         [bumpy_road_function.weights]\ndepth = -1.0\n",
    )?;
    let loaded = load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    assert_bumpy_road_normalized(&loaded.settings.bumpy_road_function);
    Ok(())
}

/// Normalize domain-invalid tuning values supplied by the environment.
#[rstest::rstest]
fn normalizes_invalid_environment_tuning(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let env = Arc::new(
        MapEnv::new()
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_THRESHOLD", "-1.0")
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_WINDOW", "2")
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_MIN_BUMP_LINES", "0")
            .with_var("WHITAKER_BUMPY_ROAD_FUNCTION_WEIGHTS_DEPTH", "-1.0"),
    );
    let loaded = load_with_sources(sources(&root, env, json!({})))?;
    assert_bumpy_road_normalized(&loaded.settings.bumpy_road_function);
    Ok(())
}

/// Normalize domain-invalid tuning values supplied explicitly by the CLI.
#[rstest::rstest]
fn normalizes_invalid_cli_tuning(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let cli = json!({
        "bumpy_road_function": {
            "threshold": -1.0,
            "window": 2,
            "min_bump_lines": 0,
            "weights": {"depth": -1.0},
        },
    });
    let loaded = load_with_sources(sources(&root, Arc::new(MapEnv::new()), cli))?;
    assert_bumpy_road_normalized(&loaded.settings.bumpy_road_function);
    Ok(())
}

/// Normalize branch and fixture tuning values from a configuration file.
#[rstest::rstest]
fn normalizes_other_domain_tuning(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(
        &root,
        "whitaker.toml",
        "[conditional_max_n_branches]\nmax_branches = 0\n\
         [rstest_helper_should_be_fixture]\nmin_calls = 0\nmin_distinct_tests = 1\n\
         provider_param_attributes = [' rstest::case ', 'case', '', 'values']\n",
    )?;
    let loaded = load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    assert_eq!(loaded.settings.conditional_max_n_branches.max_branches, 1);
    let helper = &loaded.settings.rstest_helper_should_be_fixture;
    assert_eq!(helper.min_calls, 2);
    assert_eq!(helper.min_distinct_tests, 2);
    assert_eq!(helper.provider_param_attributes, ["case", "values"]);
    assert_eq!(
        RstestHelper::default().provider_param_attributes,
        ["case", "values", "files", "future", "context"]
    );
    Ok(())
}

/// Let an explicit false override a lower-priority true.
#[rstest::rstest]
fn explicit_false_overrides_lower_priority_true(
    root: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    write(
        &root,
        "whitaker.toml",
        "[install]\nlazy = true\nprefer_prebuilt = true\n",
    )?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_INSTALL_LAZY", "false"));
    let cli = json!({"install": {"prefer_prebuilt": false}});
    let loaded = load_with_sources(sources(&root, env, cli))?;
    assert!(!loaded.settings.install.lazy);
    assert!(!loaded.settings.install.prefer_prebuilt);
    Ok(())
}

/// Reject non-Unicode values for recognized canonical environment variables.
#[rstest::rstest]
#[case("WHITAKER_DIAGNOSTICS_LOCALE")]
#[case("WHITAKER_LINT_SELECT")]
#[case("WHITAKER_MODULE_MAX_LINES_MAX_LINES")]
fn rejects_non_unicode_canonical_values(
    root: std::io::Result<TempDir>,
    #[case] variable: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root?;
    let env = Arc::new(MapEnv::new().with_var(variable, non_unicode_value()));
    assert!(load_with_sources(sources(&root, env, json!({}))).is_err());
    Ok(())
}
