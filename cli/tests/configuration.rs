//! Exercise configuration loading with isolated files and injected environments.

use std::sync::Arc;

use cap_std::{ambient_authority, fs::Dir};

use ortho_config::MapEnv;
use serde_json::json;
use tempfile::TempDir;
use whitaker_cli::{load_with_sources, Sources};

/// Write a fixture relative to its capability-owned directory.
fn write(root: &TempDir, name: &str, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    Dir::open_ambient_dir(root.path(), ambient_authority())?.write(name, contents)
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
#[test]
fn merges_all_precedence_tiers() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let empty = Arc::new(MapEnv::new());
    assert_eq!(
        load_with_sources(sources(&root, empty, json!({})))?
            .settings
            .diagnostics
            .locale,
        "en-GB"
    );
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_DIAGNOSTICS_LOCALE", "fr"));
    assert_eq!(
        load_with_sources(sources(&root, env.clone(), json!({})))?
            .settings
            .diagnostics
            .locale,
        "fr"
    );
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
#[test]
fn reads_legacy_file_and_environment() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
#[test]
fn rejects_unusable_files() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
    #[case] command: &str,
    #[case] variable_command: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let mut defaults = sources(&root, Arc::new(MapEnv::new()), json!({}));
    defaults.command = Some(command.to_owned());
    assert_eq!(
        load_with_sources(defaults)?.settings.diagnostics.locale,
        "en-GB"
    );
    write(
        &root,
        "whitaker.toml",
        format!("[cmds.{command}.diagnostics]\nlocale = 'cy'\n"),
    )?;
    let mut file = sources(&root, Arc::new(MapEnv::new()), json!({}));
    file.command = Some(command.to_owned());
    assert_eq!(load_with_sources(file)?.settings.diagnostics.locale, "cy");
    let env = Arc::new(MapEnv::new().with_var(
        format!("WHITAKER_CMDS_{variable_command}_DIAGNOSTICS_LOCALE"),
        "fr",
    ));
    let mut args = sources(&root, env.clone(), json!({}));
    args.command = Some(command.to_owned());
    assert_eq!(load_with_sources(args)?.settings.diagnostics.locale, "fr");
    let mut args = sources(&root, env, json!({"diagnostics": {"locale": "de"}}));
    args.command = Some(command.to_owned());
    assert_eq!(load_with_sources(args)?.settings.diagnostics.locale, "de");
    Ok(())
}

/// Preserve known legacy lint settings while tolerating unrelated Dylint tables.
#[test]
fn bridges_legacy_lint_tables() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    write(&root, "dylint.toml", "[module_max_lines]\nmax_lines = 123\n[other_lint]\nsetting = true\n[bumpy_road_function.weights]\ndepth = 2.0\n")?;
    let loaded = load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    assert_eq!(loaded.settings.module_max_lines.max_lines, 123);
    assert_eq!(loaded.settings.bumpy_road_function.weights.depth, 2.0);
    Ok(())
}

/// Reject malformed values and unknown canonical keys.
#[test]
fn rejects_invalid_values_and_unknown_keys() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
#[test]
fn canonical_sources_win_over_legacy() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
#[test]
fn selector_list_is_replaced() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    write(&root, "whitaker.toml", "[lint]\nselect = ['DOC']\n")?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_LINT_SELECT", "PAN"));
    let loaded = load_with_sources(sources(&root, env, json!({})))?;
    assert_eq!(loaded.settings.lint.select, ["PAN"]);
    Ok(())
}

/// Reject invalid presentation settings before returning application values.
#[test]
fn rejects_unknown_output_policy() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_OUTPUT_COLOUR", "sometimes"));
    assert!(load_with_sources(sources(&root, env, json!({}))).is_err());
    Ok(())
}

/// Prefer an explicit CLI file over an environment-selected file.
#[test]
fn explicit_selector_order() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    write(&root, "environment.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    write(&root, "explicit.toml", "[diagnostics]\nlocale = 'fr'\n")?;
    let env =
        Arc::new(MapEnv::new().with_var("WHITAKER_CONFIG", root.path().join("environment.toml")));
    assert_eq!(
        load_with_sources(sources(&root, env.clone(), json!({})))?
            .settings
            .diagnostics
            .locale,
        "cy"
    );
    let mut input = sources(&root, env, json!({}));
    input.config_path = Some(root.path().join("explicit.toml"));
    assert_eq!(load_with_sources(input)?.settings.diagnostics.locale, "fr");
    Ok(())
}

/// Fail closed for an unreadable selection or a malformed legacy file.
#[test]
fn rejects_unreadable_and_malformed_legacy() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let env = Arc::new(MapEnv::new());
    let mut input = sources(&root, env.clone(), json!({}));
    input.config_path = Some(root.path().to_path_buf());
    assert!(load_with_sources(input).is_err());
    write(&root, "dylint.toml", "[broken\n")?;
    assert!(load_with_sources(sources(&root, env, json!({}))).is_err());
    Ok(())
}

/// Do not treat absent CLI DTO fields as explicit overrides.
#[test]
fn omitted_cli_values_do_not_mask_file() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
#[test]
fn explicitly_selected_legacy_file_is_translated() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
#[test]
fn stacks_discovery_scopes() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority())?;
    dir.create_dir_all("system/whitaker")?;
    dir.create_dir_all("user/whitaker")?;
    dir.write(
        "system/whitaker/whitaker.toml",
        "[diagnostics]\nlocale = 'cy'\n",
    )?;
    let env = Arc::new(MapEnv::new());
    assert_eq!(
        load_with_sources(sources(&root, env.clone(), json!({})))?
            .settings
            .diagnostics
            .locale,
        "cy"
    );
    dir.write(
        "user/whitaker/whitaker.toml",
        "[diagnostics]\nlocale = 'fr'\n",
    )?;
    assert_eq!(
        load_with_sources(sources(&root, env.clone(), json!({})))?
            .settings
            .diagnostics
            .locale,
        "fr"
    );
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
#[test]
fn rejects_malformed_lower_scope() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority())?;
    dir.create_dir_all("system/whitaker")?;
    dir.write("system/whitaker/whitaker.toml", "[broken\n")?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    assert!(load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err());
    Ok(())
}

/// Load the hidden project file and prefer it when both project names exist.
#[test]
fn hidden_project_file_is_discovered() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
#[test]
fn loads_toml_extends_chain() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
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
#[test]
fn rejects_unknown_command_namespace() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    write(
        &root,
        "whitaker.toml",
        "[cmds.chekc.diagnostics]\nlocale = 'cy'\n",
    )?;
    assert!(load_with_sources(sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err());
    Ok(())
}
