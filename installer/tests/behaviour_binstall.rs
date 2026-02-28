//! Behaviour-driven tests for cargo-binstall metadata validation.
//!
//! These scenarios verify that the `[package.metadata.binstall]` section
//! in `installer/Cargo.toml` matches the design document specification
//! and that templates expand correctly for all supported targets.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use toml::Table;
use whitaker_installer::binstall_metadata::{
    BIN_DIR_TEMPLATE, PKG_URL_TEMPLATE, expand_bin_dir, expand_pkg_url, extract_binstall_table,
    load_cargo_toml,
};

// ---------------------------------------------------------------------------
// World type
// ---------------------------------------------------------------------------

/// Mutable state threaded through Gherkin steps.
///
/// Fields use `Option` because the world starts empty (`Default`) and is
/// populated incrementally by Given/When steps. Each Then step asserts on
/// the values set by preceding steps.
#[derive(Default)]
struct BinstallWorld {
    binstall_table: Option<Table>,
    target: Option<String>,
    version: Option<String>,
    expanded_url: Option<String>,
    expanded_bin_dir: Option<String>,
}

#[fixture]
fn world() -> BinstallWorld {
    BinstallWorld::default()
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("the installer Cargo.toml is loaded")]
fn given_cargo_toml_loaded(world: &mut BinstallWorld) {
    let table = load_cargo_toml();
    world.binstall_table = Some(extract_binstall_table(&table));
}

#[given("target \"{target}\" and version \"{version}\"")]
fn given_target_and_version(world: &mut BinstallWorld, target: String, version: String) {
    world.target = Some(target);
    world.version = Some(version);
}

#[when("the binstall metadata section is inspected")]
fn when_binstall_inspected(world: &mut BinstallWorld) {
    // The binstall_table was loaded in the Given step.
    // This step exists for Gherkin readability.
    assert!(
        world.binstall_table.is_some(),
        "binstall table must be loaded before inspection"
    );
}

#[when("the binstall overrides are inspected")]
fn when_overrides_inspected(world: &mut BinstallWorld) {
    // Verify overrides are accessible; the Then step extracts them directly.
    let binstall = world.binstall_table.as_ref().expect("binstall table set");
    assert!(
        binstall.get("overrides").is_some(),
        "overrides table not found"
    );
}

#[when("the pkg-url template is expanded")]
fn when_pkg_url_expanded(world: &mut BinstallWorld) {
    let version = world.version.as_deref().expect("version set");
    let target = world.target.as_deref().expect("target set");
    world.expanded_url = Some(expand_pkg_url(version, target));
}

#[when("the bin-dir template is expanded")]
fn when_bin_dir_expanded(world: &mut BinstallWorld) {
    let version = world.version.as_deref().expect("version set");
    let target = world.target.as_deref().expect("target set");
    world.expanded_bin_dir = Some(expand_bin_dir(version, target));
}

#[then("the pkg-url template is present")]
fn then_pkg_url_present(world: &mut BinstallWorld) {
    let binstall = world.binstall_table.as_ref().expect("binstall table set");
    let pkg_url = binstall
        .get("pkg-url")
        .and_then(|v| v.as_str())
        .expect("pkg-url not found");
    assert_eq!(pkg_url, PKG_URL_TEMPLATE);
}

#[then("the bin-dir template is present")]
fn then_bin_dir_present(world: &mut BinstallWorld) {
    let binstall = world.binstall_table.as_ref().expect("binstall table set");
    let bin_dir = binstall
        .get("bin-dir")
        .and_then(|v| v.as_str())
        .expect("bin-dir not found");
    assert_eq!(bin_dir, BIN_DIR_TEMPLATE);
}

#[then("the default pkg-fmt is \"{expected}\"")]
fn then_default_pkg_fmt(world: &mut BinstallWorld, expected: String) {
    let binstall = world.binstall_table.as_ref().expect("binstall table set");
    let pkg_fmt = binstall
        .get("pkg-fmt")
        .and_then(|v| v.as_str())
        .expect("pkg-fmt not found");
    assert_eq!(pkg_fmt, expected);
}

#[then("the x86_64-pc-windows-msvc override has pkg-fmt \"{expected}\"")]
fn then_windows_override_pkg_fmt(world: &mut BinstallWorld, expected: String) {
    let binstall = world.binstall_table.as_ref().expect("binstall table set");
    let windows = binstall
        .get("overrides")
        .and_then(|o| o.get("x86_64-pc-windows-msvc"))
        .and_then(|w| w.as_table())
        .expect("Windows override not found");
    let pkg_fmt = windows
        .get("pkg-fmt")
        .and_then(|v| v.as_str())
        .expect("pkg-fmt not found in Windows override");
    assert_eq!(pkg_fmt, expected);
}

#[then("the URL ends with \"{suffix}\"")]
fn then_url_ends_with(world: &mut BinstallWorld, suffix: String) {
    let url = world.expanded_url.as_ref().expect("expanded URL set");
    assert!(
        url.ends_with(&suffix),
        "expected URL to end with '{suffix}', got '{url}'"
    );
}

#[then("the URL contains the target triple")]
fn then_url_contains_target(world: &mut BinstallWorld) {
    let url = world.expanded_url.as_ref().expect("expanded URL set");
    let target = world.target.as_deref().expect("target set");
    assert!(
        url.contains(target),
        "expected URL to contain '{target}', got '{url}'"
    );
}

#[then("the path ends with \"{suffix}\"")]
fn then_path_ends_with(world: &mut BinstallWorld, suffix: String) {
    let path = world
        .expanded_bin_dir
        .as_ref()
        .expect("expanded bin-dir set");
    assert!(
        path.ends_with(&suffix),
        "expected path to end with '{suffix}', got '{path}'"
    );
}

#[then("no templates contain the placeholder \"{placeholder}\"")]
fn then_no_invalid_placeholder(world: &mut BinstallWorld, placeholder: String) {
    let binstall = world.binstall_table.as_ref().expect("binstall table set");
    let pkg_url = binstall
        .get("pkg-url")
        .and_then(|v| v.as_str())
        .expect("pkg-url not found");
    let bin_dir = binstall
        .get("bin-dir")
        .and_then(|v| v.as_str())
        .expect("bin-dir not found");
    assert!(
        !pkg_url.contains(&placeholder),
        "pkg-url contains invalid placeholder '{placeholder}'"
    );
    assert!(
        !bin_dir.contains(&placeholder),
        "bin-dir contains invalid placeholder '{placeholder}'"
    );
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(
    path = "tests/features/binstall_metadata.feature",
    name = "Binstall metadata section exists in Cargo.toml"
)]
fn scenario_binstall_metadata_exists(world: BinstallWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/binstall_metadata.feature",
    name = "Windows override uses zip format"
)]
fn scenario_windows_override(world: BinstallWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/binstall_metadata.feature",
    name = "URL template expands correctly for Linux"
)]
fn scenario_url_linux(world: BinstallWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/binstall_metadata.feature",
    name = "URL template expands correctly for Windows"
)]
fn scenario_url_windows(world: BinstallWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/binstall_metadata.feature",
    name = "Binary directory expands correctly for Unix"
)]
fn scenario_bin_dir_unix(world: BinstallWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/binstall_metadata.feature",
    name = "Binary directory expands correctly for Windows"
)]
fn scenario_bin_dir_windows(world: BinstallWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/binstall_metadata.feature",
    name = "No invalid placeholders in templates"
)]
fn scenario_no_invalid_placeholders(world: BinstallWorld) {
    let _ = world;
}
