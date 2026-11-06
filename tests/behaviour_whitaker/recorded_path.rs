//! Tests recorded toolchain path forwarding from outside a workspace.

use std::path::PathBuf;

use super::support::BootstrapFixture;

/// Every recorded staging directory reaches Dylint, so a project outside a
/// Whitaker checkout can still select lints by the toolchain it declares.
#[test]
fn forwarding_uses_all_recorded_custom_paths_outside_a_checkout() {
    let fixture = BootstrapFixture::new().expect("fixture should be installed");
    let (stable, nightly) =
        record_two_toolchains(&fixture).expect("two toolchains should be recorded");
    let external_cwd = fixture.root().join("external-project");
    std::fs::create_dir_all(&external_cwd).expect("external project directory should be created");

    let output = fixture
        .command()
        .expect("fixture command should be built")
        .arg("--all")
        .current_dir(&external_cwd)
        .output()
        .expect("root CLI should start outside a Whitaker checkout");

    assert_eq!(
        output.status.code(),
        Some(23),
        "the child status must survive"
    );
    let path_list = fixture
        .library_path()
        .expect("the stub cargo should receive a library path");
    let forwarded: Vec<PathBuf> = std::env::split_paths(&path_list).collect();
    assert!(
        forwarded.contains(&stable),
        "stable path missing: {path_list}"
    );
    assert!(
        forwarded.contains(&nightly),
        "nightly path missing: {path_list}"
    );
    assert!(!external_cwd.join("rust-toolchain.toml").exists());
}

/// Record two toolchains so the assertion covers a multi-toolchain list.
///
/// # Errors
///
/// Returns a description of the first fixture step that failed.
fn record_two_toolchains(fixture: &BootstrapFixture) -> Result<(PathBuf, PathBuf), String> {
    let stable = staging_directory(fixture, "custom-stable")?;
    let nightly = staging_directory(fixture, "custom-nightly")?;
    for (toolchain, path) in [("stable", &stable), ("nightly", &nightly)] {
        let staging = camino::Utf8Path::from_path(path).ok_or_else(|| {
            format!(
                "fixture staging path should be valid UTF-8: {}",
                path.display()
            )
        })?;
        fixture.record_toolchain(toolchain, staging)?;
    }
    Ok((stable, nightly))
}

/// Create an existing staging directory, as a completed install would leave it.
///
/// The install record stores canonical paths, so the fixture returns the
/// canonical form too. On macOS the temporary directory resolves from `/var`
/// to `/private/var`, and comparing the unresolved path would fail the
/// forwarding assertion for a reason unrelated to the behaviour under test.
///
/// # Errors
///
/// Returns an error when the directory cannot be created or resolved.
fn staging_directory(fixture: &BootstrapFixture, name: &str) -> Result<PathBuf, String> {
    let path = fixture.root().join(name);
    std::fs::create_dir_all(&path)
        .map_err(|error| format!("staging directory should be created: {error}"))?;
    path.canonicalize()
        .map_err(|error| format!("staging directory should be resolved: {error}"))
}
