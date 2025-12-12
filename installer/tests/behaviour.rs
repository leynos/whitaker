//! Behaviour-driven tests for the installer.
//!
//! These tests validate the core logic of the installer using rstest-bdd
//! scenarios that cover crate resolution, toolchain detection, staging,
//! and output generation.

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use whitaker_installer::builder::{
    CrateName, LINT_CRATES, SUITE_CRATE, resolve_crates, validate_crate_names,
};
use whitaker_installer::output::ShellSnippet;
use whitaker_installer::stager::Stager;
use whitaker_installer::toolchain::parse_toolchain_channel;

// ---------------------------------------------------------------------------
// Crate resolution world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct CrateResolutionWorld {
    specific_lints: RefCell<Vec<CrateName>>,
    suite_only: Cell<bool>,
    no_suite: Cell<bool>,
    resolved: RefCell<Vec<CrateName>>,
}

#[fixture]
fn crate_world() -> CrateResolutionWorld {
    CrateResolutionWorld::default()
}

#[given("no specific lints are requested")]
fn given_no_specific_lints(crate_world: &CrateResolutionWorld) {
    crate_world.specific_lints.replace(Vec::new());
}

#[given("suite is not excluded")]
fn given_suite_not_excluded(crate_world: &CrateResolutionWorld) {
    crate_world.no_suite.set(false);
}

#[given("suite-only mode is enabled")]
fn given_suite_only(crate_world: &CrateResolutionWorld) {
    crate_world.suite_only.set(true);
}

#[given("suite is excluded")]
fn given_suite_excluded(crate_world: &CrateResolutionWorld) {
    crate_world.no_suite.set(true);
}

#[given("specific lints are requested")]
fn given_specific_lints(crate_world: &CrateResolutionWorld) {
    crate_world
        .specific_lints
        .replace(vec![CrateName::from("module_max_lines")]);
}

#[when("the crate list is resolved")]
fn when_crates_resolved(crate_world: &CrateResolutionWorld) {
    let lints = crate_world.specific_lints.replace(Vec::new());
    let resolved = resolve_crates(
        &lints,
        crate_world.suite_only.get(),
        crate_world.no_suite.get(),
    );
    crate_world.resolved.replace(resolved);
}

#[then("all lint crates are included")]
fn then_all_lints_included(crate_world: &CrateResolutionWorld) {
    let resolved = crate_world.resolved.borrow();
    for lint in LINT_CRATES {
        assert!(
            resolved.contains(&CrateName::from(*lint)),
            "expected {lint} to be included"
        );
    }
}

#[then("the suite crate is included")]
fn then_suite_included(crate_world: &CrateResolutionWorld) {
    let resolved = crate_world.resolved.borrow();
    assert!(
        resolved.contains(&CrateName::from(SUITE_CRATE)),
        "expected suite to be included"
    );
}

#[then("only the suite crate is included")]
fn then_only_suite(crate_world: &CrateResolutionWorld) {
    let resolved = crate_world.resolved.borrow();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved.first().map(CrateName::as_str), Some(SUITE_CRATE));
}

#[then("the suite crate is not included")]
fn then_suite_not_included(crate_world: &CrateResolutionWorld) {
    let resolved = crate_world.resolved.borrow();
    assert!(
        !resolved.contains(&CrateName::from(SUITE_CRATE)),
        "expected suite to be excluded"
    );
}

#[then("only the requested lints are included")]
fn then_only_requested(crate_world: &CrateResolutionWorld) {
    let resolved = crate_world.resolved.borrow();
    assert_eq!(resolved.len(), 1);
    assert_eq!(
        resolved.first().map(CrateName::as_str),
        Some("module_max_lines")
    );
}

// ---------------------------------------------------------------------------
// Crate name validation world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ValidationWorld {
    names: RefCell<Vec<CrateName>>,
    result: Cell<Option<bool>>,
}

#[fixture]
fn validation_world() -> ValidationWorld {
    ValidationWorld::default()
}

#[given("a list of valid crate names")]
fn given_valid_names(validation_world: &ValidationWorld) {
    validation_world.names.replace(vec![
        CrateName::from("module_max_lines"),
        CrateName::from("suite"),
    ]);
}

#[given("a list containing an unknown crate name")]
fn given_unknown_name(validation_world: &ValidationWorld) {
    validation_world
        .names
        .replace(vec![CrateName::from("nonexistent_lint")]);
}

#[when("the names are validated")]
fn when_names_validated(validation_world: &ValidationWorld) {
    let names = validation_world.names.borrow();
    let result = validate_crate_names(&names).is_ok();
    validation_world.result.set(Some(result));
}

#[then("validation succeeds")]
fn then_validation_succeeds(validation_world: &ValidationWorld) {
    assert_eq!(validation_world.result.get(), Some(true));
}

#[then("validation fails with a lint not found error")]
fn then_validation_fails(validation_world: &ValidationWorld) {
    assert_eq!(validation_world.result.get(), Some(false));
}

// ---------------------------------------------------------------------------
// Toolchain detection world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ToolchainWorld {
    contents: RefCell<String>,
    channel: RefCell<Option<String>>,
    error: Cell<bool>,
}

#[fixture]
fn toolchain_world() -> ToolchainWorld {
    ToolchainWorld::default()
}

#[given("a rust-toolchain.toml with standard format")]
fn given_standard_toolchain(toolchain_world: &ToolchainWorld) {
    toolchain_world.contents.replace(
        r#"
[toolchain]
channel = "nightly-2025-09-18"
components = ["rust-src"]
"#
        .to_owned(),
    );
}

#[given("a rust-toolchain.toml without a channel")]
fn given_no_channel(toolchain_world: &ToolchainWorld) {
    toolchain_world.contents.replace(
        r#"
[toolchain]
components = ["rust-src"]
"#
        .to_owned(),
    );
}

#[when("the toolchain is detected")]
fn when_toolchain_detected(toolchain_world: &ToolchainWorld) {
    let contents = toolchain_world.contents.borrow();

    match parse_toolchain_channel(&contents) {
        Ok(channel) => {
            toolchain_world.channel.replace(Some(channel));
        }
        Err(_) => toolchain_world.error.set(true),
    }
}

#[then("the channel is extracted correctly")]
fn then_channel_extracted(toolchain_world: &ToolchainWorld) {
    let channel = toolchain_world.channel.borrow();
    assert_eq!(*channel, Some("nightly-2025-09-18".to_owned()));
}

#[then("detection fails with an invalid file error")]
fn then_detection_fails(toolchain_world: &ToolchainWorld) {
    assert!(toolchain_world.error.get());
}

// ---------------------------------------------------------------------------
// Shell snippet world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SnippetWorld {
    path: RefCell<String>,
    snippet: RefCell<Option<ShellSnippet>>,
}

#[fixture]
fn snippet_world() -> SnippetWorld {
    SnippetWorld::default()
}

#[given("a target library path")]
fn given_library_path(snippet_world: &SnippetWorld) {
    snippet_world
        .path
        .replace("/home/user/.local/share/dylint/lib".to_owned());
}

#[when("shell snippets are generated")]
fn when_snippets_generated(snippet_world: &SnippetWorld) {
    let path = snippet_world.path.borrow();
    let utf8_path = camino::Utf8PathBuf::from(path.as_str());
    let snippet = ShellSnippet::new(&utf8_path);
    snippet_world.snippet.replace(Some(snippet));
}

#[then("bash snippet uses export syntax")]
fn then_bash_export(snippet_world: &SnippetWorld) {
    let snippet = snippet_world.snippet.borrow();
    let s = snippet.as_ref().expect("snippet should exist");
    assert!(s.bash.starts_with("export "));
}

#[then("fish snippet uses set -gx syntax")]
fn then_fish_set(snippet_world: &SnippetWorld) {
    let snippet = snippet_world.snippet.borrow();
    let s = snippet.as_ref().expect("snippet should exist");
    assert!(s.fish.starts_with("set -gx "));
}

#[then("PowerShell snippet uses $env syntax")]
fn then_powershell_env(snippet_world: &SnippetWorld) {
    let snippet = snippet_world.snippet.borrow();
    let s = snippet.as_ref().expect("snippet should exist");
    assert!(s.powershell.starts_with("$env:"));
}

// ---------------------------------------------------------------------------
// Staging world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct StagingWorld {
    crate_name: RefCell<Option<CrateName>>,
    toolchain: RefCell<String>,
    staged_name: RefCell<String>,
}

#[fixture]
fn staging_world() -> StagingWorld {
    StagingWorld::default()
}

#[given("a built library")]
fn given_built_library(staging_world: &StagingWorld) {
    staging_world
        .crate_name
        .replace(Some(CrateName::from("module_max_lines")));
}

#[given("a staging directory")]
fn given_staging_dir(staging_world: &StagingWorld) {
    staging_world
        .toolchain
        .replace("nightly-2025-09-18".to_owned());
}

#[when("the library is staged")]
fn when_library_staged(staging_world: &StagingWorld) {
    let crate_name = staging_world.crate_name.borrow();
    let crate_name = crate_name.as_ref().expect("crate name not set");
    let toolchain = staging_world.toolchain.borrow();

    // Use the production Stager to compute the filename
    let stager = Stager::new(Utf8PathBuf::from("/tmp/test"), &toolchain);
    let staged_name = stager.staged_filename(crate_name);

    staging_world.staged_name.replace(staged_name);
}

#[then("the staged filename includes the toolchain")]
fn then_staged_includes_toolchain(staging_world: &StagingWorld) {
    let name = staging_world.staged_name.borrow();
    assert!(name.contains("nightly-2025-09-18"));
    assert!(name.contains("module_max_lines"));
}

// ---------------------------------------------------------------------------
// Staging failure world (Unix only - relies on Unix file permissions)
// ---------------------------------------------------------------------------

#[cfg(unix)]
use staging_failure::StagingFailureWorld;
#[cfg(unix)]
use staging_failure::staging_failure_world;

#[cfg(unix)]
mod staging_failure {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;
    use whitaker_installer::error::InstallerError;

    pub struct StagingFailureWorld {
        stager: RefCell<Option<Stager>>,
        result: RefCell<Option<Result<(), InstallerError>>>,
        // Keep temp_dir alive for the lifetime of the test
        _temp_dir: RefCell<Option<TempDir>>,
    }

    impl Default for StagingFailureWorld {
        fn default() -> Self {
            Self {
                stager: RefCell::new(None),
                result: RefCell::new(None),
                _temp_dir: RefCell::new(None),
            }
        }
    }

    #[fixture]
    pub fn staging_failure_world() -> StagingFailureWorld {
        StagingFailureWorld::default()
    }

    #[given("a non-writable staging directory")]
    pub fn given_non_writable_dir(staging_failure_world: &StagingFailureWorld) {
        // Create a temp directory and make it read-only
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let dir_path = temp_dir.path();

        // Create the nested staging path structure that Stager expects
        let staging_path = dir_path.join("nightly-2025-09-18").join("release");
        fs::create_dir_all(&staging_path).expect("failed to create staging path");

        // Make the directory read-only (no write permission)
        let mut perms = fs::metadata(&staging_path)
            .expect("failed to get metadata")
            .permissions();
        perms.set_mode(0o444); // read-only
        fs::set_permissions(&staging_path, perms).expect("failed to set permissions");

        let utf8_path =
            Utf8PathBuf::try_from(dir_path.to_path_buf()).expect("temp dir path not UTF-8");
        let stager = Stager::new(utf8_path, "nightly-2025-09-18");

        staging_failure_world.stager.replace(Some(stager));
        staging_failure_world._temp_dir.replace(Some(temp_dir));
    }

    #[when("the staging directory is prepared")]
    pub fn when_staging_prepared(staging_failure_world: &StagingFailureWorld) {
        let stager = staging_failure_world.stager.borrow();
        let stager = stager.as_ref().expect("stager not set");
        let result = stager.prepare();
        staging_failure_world.result.replace(Some(result));
    }

    #[then("staging fails with a target not writable error")]
    pub fn then_staging_fails_not_writable(staging_failure_world: &StagingFailureWorld) {
        // Skip this assertion when running as root (uid 0) since root can bypass
        // filesystem permissions. This is similar to how CI containers often run.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }

        let result = staging_failure_world.result.borrow();
        let result = result.as_ref().expect("result not set");
        assert!(
            matches!(result, Err(InstallerError::TargetNotWritable { .. })),
            "expected TargetNotWritable error, got {result:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/installer.feature", index = 0)]
fn scenario_resolve_all_crates(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 1)]
fn scenario_resolve_suite_only(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 2)]
fn scenario_resolve_without_suite(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 3)]
fn scenario_resolve_specific_lints(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 4)]
fn scenario_validate_known_names(validation_world: ValidationWorld) {
    let _ = validation_world;
}

#[scenario(path = "tests/features/installer.feature", index = 5)]
fn scenario_reject_unknown_names(validation_world: ValidationWorld) {
    let _ = validation_world;
}

#[scenario(path = "tests/features/installer.feature", index = 6)]
fn scenario_parse_standard_toolchain(toolchain_world: ToolchainWorld) {
    let _ = toolchain_world;
}

#[scenario(path = "tests/features/installer.feature", index = 7)]
fn scenario_reject_missing_channel(toolchain_world: ToolchainWorld) {
    let _ = toolchain_world;
}

#[scenario(path = "tests/features/installer.feature", index = 8)]
fn scenario_generate_shell_snippets(snippet_world: SnippetWorld) {
    let _ = snippet_world;
}

#[scenario(path = "tests/features/installer.feature", index = 9)]
fn scenario_stage_with_toolchain_suffix(staging_world: StagingWorld) {
    let _ = staging_world;
}

#[cfg(unix)]
#[scenario(path = "tests/features/installer.feature", index = 10)]
fn scenario_reject_staging_non_writable(staging_failure_world: StagingFailureWorld) {
    let _ = staging_failure_world;
}
