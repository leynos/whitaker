//! Source-precedence scenarios for the configuration loader.

use std::sync::Arc;

use ortho_config::MapEnv;
use serde_json::json;
use tempfile::TempDir;
use whitaker_cli::load_with_sources;

use crate::support::{locale, root, sources, write};

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
