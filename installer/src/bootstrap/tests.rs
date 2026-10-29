//! Layout, metadata, and driver-slot behaviour of the installed bootstrap.

use super::*;
use rstest::rstest;
use tempfile::TempDir;

/// A temporary directory viewed as UTF-8.
fn temp_root() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().expect("create temp dir");
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8 temp path");
    (dir, path)
}

fn metadata(library_dir: StoredPath) -> BootstrapMetadata {
    BootstrapMetadata {
        schema_version: BOOTSTRAP_SCHEMA,
        dylint_version: "6.3.0".to_owned(),
        snapshot_commit: "e4608a25c2e20116fbc1e97a51a73f094a1db7a1".to_owned(),
        toolchain: "nightly-2026-05-28".to_owned(),
        library_dir,
    }
}

#[test]
fn layout_keys_drivers_by_dylint_release_and_snapshot() {
    let layout = BootstrapLayout::at(Utf8Path::new("/data/whitaker-bootstrap"));

    assert_ne!(
        layout.driver_dir("6.3.0", "aaaa"),
        layout.driver_dir("6.4.0", "aaaa"),
        "a Dylint upgrade selects a new slot"
    );
    assert_ne!(
        layout.driver_dir("6.3.0", "aaaa"),
        layout.driver_dir("6.3.0", "bbbb"),
        "a snapshot re-pin selects a new slot"
    );
    assert_eq!(
        layout.snapshot("aaaa"),
        Utf8Path::new("/data/whitaker-bootstrap/clippy/aaaa")
    );
}

#[rstest]
#[case::inside("/data/whitaker/lints/lib", StoredPath::DataRootRelative("whitaker/lints/lib".into()))]
#[case::outside("/work/target/dylint/lib", StoredPath::Absolute("/work/target/dylint/lib".into()))]
fn stores_paths_relative_to_the_data_root_only_when_inside_it(
    #[case] path: &str,
    #[case] expected: StoredPath,
) {
    assert_eq!(
        StoredPath::new(Utf8Path::new(path), Utf8Path::new("/data")),
        expected
    );
}

#[test]
fn metadata_round_trips_and_relocates() {
    let (_dir, root) = temp_root();
    let path = root.join("whitaker-bootstrap/bootstrap.json");
    let written = metadata(StoredPath::DataRootRelative("whitaker/lints".into()));

    written.write(&path).expect("write metadata");
    let read = BootstrapMetadata::read(&path).expect("read metadata");

    assert_eq!(read, written);
    assert_eq!(
        read.library_dir.resolve(Utf8Path::new("/moved")),
        Utf8Path::new("/moved/whitaker/lints")
    );
}

#[test]
fn missing_metadata_points_at_the_installer() {
    let (_dir, root) = temp_root();
    let error = BootstrapMetadata::read(&root.join("bootstrap.json")).expect_err("absent");

    assert!(
        matches!(error, BootstrapError::NotInstalled { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("run whitaker-installer"));
}

#[test]
fn rejects_metadata_from_another_schema() {
    let (_dir, root) = temp_root();
    let path = root.join("bootstrap.json");
    let mut future = metadata(StoredPath::Absolute("/x".into()));
    future.schema_version = BOOTSTRAP_SCHEMA + 1;
    future.write(&path).expect("write metadata");

    let error = BootstrapMetadata::read(&path).expect_err("future schema is rejected");

    assert!(error.to_string().contains("is not supported"), "{error}");
}

/// Write a driver file for `toolchain_dir` inside `slot`.
fn write_driver(slot: &Utf8Path, toolchain_dir: &str, contents: &[u8]) {
    let dir = slot.join(toolchain_dir);
    std::fs::create_dir_all(&dir).expect("create toolchain dir");
    // Dylint persists the driver without an extension on every platform.
    std::fs::write(dir.join("dylint-driver"), contents).expect("write driver");
}

/// Regression: the slot once looked for `dylint-driver.exe` on Windows, where
/// Dylint writes `dylint-driver`, so a Windows driver was never marked ready.
#[test]
fn an_extension_suffixed_driver_is_not_dylints() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    let dir = root.join("nightly-2026-05-28-x86_64-pc-windows-msvc");
    std::fs::create_dir_all(&dir).expect("create toolchain dir");
    std::fs::write(dir.join("dylint-driver.exe"), b"driver").expect("write driver");

    assert!(
        !slot.has_driver(),
        "Dylint never persists `dylint-driver.exe`"
    );
}

#[test]
fn a_driver_without_a_marker_is_cold() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    write_driver(
        &root,
        "nightly-2026-05-28-x86_64-unknown-linux-gnu",
        b"driver",
    );

    assert!(slot.has_driver());
    assert!(
        !slot.is_ready(),
        "an unmarked driver may be partial or foreign"
    );

    slot.mark_ready().expect("mark ready");
    assert!(slot.is_ready());
}

#[rstest]
#[case::other_channel("nightly-2026-06-11-x86_64-unknown-linux-gnu", b"driver".as_slice())]
#[case::empty_file("nightly-2026-05-28-x86_64-unknown-linux-gnu", b"".as_slice())]
fn ignores_drivers_that_are_not_this_toolchains(#[case] dir: &str, #[case] contents: &[u8]) {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    write_driver(&root, dir, contents);

    assert!(!slot.has_driver());
}

#[test]
fn shared_locks_coexist() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");

    let first = slot.lock_shared().expect("first shared lock");
    let second = slot.lock_shared().expect("second shared lock");

    drop((first, second));
    slot.lock_exclusive()
        .expect("exclusive once shared locks are released");
}
