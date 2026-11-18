//! Tests for the installation lock.

use std::path::PathBuf;

use tempfile::TempDir;

use super::*;

/// Directories whose bootstrap location the test controls.
struct TestDirs {
    /// The data root the bootstrap directory is derived from.
    root: PathBuf,
    /// Whether [`BaseDirs::whitaker_data`] resolves at all.
    has_data: bool,
}

impl TestDirs {
    /// Resolve a data root inside `dir`, or report no data directory at all.
    fn new(dir: &TempDir, has_data: bool) -> Self {
        Self {
            root: dir.path().to_path_buf(),
            has_data,
        }
    }

    /// The bootstrap directory this test resolves.
    fn bootstrap(&self) -> PathBuf { self.root.join("whitaker-bootstrap") }
}

impl BaseDirs for TestDirs {
    fn home(&self) -> Option<PathBuf> { Some(self.root.clone()) }

    fn executables(&self) -> Option<PathBuf> { Some(self.root.join("bin")) }

    fn whitaker_data(&self) -> Option<PathBuf> { self.has_data.then(|| self.root.join("whitaker")) }
}

#[test]
fn runs_the_closure_under_the_lock_and_reports_its_value() {
    let dir = TempDir::new().expect("create the temporary directory");
    let dirs = TestDirs::new(&dir, true);

    let answer = with_install_lock(&dirs, || 42).expect("take the lock");

    assert_eq!(answer, 42, "the closure's value must come back unchanged");
}

#[test]
fn creates_the_lock_file_in_the_bootstrap_directory() {
    let dir = TempDir::new().expect("create the temporary directory");
    let dirs = TestDirs::new(&dir, true);
    let expected = dirs.bootstrap().join("install.lock");

    with_install_lock(&dirs, || ()).expect("take the lock");

    assert!(
        expected.is_file(),
        "expected the lock file at {}",
        expected.display()
    );
}

#[test]
fn the_lock_is_released_when_the_closure_returns() {
    let dir = TempDir::new().expect("create the temporary directory");
    let dirs = TestDirs::new(&dir, true);

    with_install_lock(&dirs, || ()).expect("take the lock once");
    // The second acquisition can only succeed if the first released the lock,
    // and it does so synchronously, so no waiting is needed.
    with_install_lock(&dirs, || ()).expect("take the lock twice");
}

#[test]
fn the_lock_is_released_when_the_closure_panics() {
    let dir = TempDir::new().expect("create the temporary directory");
    let dirs = TestDirs::new(&dir, true);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_install_lock(&dirs, || panic!("repair panicked")).expect("take the lock");
    }));

    assert!(panicked.is_err(), "the closure's panic must propagate");
    with_install_lock(&dirs, || ()).expect("the lock must have been released");
}

#[test]
fn reports_a_missing_data_directory_as_the_bootstrap_error() {
    let dir = TempDir::new().expect("create the temporary directory");
    let dirs = TestDirs::new(&dir, false);
    let mut ran = false;

    let error = with_install_lock(&dirs, || ran = true).expect_err("no data directory");

    assert!(
        matches!(
            error,
            InstallerError::Bootstrap(crate::bootstrap::BootstrapError::NoDataDirectory)
        ),
        "expected the bootstrap's NoDataDirectory, got {error}"
    );
    assert!(!ran, "the closure must not run without the lock");
}

#[test]
fn reports_a_bootstrap_directory_that_cannot_be_created() {
    let dir = TempDir::new().expect("create the temporary directory");
    let dirs = TestDirs::new(&dir, true);
    // A regular file where the bootstrap directory should be makes
    // `create_dir_all` fail with "not a directory" on every platform.
    std::fs::write(dirs.bootstrap(), "occupied").expect("write the blocking file");
    let mut ran = false;

    let error = with_install_lock(&dirs, || ran = true).expect_err("the lock cannot be created");

    assert!(
        matches!(
            error,
            InstallerError::InstallLock { ref path, .. } if path.ends_with("install.lock")
        ),
        "expected an InstallLock error naming the lock file, got {error}"
    );
    assert!(!ran, "the closure must not run without the lock");
}

#[test]
fn holds_the_lock_for_as_long_as_the_closure_runs() {
    use fs2::FileExt as _;

    let dir = TempDir::new().expect("create the temporary directory");
    let dirs = TestDirs::new(&dir, true);
    let lock_path = dirs.bootstrap().join("install.lock");

    with_install_lock(&dirs, || {
        // A second open file description contends for the same advisory lock,
        // so this probe succeeds only if the closure is running unlocked.
        let probe = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .expect("open the lock file alongside the holder");
        let contended = probe.try_lock_exclusive().is_err();

        assert!(
            contended,
            "the lock must be held while the closure runs; {} was free",
            lock_path.display()
        );
    })
    .expect("take the lock");
}
