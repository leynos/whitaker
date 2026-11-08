//! Source-precedence scenarios for the configuration loader.

use std::sync::Arc;

use ortho_config::MapEnv;
use serde_json::json;
use tempfile::TempDir;
use whitaker_cli::load_with_sources;

use crate::support::{ensure, ensure_eq, locale, root, sources, write};

/// Exercise default, file, environment and explicit CLI precedence.
#[rstest::rstest]
fn merges_all_precedence_tiers(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    let empty = Arc::new(MapEnv::new());
    ensure_eq(
        &locale(&sources(&root, empty, json!({})))?,
        &"en-GB",
        "the built-in default applies when nothing is supplied",
    )?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_DIAGNOSTICS_LOCALE", "fr"));
    ensure_eq(
        &locale(&sources(&root, env.clone(), json!({})))?,
        &"fr",
        "the environment outranks the project file",
    )?;
    ensure_eq(
        &(load_with_sources(&sources(
            &root,
            env,
            json!({"diagnostics": {"locale": "de"}}),
        ))?
        .settings
        .diagnostics
        .locale),
        &"de",
        "the CLI value outranks the environment",
    )?;
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
    #[from(root)] root_res: std::io::Result<TempDir>,
    #[case] command: &str,
    #[case] variable_command: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    let mut defaults = sources(&root, Arc::new(MapEnv::new()), json!({}));
    defaults.command = Some(command.to_owned());
    ensure_eq(
        &locale(&defaults)?,
        &"en-GB",
        "a command namespace falls back to the default",
    )?;
    write(
        &root,
        "whitaker.toml",
        format!("[cmds.{command}.diagnostics]\nlocale = 'cy'\n"),
    )?;
    let mut file = sources(&root, Arc::new(MapEnv::new()), json!({}));
    file.command = Some(command.to_owned());
    ensure_eq(
        &locale(&file)?,
        &"cy",
        "the command namespace reads the project file",
    )?;
    let env = Arc::new(MapEnv::new().with_var(
        format!("WHITAKER_CMDS_{variable_command}_DIAGNOSTICS_LOCALE"),
        "fr",
    ));
    let mut env_scoped = sources(&root, env.clone(), json!({}));
    env_scoped.command = Some(command.to_owned());
    ensure_eq(
        &locale(&env_scoped)?,
        &"fr",
        "the command namespace reads the environment",
    )?;
    let mut cli_scoped = sources(&root, env, json!({"diagnostics": {"locale": "de"}}));
    cli_scoped.command = Some(command.to_owned());
    ensure_eq(
        &locale(&cli_scoped)?,
        &"de",
        "the CLI value outranks the environment for a command namespace",
    )?;
    Ok(())
}

/// Prefer the canonical file and apply canonical environment over legacy controls.
#[rstest::rstest]
fn canonical_sources_win_over_legacy(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
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
    let loaded = load_with_sources(&sources(&root, env, json!({})))?;
    ensure(
        loaded.settings.install.allow_build_from_source,
        "the canonical environment value wins over the legacy alias",
    )?;
    ensure_eq(
        &loaded.settings.diagnostics.locale,
        &"en-GB",
        "the legacy file does not contribute when a canonical file exists",
    )?;
    ensure(
        loaded.notices.is_empty(),
        "no deprecation notice applies to the canonical sources",
    )?;
    Ok(())
}

/// Replace selector lists when a higher-priority source supplies a value.
#[rstest::rstest]
fn selector_list_is_replaced(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(&root, "whitaker.toml", "[lint]\nselect = ['DOC']\n")?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_LINT_SELECT", "PAN"));
    let loaded = load_with_sources(&sources(&root, env, json!({})))?;
    ensure_eq(
        &loaded.settings.lint.select,
        &["PAN"],
        "the higher-priority selector list replaces the file list",
    )?;
    Ok(())
}

/// Prefer an explicit CLI file over an environment-selected file.
#[rstest::rstest]
fn explicit_selector_order(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(&root, "environment.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    write(&root, "explicit.toml", "[diagnostics]\nlocale = 'fr'\n")?;
    let env =
        Arc::new(MapEnv::new().with_var("WHITAKER_CONFIG", root.path().join("environment.toml")));
    ensure_eq(
        &locale(&sources(&root, env.clone(), json!({})))?,
        &"cy",
        "the environment selects its file",
    )?;
    let mut input = sources(&root, env, json!({}));
    input.config_path = Some(root.path().join("explicit.toml"));
    ensure_eq(
        &load_with_sources(&input)?.settings.diagnostics.locale,
        &"fr",
        "the explicit selection outranks the environment selection",
    )?;
    Ok(())
}

/// Do not treat absent CLI DTO fields as explicit overrides.
#[rstest::rstest]
fn omitted_cli_values_do_not_mask_file(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(&root, "whitaker.toml", "[diagnostics]\nlocale = 'cy'\n")?;
    let loaded = load_with_sources(&sources(
        &root,
        Arc::new(MapEnv::new()),
        json!({"diagnostics": {"locale": null}}),
    ))?;
    ensure_eq(
        &loaded.settings.diagnostics.locale,
        &"cy",
        "a null CLI field leaves the file value in place",
    )?;
    Ok(())
}

/// Let an explicit false override a lower-priority true.
#[rstest::rstest]
fn explicit_false_overrides_lower_priority_true(
    #[from(root)] root_res: std::io::Result<TempDir>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root_res?;
    write(
        &root,
        "whitaker.toml",
        "[install]\nlazy = true\nprefer_prebuilt = true\n",
    )?;
    let env = Arc::new(MapEnv::new().with_var("WHITAKER_INSTALL_LAZY", "false"));
    let cli = json!({"install": {"prefer_prebuilt": false}});
    let loaded = load_with_sources(&sources(&root, env, cli))?;
    ensure(
        !loaded.settings.install.lazy,
        "an explicit false environment value overrides the file",
    )?;
    ensure(
        !loaded.settings.install.prefer_prebuilt,
        "an explicit false CLI value overrides the file",
    )?;
    Ok(())
}
