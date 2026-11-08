//! File discovery and legacy compatibility scenarios for the configuration loader.

use std::sync::Arc;

use cap_std::{ambient_authority, fs::Dir};
use ortho_config::MapEnv;
use serde_json::json;
use tempfile::TempDir;
use whitaker_cli::load_with_sources;

use crate::support::{ensure, ensure_eq, locale, root, sources, write};

/// Translate both legacy locale sources and report their deprecation.
#[rstest::rstest]
fn reads_legacy_file_and_environment(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(&root, "dylint.toml", "locale = 'cy'\n")?;
    let env = Arc::new(MapEnv::new().with_var("DYLINT_LOCALE", "fr"));
    let loaded = load_with_sources(&sources(&root, env, json!({})))?;
    ensure_eq(
        &loaded.settings.diagnostics.locale,
        &"fr",
        "the environment outranks the legacy file",
    )?;
    ensure_eq(
        &loaded.notices,
        &([
            "dylint.toml is deprecated; use whitaker.toml",
            "DYLINT_LOCALE is deprecated; use WHITAKER_DIAGNOSTICS_LOCALE",
        ]),
        "both deprecations are reported",
    )?;
    Ok(())
}

/// Reject an unusable explicit selection and a malformed discovered file.
#[rstest::rstest]
#[case(Some("missing.toml"), "whitaker.toml", "[diagnostics\n")]
#[case(None, "dylint.toml", "[broken\n")]
fn rejects_unusable_files(
    #[from(root)] root_res: std::io::Result<TempDir>,
    #[case] explicit_child: Option<&str>,
    #[case] discovered_name: &str,
    #[case] discovered_contents: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    // `None` selects the project directory itself, which cannot be read as a file.
    let explicit =
        explicit_child.map_or_else(|| root.path().to_path_buf(), |name| root.path().join(name));
    let env = Arc::new(MapEnv::new());
    let mut selected = sources(&root, env.clone(), json!({}));
    selected.config_path = Some(explicit);
    ensure(
        load_with_sources(&selected).is_err(),
        "an unreadable explicit selection should fail",
    )?;
    write(&root, discovered_name, discovered_contents)?;
    ensure(
        load_with_sources(&sources(&root, env, json!({}))).is_err(),
        "a malformed discovered file should fail",
    )?;
    Ok(())
}

/// Preserve known legacy lint settings while tolerating unrelated Dylint tables.
#[rstest::rstest]
fn bridges_legacy_lint_tables(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(
        &root,
        "dylint.toml",
        "[module_max_lines]\nmax_lines = 123\n[other_lint]\nsetting = \
         true\n[bumpy_road_function.weights]\ndepth = 2.0\n",
    )?;
    let loaded = load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    ensure_eq(
        &loaded.settings.module_max_lines.max_lines,
        &123,
        "the legacy line limit is kept",
    )?;
    let depth = loaded.settings.bumpy_road_function.weights.depth;
    // `total_cmp` compares the exact ordering; the loaded value is parsed from
    // `2.0`, so an exact match is the contract here and avoids float arithmetic.
    ensure(
        depth.total_cmp(&2.0).is_eq(),
        format!("the legacy weight depth is kept, got {depth}"),
    )?;
    Ok(())
}

/// Bridge a legacy file explicitly selected by the application.
#[rstest::rstest]
fn explicitly_selected_legacy_file_is_translated(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(&root, "dylint.toml", "locale = 'cy'\n")?;
    let mut input = sources(&root, Arc::new(MapEnv::new()), json!({}));
    input.config_path = Some(root.path().join("dylint.toml"));
    let loaded = load_with_sources(&input)?;
    ensure_eq(
        &loaded.settings.diagnostics.locale,
        &"cy",
        "the explicit legacy file is translated",
    )?;
    ensure_eq(
        &loaded.notices,
        &["dylint.toml is deprecated; use whitaker.toml"],
        "the legacy filename is deprecated",
    )?;
    Ok(())
}

/// Stack system, user, and project files in increasing precedence order.
#[rstest::rstest]
fn stacks_discovery_scopes(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority())?;
    dir.create_dir_all("system/whitaker")?;
    dir.create_dir_all("user/whitaker")?;
    dir.write(
        "system/whitaker/whitaker.toml",
        "[diagnostics]\nlocale = 'cy'\n",
    )?;
    let env = Arc::new(MapEnv::new());
    ensure_eq(
        &locale(&sources(&root, env.clone(), json!({})))?,
        &"cy",
        "the system file applies when it is the only source",
    )?;
    dir.write(
        "user/whitaker/whitaker.toml",
        "[diagnostics]\nlocale = 'fr'\n",
    )?;
    ensure_eq(
        &locale(&sources(&root, env.clone(), json!({})))?,
        &"fr",
        "the user file outranks the system file",
    )?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'de'\n")?;
    ensure_eq(
        &(load_with_sources(&sources(&root, env, json!({})))?
            .settings
            .diagnostics
            .locale),
        &"de",
        "the project file outranks the user file",
    )?;
    Ok(())
}

/// Surface a malformed lower-priority file even if a later file is valid.
#[rstest::rstest]
fn rejects_malformed_lower_scope(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority())?;
    dir.create_dir_all("system/whitaker")?;
    dir.write("system/whitaker/whitaker.toml", "[broken\n")?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    ensure(
        load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({}))).is_err(),
        "a malformed lower scope should surface",
    )?;
    Ok(())
}

/// Load the hidden project file and prefer it when both project names exist.
#[rstest::rstest]
fn hidden_project_file_is_discovered(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(&root, ".whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    write(&root, "dylint.toml", "locale = 'fr'\n")?;
    let env = Arc::new(MapEnv::new());
    let loaded = load_with_sources(&sources(&root, env.clone(), json!({})))?;
    ensure_eq(
        &loaded.settings.diagnostics.locale,
        &"cy",
        "the hidden project file is discovered",
    )?;
    ensure(
        loaded.notices.is_empty(),
        "a lone hidden project file needs no deprecation notice",
    )?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'de'\n")?;
    ensure_eq(
        &(load_with_sources(&sources(&root, env, json!({})))?
            .settings
            .diagnostics
            .locale),
        &"cy",
        "the hidden project file outranks the legacy name",
    )?;
    Ok(())
}

/// Merge a parent file before the extending project configuration.
#[rstest::rstest]
fn loads_toml_extends_chain(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
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
    let loaded = load_with_sources(&sources(&root, Arc::new(MapEnv::new()), json!({})))?;
    ensure_eq(
        &loaded.settings.diagnostics.locale,
        &"fr",
        "the extending file overrides its parent",
    )?;
    ensure_eq(
        &loaded.settings.output.colour,
        &"never",
        "a parent-only value is inherited",
    )?;
    Ok(())
}
