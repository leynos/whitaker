//! Tests for the installation lock.

use std::path::PathBuf;

use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;

/// Directories whose bootstrap location the test controls.
struct TestDirs {
    /// Keeps the temporary data root alive for as long as the resolved paths.
    _temp: TempDir,
    /// The data root the bootstrap directory is derived from.
    root: PathBuf,
    /// Whether [`BaseDirs::whitaker_data`] resolves at all.
    has_data: bool,
}

impl TestDirs {
    /// Take ownership of a temporary directory and resolve `has_data` inside it.
    fn new(temp: TempDir, has_data: bool) -> Self {
        let root = temp.path().to_path_buf();
        Self {
            _temp: temp,
            root,
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

/// Provide a fresh data root whose bootstrap directory resolves.
#[fixture]
fn test_dirs() -> TestDirs {
    TestDirs::new(
        TempDir::new().expect("create the temporary directory"),
        true,
    )
}

/// Provide a fresh data root with no data directory, keeping the missing-data
/// case distinct from the resolvable [`test_dirs`] fixture.
#[fixture]
fn missing_data_dirs() -> TestDirs {
    TestDirs::new(
        TempDir::new().expect("create the temporary directory"),
        false,
    )
}

/// Open a second handle on the lock file for a non-blocking lock probe.
fn probe_handle(path: &std::path::Path) -> std::fs::File {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("open the lock file alongside its holder")
}

/// Report whether `error` is the platform's lock-contention error.
///
/// The kind and the raw OS code are compared together so that a transient I/O
/// failure reaches the caller as itself rather than being mistaken for a held
/// lock.
fn is_lock_contended(error: &std::io::Error) -> bool {
    let contended = fs2::lock_contended_error();
    error.kind() == contended.kind() && error.raw_os_error() == contended.raw_os_error()
}

/// Read a panic payload's message as text.
///
/// A formatted panic carries a [`String`]; a literal panic carries a
/// `&'static str`. Anything else is described as a placeholder rather than
/// accepted as the closure's own message.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("<non-string panic payload>")
}

/// Run the closure under the lock and return its value unchanged.
#[rstest]
fn runs_the_closure_under_the_lock_and_reports_its_value(test_dirs: TestDirs) {
    let answer = with_install_lock(&test_dirs, || 42).expect("take the lock");

    assert_eq!(answer, 42, "the closure's value must come back unchanged");
}

/// Create the lock file in the bootstrap directory on first use.
#[rstest]
fn creates_the_lock_file_in_the_bootstrap_directory(test_dirs: TestDirs) {
    let expected = test_dirs.bootstrap().join("install.lock");

    with_install_lock(&test_dirs, || ()).expect("take the lock");

    assert!(
        expected.is_file(),
        "expected the lock file at {}",
        expected.display()
    );
}

/// Release the lock when the closure returns normally.
#[rstest]
fn the_lock_is_released_when_the_closure_returns(test_dirs: TestDirs) {
    let lock_path = test_dirs.bootstrap().join("install.lock");

    with_install_lock(&test_dirs, || ()).expect("take the lock once");

    // A retained handle fails this non-blocking probe. A second
    // `with_install_lock` call would instead block on the very lock it is
    // meant to prove released.
    let probe = probe_handle(&lock_path);
    assert!(
        fs2::FileExt::try_lock_exclusive(&probe).is_ok(),
        "the lock at {} must be released when the closure returns",
        lock_path.display()
    );
}

/// Release the lock when the closure unwinds.
#[rstest]
fn the_lock_is_released_when_the_closure_panics(test_dirs: TestDirs) {
    let lock_path = test_dirs.bootstrap().join("install.lock");
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_install_lock(&test_dirs, || panic!("repair panicked")).expect("take the lock");
    }));
    let payload = panicked.expect_err("the closure's panic must propagate");

    // A lock-acquisition failure would also make `catch_unwind` return an
    // error, so identify the closure's own payload rather than any panic.
    assert_eq!(
        panic_message(payload.as_ref()),
        "repair panicked",
        "the observed panic must come from the repair closure, not from taking the lock"
    );

    // A retained handle fails this non-blocking probe. A second
    // `with_install_lock` call would instead block on the very lock it is
    // meant to prove released.
    let probe = probe_handle(&lock_path);
    assert!(
        fs2::FileExt::try_lock_exclusive(&probe).is_ok(),
        "the lock at {} must be released when the closure unwinds",
        lock_path.display()
    );
}

/// Report a missing data directory as the bootstrap error without running the
/// closure.
#[rstest]
fn reports_a_missing_data_directory_as_the_bootstrap_error(missing_data_dirs: TestDirs) {
    let mut ran = false;

    let error =
        with_install_lock(&missing_data_dirs, || ran = true).expect_err("no data directory");

    assert!(
        matches!(
            error,
            InstallerError::Bootstrap(crate::bootstrap::BootstrapError::NoDataDirectory)
        ),
        "expected the bootstrap's NoDataDirectory, got {error}"
    );
    assert!(!ran, "the closure must not run without the lock");
}

/// Report a bootstrap directory that cannot be created without running the
/// closure.
#[rstest]
fn reports_a_bootstrap_directory_that_cannot_be_created(test_dirs: TestDirs) {
    // A regular file where the bootstrap directory should be makes
    // `create_dir_all` fail with "not a directory" on every platform.
    std::fs::write(test_dirs.bootstrap(), "occupied").expect("write the blocking file");
    let mut ran = false;

    let error =
        with_install_lock(&test_dirs, || ran = true).expect_err("the lock cannot be created");

    assert!(
        matches!(
            error,
            InstallerError::InstallLock { ref path, .. } if path.ends_with("install.lock")
        ),
        "expected an InstallLock error naming the lock file, got {error}"
    );
    assert!(!ran, "the closure must not run without the lock");
}

/// The variable naming the lock file a probe child should inspect.
const PROBE_PATH: &str = "WHITAKER_LOCK_PROBE";

/// The variable naming the file a probe child should write its verdict to.
const PROBE_VERDICT: &str = "WHITAKER_LOCK_VERDICT";

/// Report whether the lock is free from a separate process.
///
/// It is ignored so a normal run never executes it; the parent starts it as a
/// separate process with [`PROBE_PATH`] and [`PROBE_VERDICT`] set. The verdict
/// travels through a file because the process policy denies printing, and
/// because a file cannot be confused with the libtest harness's own output.
#[test]
#[ignore = "started as a separate process by the two-process lock test"]
fn process_lock_probe() {
    let (Ok(path), Ok(verdict_path)) = (std::env::var(PROBE_PATH), std::env::var(PROBE_VERDICT))
    else {
        return;
    };
    let probe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("the probe opens the lock file the parent created");
    let verdict = match fs2::FileExt::try_lock_exclusive(&probe) {
        Ok(()) => "free",
        Err(error) if is_lock_contended(&error) => "contended",
        Err(error) => panic!("the probe's lock attempt failed unexpectedly: {error}"),
    };
    std::fs::write(&verdict_path, verdict).expect("the probe writes its verdict");
}

/// Ask another process whether it can take the lock.
///
/// `verdict_path` must not exist yet: its appearance is what proves the child
/// was selected and ran.
///
/// # Errors
///
/// Returns the description of a probe that could not be started, did not exit
/// successfully, or reported no verdict.
fn probe_from_another_process(
    lock_path: &std::path::Path,
    verdict_path: &std::path::Path,
) -> std::result::Result<String, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("the test binary is unknown: {error}"))?;
    let output = std::process::Command::new(executable)
        .args([
            "--exact",
            "orchestration::lock::tests::process_lock_probe",
            "--ignored",
        ])
        .env(PROBE_PATH, lock_path)
        .env(PROBE_VERDICT, verdict_path)
        .output()
        .map_err(|error| format!("the probe process should start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "the probe process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    std::fs::read_to_string(verdict_path)
        .map(|verdict| verdict.trim().to_owned())
        .map_err(|error| format!("the probe reported no verdict: {error}"))
}

/// Exclude another process while the lock is held and admit it after release.
#[rstest]
fn another_process_is_excluded_while_the_lock_is_held(test_dirs: TestDirs) {
    let verdicts = TempDir::new().expect("create the verdict directory");
    let lock_path = test_dirs.bootstrap().join("install.lock");
    let while_held_path = verdicts.path().join("while-held");
    let after_release_path = verdicts.path().join("after-release");

    let while_held = with_install_lock(&test_dirs, || {
        probe_from_another_process(&lock_path, &while_held_path)
    })
    .expect("take the lock")
    .expect("the probe should report a verdict");
    let after_release = probe_from_another_process(&lock_path, &after_release_path)
        .expect("probe again once the closure has returned");

    assert_eq!(
        while_held, "contended",
        "another process must wait while the installation lock is held"
    );
    assert_eq!(
        after_release, "free",
        "the lock must be released when the closure returns, for other processes too"
    );
}

/// Hold the lock for as long as the closure runs.
#[rstest]
fn holds_the_lock_for_as_long_as_the_closure_runs(test_dirs: TestDirs) {
    let lock_path = test_dirs.bootstrap().join("install.lock");

    with_install_lock(&test_dirs, || {
        // A second open file description contends for the same advisory lock,
        // so this probe fails with contention only if the closure is running
        // while the lock is held.
        let probe = probe_handle(&lock_path);
        let contended = match fs2::FileExt::try_lock_exclusive(&probe) {
            Ok(()) => false,
            Err(error) if is_lock_contended(&error) => true,
            Err(error) => panic!("the probe's lock attempt failed unexpectedly: {error}"),
        };

        assert!(
            contended,
            "the lock must be held while the closure runs; {} was free",
            lock_path.display()
        );
    })
    .expect("take the lock");
}
