//! File discovery and legacy compatibility scenarios for the configuration loader.

use std::sync::Arc;

use cap_std::{ambient_authority, fs::Dir};
use ortho_config::MapEnv;
use serde_json::json;
use tempfile::TempDir;
use whitaker_cli::load_with_sources;

use crate::support::{locale, root, sources, write};

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
