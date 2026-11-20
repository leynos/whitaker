//! End-to-end proof that a check which waits for the installation lock
//! re-checks readiness instead of installing again.
//!
//! The scenario is H-6 in the `ExecPlan`: one process provisions the machine
//! while a second `check` is already waiting on `install.lock`. The waiting
//! check must acquire the lock, find the installation ready, and run Dylint —
//! not repeat the installation. Two processes are used because the claim is
//! about contention between processes; the in-process equivalent cannot
//! produce it.
//!
//! The holder is this test binary re-invoked in an ignored mode, the pattern
//! the installer's own two-process lock test uses. It takes the lock, signals
//! readiness, waits, and only then writes the installation record, so the
//! waiting check's first preflight sees an unprovisioned machine while its
//! in-lock preflight sees a provisioned one.

#![cfg(unix)]

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use whitaker_installer::{dirs::BaseDirs, orchestration::with_install_lock};

mod e2e_check_support;

/// The variable naming the data root the holder provisions.
const HOLDER_DATA_ROOT: &str = "WHITAKER_CHECK_LOCK_DATA_ROOT";

/// The variable naming the record the holder installs under the lock.
const HOLDER_RECORD: &str = "WHITAKER_CHECK_LOCK_RECORD";

/// The variable naming the file whose appearance means the lock is held.
const HOLDER_READY: &str = "WHITAKER_CHECK_LOCK_READY";

/// The variable naming how long the holder keeps the lock, in milliseconds.
const HOLDER_HOLD_MS: &str = "WHITAKER_CHECK_LOCK_HOLD_MS";

/// How long the holder keeps the lock after signalling readiness.
///
/// The waiting check starts as soon as the marker appears, so this is the
/// window in which it must reach the lock. It is generous because the
/// assertion is that the check waited at all, not how quickly it started.
const HOLD: Duration = Duration::from_secs(3);

/// The least time a blocked check can have waited and still pass.
const MINIMUM_WAIT: Duration = Duration::from_millis(1_500);

/// The phrase the repair notice carries.
///
/// The adapter's own constant is not exported to integration tests, so this
/// mirrors its wording; the assertion only needs a phrase that appears when a
/// repair starts and nowhere else.
const REPAIR_NOTICE_PHRASE: &str = "installing missing Whitaker components";

/// Directories whose bootstrap location the holder controls.
struct HolderDirs {
    /// The project-data directory the bootstrap directory sits beside.
    data_root: PathBuf,
}

impl BaseDirs for HolderDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data_root.clone()) }
}

/// Name the installation record the binary reads for a given data root.
///
/// The bootstrap directory sits beside the project-data directory, never
/// inside it, so the record is one level up and next door. Deriving it here
/// keeps the shared fixture to the API every end-to-end target uses.
///
/// Returns `None` when the data root has no parent, which a fixture directory
/// never is; callers inside a test decide how to report that.
fn record_path(data_root: &Path) -> Option<PathBuf> {
    Some(
        data_root
            .parent()?
            .join("whitaker-bootstrap")
            .join("bootstrap.json"),
    )
}

/// Provision the machine while holding the installation lock.
///
/// It is ignored so a normal run never executes it; the parent starts it as a
/// separate process with the `HOLDER_*` variables set. The record is written
/// only after the wait, which is what makes the parent's waiting check observe
/// an unprovisioned machine first and a provisioned one second.
#[test]
#[ignore = "started as a separate process by the lock-wait test"]
fn lock_holder_child() {
    let (Some(data_root), Some(record), Some(ready), Some(hold_ms)) = (
        std::env::var(HOLDER_DATA_ROOT).ok(),
        std::env::var(HOLDER_RECORD).ok(),
        std::env::var(HOLDER_READY).ok(),
        std::env::var(HOLDER_HOLD_MS).ok(),
    ) else {
        return;
    };
    let hold = Duration::from_millis(hold_ms.parse().expect("the hold should be milliseconds"));
    let dirs = HolderDirs {
        data_root: PathBuf::from(data_root),
    };
    // Taking the lock creates the bootstrap directory, which is also where the
    // record belongs, so the copy below needs no directory of its own.
    let target =
        record_path(&dirs.data_root).expect("the holder's project-data directory has a parent");

    with_install_lock(&dirs, || {
        std::fs::write(&ready, b"").expect("the ready marker is written while the lock is held");
        thread::sleep(hold);
        std::fs::copy(&record, &target)
            .expect("the record is installed before the lock is released");
    })
    .expect("the holder takes the installation lock");
}

/// Wait until `path` exists, or report that it never appeared.
fn wait_until_exists(path: &Path, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!(
        "{} did not appear within {timeout:?}",
        path.display()
    ))
}

/// Report whether a waiting check re-checks readiness instead of installing.
///
/// The fixture is fully provisioned except for its record, which is parked
/// outside the installation so a repair can converge by putting it back. That
/// is what a concurrent install would do, and only the fixture's own record
/// will do: it names the snapshot generation this installation actually holds.
/// The check must wait for the lock, find the record, skip the install, and
/// pass the stub's status through. Its stderr must not carry the repair
/// notice, which is what an install attempt would print.
#[test]
fn a_waiting_check_finds_the_installation_ready() {
    let destination = e2e_check_support::BootstrapFixture::new()
        .expect("the destination installation should be created");
    let marker = tempfile::TempDir::new().expect("a marker directory should be created");
    let ready = marker.path().join("lock-held");
    let parked_record = marker.path().join("bootstrap.json");
    let destination_record =
        record_path(&destination.data_root()).expect("the destination has a parent");
    std::fs::rename(destination_record, &parked_record)
        .expect("the destination record should be parked");

    let holder = Command::new(std::env::current_exe().expect("the test binary is known"))
        .args(["--exact", "lock_holder_child", "--ignored"])
        .env(HOLDER_DATA_ROOT, destination.data_root())
        .env(HOLDER_RECORD, &parked_record)
        .env(HOLDER_READY, &ready)
        .env(HOLDER_HOLD_MS, HOLD.as_millis().to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the holder process should start");
    wait_until_exists(&ready, Duration::from_secs(10)).expect("the holder should take the lock");

    let started = Instant::now();
    let output = destination
        .command()
        .expect("the destination command should build")
        .arg("check")
        .output()
        .expect("the waiting check should start");
    let waited = started.elapsed();
    let holder_output = holder
        .wait_with_output()
        .expect("the holder should be waited for");

    assert!(
        holder_output.status.success(),
        "the holder should provision and exit successfully, stderr: {}",
        String::from_utf8_lossy(&holder_output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(23),
        "the check should lint once the record appeared, stderr: {stderr}"
    );
    assert!(
        waited >= MINIMUM_WAIT,
        "the check must have waited for the lock rather than preflighting a ready machine, took \
         {waited:?}"
    );
    assert!(
        !stderr.contains(REPAIR_NOTICE_PHRASE),
        "a check that found the installation ready must not install again: {stderr}"
    );
}
