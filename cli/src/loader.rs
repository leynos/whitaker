//! Discover files and compose injected sources at the application boundary.

use std::{path::PathBuf, sync::Arc};

use ortho_config::{
    AutomaticMode, ConfigDiscovery, ConfigFilePolicy, ConfigPathSelector, MergeComposer,
    OrthoError, OrthoResult, SharedEnvSource, SharedScanEnvSource,
};
use serde_json::{json, Value};

use crate::{environment, Configuration};

/// Inputs needed to resolve one invocation without mutating the environment.
#[derive(Clone)]
pub struct Sources {
    /// Root directory for project configuration discovery.
    pub project_root: PathBuf,
    /// Optional explicit configuration path supplied by the CLI.
    pub config_path: Option<PathBuf>,
    /// Environment lookup used by file discovery.
    pub discovery: SharedEnvSource,
    /// Environment enumeration used for the settings layer.
    pub environment: SharedScanEnvSource,
    /// Selected command namespace, when a command is being configured.
    pub command: Option<String>,
    /// Values explicitly supplied by the CLI, expressed as a configuration object.
    pub cli: Value,
}

/// Configuration and compatibility notices for the application to present.
pub struct LoadedConfiguration {
    /// Effective merged settings.
    pub settings: Configuration,
    /// Deprecation notices; presentation belongs to the executable.
    pub notices: Vec<String>,
}

/// Resolve injected file, environment and explicit CLI sources.
///
/// # Errors
///
/// Returns a configuration error when a selected file cannot be read or parsed,
/// a recognised environment value is invalid, or merged settings are invalid.
///
/// # Examples
///
/// ```no_run
/// use std::{path::PathBuf, sync::Arc};
/// use ortho_config::MapEnv;
/// use serde_json::json;
/// use whitaker_cli::{Sources, load_with_sources};
/// let env = Arc::new(MapEnv::new());
/// let sources = Sources { project_root: PathBuf::from("."), config_path: None,
///     discovery: env.clone(), environment: env, command: None, cli: json!({}) };
/// let loaded = load_with_sources(sources)?;
/// assert_eq!(loaded.settings.diagnostics.locale, "en-GB");
/// # Ok::<(), std::sync::Arc<ortho_config::OrthoError>>(())
/// ```
pub fn load_with_sources(sources: Sources) -> OrthoResult<LoadedConfiguration> {
    let mut notices = Vec::new();
    let canonical = policy(&sources, "whitaker.toml", ".whitaker.toml", true).resolve_layers();
    let explicit = canonical.selection().is_some();
    let explicit_legacy = canonical.selection().is_some_and(|selection| {
        selection
            .path()
            .file_name()
            .is_some_and(|name| name == "dylint.toml" || name == ".dylint.toml")
    });
    check_errors(&canonical)?;
    let hidden = if explicit {
        None
    } else {
        let outcome = hidden_project_policy(&sources).resolve_layers();
        check_errors(&outcome)?;
        Some(outcome)
    };
    let has_hidden = hidden
        .as_ref()
        .is_some_and(|outcome| !outcome.layers_are_empty());
    let fallback_legacy = !explicit && canonical.layers_are_empty() && !has_hidden;
    let use_legacy = fallback_legacy || explicit_legacy;
    let mut layers = if fallback_legacy {
        let legacy = policy(&sources, "dylint.toml", ".dylint.toml", false).resolve_layers();
        check_errors(&legacy)?;
        if !legacy.layers_are_empty() {
            notices.push(String::from("dylint.toml is deprecated; use whitaker.toml"));
        }
        legacy.into_result()?
    } else {
        if explicit_legacy {
            notices.push(String::from("dylint.toml is deprecated; use whitaker.toml"));
        }
        canonical.into_result()?
    };
    if !fallback_legacy {
        if let Some(hidden) = hidden {
            layers.extend(hidden.into_result()?);
        }
    }
    let mut composer = MergeComposer::new();
    composer.push_defaults(
        serde_json::to_value(Configuration::default())
            .map_err(|error| invalid("defaults", &error.to_string()))?,
    );
    for layer in layers {
        let path = layer.path().map(ToOwned::to_owned);
        let mut value = layer.into_value();
        if use_legacy {
            translate_legacy(&mut value);
        }
        apply_command(&mut value, sources.command.as_deref())?;
        composer.push_file(value, path);
    }
    let (mut legacy_env, mut canonical_env, has_legacy_locale) =
        environment::collect(sources.environment.as_ref())?;
    if has_legacy_locale {
        notices.push(String::from(
            "DYLINT_LOCALE is deprecated; use WHITAKER_DIAGNOSTICS_LOCALE",
        ));
    }
    apply_command(&mut legacy_env, sources.command.as_deref())?;
    apply_command(&mut canonical_env, sources.command.as_deref())?;
    composer.push_environment(legacy_env);
    composer.push_environment(canonical_env);
    let mut cli = ortho_config::sanitize_value(&sources.cli)?;
    apply_command(&mut cli, sources.command.as_deref())?;
    composer.push_cli(cli);
    let settings = Configuration::merge_from_layers(composer.layers())?;
    validate(&settings)?;
    Ok(LoadedConfiguration { settings, notices })
}

/// Build the canonical or compatibility discovery policy.
fn policy(sources: &Sources, file: &str, dotfile: &str, explicit: bool) -> ConfigFilePolicy {
    let builder = ConfigDiscovery::builder("whitaker")
        .env_source(sources.discovery.clone())
        .config_file_name(file)
        .project_file_name(file)
        .dotfile_name(dotfile);
    let mut policy = ConfigFilePolicy::from_builder(builder)
        .project_root(sources.project_root.clone())
        .automatic_mode(AutomaticMode::StackScopes);
    if explicit {
        policy = policy.selectors([
            ConfigPathSelector::cli(sources.config_path.clone()),
            ConfigPathSelector::env("WHITAKER_CONFIG"),
        ]);
    }
    policy
}

/// Resolve the hidden filename at the project root as a project-scope layer.
fn hidden_project_policy(sources: &Sources) -> ConfigFilePolicy {
    let builder = ConfigDiscovery::builder("whitaker")
        .env_source(sources.discovery.clone())
        .project_file_name(".whitaker.toml");
    ConfigFilePolicy::from_builder(builder)
        .project_root(sources.project_root.clone())
        .automatic_mode(AutomaticMode::StackScopes)
        .scope_order([ortho_config::DiscoveryScope::Project])
}

/// Keep automatic candidate failures visible even when another file was loaded.
fn check_errors(outcome: &ortho_config::FileLayerOutcome) -> OrthoResult<()> {
    if let Some(error) = outcome
        .selected_error()
        .or_else(|| outcome.reportable_errors().first())
        .or_else(|| outcome.ignorable_errors().first())
    {
        return Err(error.clone());
    }
    Ok(())
}

/// Map the legacy root locale to the application namespace.
fn translate_legacy(value: &mut Value) {
    if let Value::Object(root) = value {
        root.retain(|key, _| {
            matches!(
                key.as_str(),
                "locale"
                    | "module_max_lines"
                    | "conditional_max_n_branches"
                    | "no_expect_outside_tests"
                    | "test_must_not_have_example"
                    | "no_unwrap_or_else_panic"
                    | "rstest_helper_should_be_fixture"
                    | "no_std_fs_operations"
                    | "bumpy_road_function"
            )
        });
        if let Some(locale) = root.remove("locale") {
            let diagnostics = root.entry("diagnostics").or_insert_with(|| json!({}));
            if let Value::Object(fields) = diagnostics {
                fields.entry("locale").or_insert(locale);
            }
        }
    }
}

/// Build a typed error for a failed internal conversion.
fn invalid(key: &str, message: &str) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: key.to_owned(),
        message: message.to_owned(),
    })
}

/// Overlay the selected namespace within a source, rejecting unknown commands.
fn apply_command(value: &mut Value, selected: Option<&str>) -> OrthoResult<()> {
    const COMMANDS: &[&str] = &[
        "check",
        "install",
        "ls",
        "doctor",
        "rewrite",
        "rewrite_check",
        "rewrite_diff",
        "rewrite_apply",
        "rewrite_explain",
    ];
    if let Some(name) = selected {
        if !COMMANDS.contains(&name) {
            return Err(invalid("command", "unknown command namespace"));
        }
    }
    let Some(root) = value.as_object_mut() else {
        return Err(invalid("configuration", "expected an object"));
    };
    let Some(commands) = root.remove("cmds") else {
        return Ok(());
    };
    let Some(commands) = commands.as_object() else {
        return Err(invalid("cmds", "expected a table"));
    };
    for name in commands.keys() {
        if !COMMANDS.contains(&name.as_str()) {
            return Err(invalid(
                &format!("cmds.{name}"),
                "unknown command namespace",
            ));
        }
    }
    if let Some(command) = selected.and_then(|name| commands.get(name)) {
        if !command.is_object() {
            return Err(invalid("cmds", "command settings must be a table"));
        }
        overlay(root, command);
    }
    Ok(())
}

/// Overlay a command's object over its source's global values.
fn overlay(target: &mut serde_json::Map<String, Value>, source: &Value) {
    if let Some(fields) = source.as_object() {
        for (key, value) in fields {
            if let (Some(Value::Object(existing)), Some(incoming)) =
                (target.get_mut(key), value.as_object())
            {
                overlay(existing, &Value::Object(incoming.clone()));
            } else {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

/// Validate application presentation values before passing settings onward.
fn validate(settings: &Configuration) -> OrthoResult<()> {
    if !["auto", "always", "never"].contains(&settings.output.colour.as_str()) {
        return Err(invalid("output.colour", "expected auto, always, or never"));
    }
    if !["auto", "plain", "off"].contains(&settings.output.progress.as_str()) {
        return Err(invalid("output.progress", "expected auto, plain, or off"));
    }
    Ok(())
}
