//! Behaviour-driven tests for cargo-binstall metadata validation.
//!
//! These scenarios verify that the `[package.metadata.binstall]` section
//! in `installer/Cargo.toml` matches the design document specification
//! and that templates expand correctly for all supported targets.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use toml::Table;
use whitaker_installer::binstall_metadata::{
    BIN_DIR_TEMPLATE, PKG_URL_TEMPLATE, WINDOWS_OVERRIDE_TARGET, expand_bin_dir, expand_pkg_url,
    extract_binstall_table, load_cargo_toml,
};

// ---------------------------------------------------------------------------
// World type
// ---------------------------------------------------------------------------

/// Mutable state threaded through Gherkin steps.
///
/// `binstall_table` remains `Option` because `toml::Table` has no meaningful
/// empty default; the remaining fields use plain `String` (defaulting to
/// empty) and are populated by Given/When steps before Then steps read them.
#[derive(Default)]
struct BinstallWorld {
    binstall_table: Option<Table>,
    target: String,
    version: String,
    expanded_url: String,
    expanded_bin_dir: String,
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> BinstallWorld {
    BinstallWorld::default()
}

/// Fetch the binstall table, failing if it has not been loaded.
fn binstall_table(world: &BinstallWorld) -> Result<&Table, String> {
    world
        .binstall_table
        .as_ref()
        .ok_or_else(|| String::from("binstall table must be loaded"))
}

/// Read a string-valued key from a TOML table, failing if absent.
fn table_str<'a>(table: &'a Table, key: &str) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{key} not found"))
}

/// Assert that a string-valued binstall key equals `expected`.
///
/// The mismatch is returned as an `Err` rather than asserted: these steps
/// return `Result`, so a panic here would trip `clippy::panic_in_result_fn`,
/// and returning the message keeps the failure reporting identical to the
/// other steps in this file.
fn assert_binstall_value_equals(
    world: &BinstallWorld,
    key: &str,
    expected: &str,
    value_name: &str,
) -> Result<(), String> {
    let binstall = binstall_table(world)?;
    let actual = table_str(binstall, key)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{value_name} mismatch: expected {expected}, got {actual}"
        ))
    }
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("the installer Cargo.toml is loaded")]
fn given_cargo_toml_loaded(world: &mut BinstallWorld) -> Result<(), String> {
    let table = load_cargo_toml();
    world.binstall_table = Some(extract_binstall_table(&table));
    Ok(())
}

#[given("target \"{target}\" and version \"{version}\"")]
fn given_target_and_version(world: &mut BinstallWorld, target: String, version: String) {
    world.target = target;
    world.version = version;
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
fn when_overrides_inspected(world: &mut BinstallWorld) -> Result<(), String> {
    // Verify overrides are accessible; the Then step extracts them directly.
    let binstall = binstall_table(world)?;
    if binstall.get("overrides").is_none() {
        return Err(String::from("overrides table not found"));
    }
    Ok(())
}

#[when("the pkg-url template is expanded")]
fn when_pkg_url_expanded(world: &mut BinstallWorld) {
    world.expanded_url = expand_pkg_url(&world.version, &world.target);
}

#[when("the bin-dir template is expanded")]
fn when_bin_dir_expanded(world: &mut BinstallWorld) {
    world.expanded_bin_dir = expand_bin_dir(&world.version, &world.target);
}

#[then("the pkg-url template is present")]
fn then_pkg_url_present(world: &mut BinstallWorld) -> Result<(), String> {
    assert_binstall_value_equals(world, "pkg-url", PKG_URL_TEMPLATE, "pkg-url")
}

#[then("the bin-dir template is present")]
fn then_bin_dir_present(world: &mut BinstallWorld) -> Result<(), String> {
    assert_binstall_value_equals(world, "bin-dir", BIN_DIR_TEMPLATE, "bin-dir")
}

#[then("the default pkg-fmt is \"{expected}\"")]
fn then_default_pkg_fmt(world: &mut BinstallWorld, expected: String) -> Result<(), String> {
    assert_binstall_value_equals(world, "pkg-fmt", &expected, "pkg-fmt")
}

#[then("the x86_64-pc-windows-msvc override has pkg-fmt \"{expected}\"")]
fn then_windows_override_pkg_fmt(
    world: &mut BinstallWorld,
    expected: String,
) -> Result<(), String> {
    let binstall = binstall_table(world)?;
    let windows = binstall
        .get("overrides")
        .and_then(|o| o.get(WINDOWS_OVERRIDE_TARGET))
        .and_then(|w| w.as_table())
        .ok_or_else(|| String::from("Windows override not found"))?;
    let pkg_fmt = windows
        .get("pkg-fmt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| String::from("pkg-fmt not found in Windows override"))?;
    if pkg_fmt != expected {
        return Err(format!(
            "Windows override pkg-fmt mismatch: expected {expected}, got {pkg_fmt}"
        ));
    }
    Ok(())
}

#[then("the URL ends with \"{suffix}\"")]
fn then_url_ends_with(world: &mut BinstallWorld, suffix: String) {
    let url = &world.expanded_url;
    assert!(
        url.ends_with(&suffix),
        "expected URL to end with '{suffix}', got '{url}'"
    );
}

#[then("the URL contains the target triple")]
fn then_url_contains_target(world: &mut BinstallWorld) {
    let url = &world.expanded_url;
    let target = &world.target;
    assert!(
        url.contains(target.as_str()),
        "expected URL to contain '{target}', got '{url}'"
    );
}

#[then("the path ends with \"{suffix}\"")]
fn then_path_ends_with(world: &mut BinstallWorld, suffix: String) {
    let path = &world.expanded_bin_dir;
    assert!(
        path.ends_with(&suffix),
        "expected path to end with '{suffix}', got '{path}'"
    );
}

#[then("no templates contain the placeholder \"{placeholder}\"")]
fn then_no_invalid_placeholder(
    world: &mut BinstallWorld,
    placeholder: String,
) -> Result<(), String> {
    let binstall = binstall_table(world)?;
    let pkg_url = table_str(binstall, "pkg-url")?;
    let bin_dir = table_str(binstall, "bin-dir")?;
    if pkg_url.contains(&placeholder) {
        return Err(format!(
            "pkg-url contains invalid placeholder '{placeholder}'"
        ));
    }
    if bin_dir.contains(&placeholder) {
        return Err(format!(
            "bin-dir contains invalid placeholder '{placeholder}'"
        ));
    }
    Ok(())
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
