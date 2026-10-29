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
    let dir = root.join("nightly-2026-05-28");
    std::fs::create_dir_all(&dir).expect("create toolchain dir");
    std::fs::write(dir.join("dylint-driver.exe"), b"driver").expect("write driver");

    assert!(
        !slot.has_driver().expect("inspect the slot"),
        "Dylint never persists `dylint-driver.exe`"
    );
}

#[test]
fn a_driver_without_a_marker_is_cold() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    write_driver(&root, "nightly-2026-05-28", b"driver");

    assert!(slot.has_driver().expect("inspect the slot"));
    assert!(
        !slot.is_ready().expect("inspect the slot"),
        "an unmarked driver may be partial or foreign"
    );

    slot.mark_ready().expect("mark ready");
    assert!(slot.is_ready().expect("inspect the slot"));
}

#[rstest]
#[case::other_channel("nightly-2026-06-11", b"driver".as_slice())]
#[case::empty_file("nightly-2026-05-28", b"".as_slice())]
fn ignores_drivers_that_are_not_this_toolchains(#[case] dir: &str, #[case] contents: &[u8]) {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    write_driver(&root, dir, contents);

    assert!(!slot.has_driver().expect("inspect the slot"));
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

#[test]
fn a_slot_that_does_not_exist_yet_is_cold() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root.join("absent"), "nightly-2026-05-28");

    assert!(!slot.has_driver().expect("absence is not an error"));
    assert!(!slot.is_ready().expect("absence is not an error"));
}

/// Regression: an unreadable slot once read as cold, so a cold build would
/// start against a slot it could not use. A NUL byte makes every platform
/// refuse the path with `InvalidInput`; a file standing in for the slot would
/// not, because Windows reports a path through a file as `NotFound`.
#[test]
fn a_slot_that_cannot_be_read_is_an_error() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root.join("uninspectable\0slot"), "nightly-2026-05-28");

    let error = slot
        .has_driver()
        .expect_err("an uninspectable slot is an error");

    assert!(matches!(error, BootstrapError::Io { .. }), "{error:?}");
}

/// Dylint 6.3.0 keys the driver by the toolchain in the library file names,
/// which is the bare channel, so only that directory is its driver.
#[rstest]
#[case::bare_channel("nightly-2026-05-28", true)]
#[case::host_suffixed("nightly-2026-05-28-x86_64-unknown-linux-gnu", false)]
#[case::later_channel_sharing_a_prefix("nightly-2026-05-280", false)]
fn recognizes_only_the_toolchain_directory_dylint_selects(
    #[case] dir: &str,
    #[case] expected: bool,
) {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    write_driver(&root, dir, b"driver");

    assert_eq!(slot.has_driver().expect("inspect the slot"), expected);
}

#[test]
fn a_slot_without_a_driver_has_no_identity() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");

    assert_eq!(slot.driver_identity().expect("inspect the slot"), None);
}

#[test]
fn an_untouched_driver_keeps_its_identity() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    write_driver(&root, "nightly-2026-05-28", b"driver");

    let first = slot.driver_identity().expect("inspect the slot");

    assert!(first.is_some());
    assert_eq!(slot.driver_identity().expect("inspect again"), first);
}

/// Dylint persists a rebuilt driver by renaming a new file into place, which
/// must register as a change even when the bytes are identical.
#[test]
fn a_driver_replaced_with_identical_bytes_has_a_new_identity() {
    let (_dir, root) = temp_root();
    let slot = DriverSlot::new(&root, "nightly-2026-05-28");
    let toolchain_dir = "nightly-2026-05-28";
    write_driver(&root, toolchain_dir, b"driver");
    let before = slot.driver_identity().expect("inspect the slot");
    // Keep the original alive so the replacement cannot reuse its inode.
    let original = root.join(toolchain_dir).join("dylint-driver.original");
    std::fs::hard_link(root.join(toolchain_dir).join("dylint-driver"), &original)
        .expect("keep the original");
    // Creation times have 100 ns resolution on Windows; make the gap certain.
    std::thread::sleep(std::time::Duration::from_millis(20));
    let partial = root.join(toolchain_dir).join("dylint-driver.partial");
    std::fs::write(&partial, b"driver").expect("write the replacement");
    std::fs::rename(&partial, root.join(toolchain_dir).join("dylint-driver"))
        .expect("persist the replacement");

    assert_ne!(slot.driver_identity().expect("inspect again"), before);
}
