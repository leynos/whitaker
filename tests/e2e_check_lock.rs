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
//! readiness, and waits. The parent starts the check, watches until the check
//! holds an open descriptor on the lock file — which is what a process blocked
//! acquiring the lock looks like — and only then releases the holder to write
//! the installation record. The check's first preflight therefore sees an
//! unprovisioned machine and its in-lock preflight sees a provisioned one,
//! and the test knows that ordering held rather than inferring it from how
//! long the child happened to run.
//!
//! The handshake reads `/proc/<pid>/fd`, so it is Linux-only. The repository's
//! hosted lanes run this suite on Linux and Windows; macOS appears only in
//! release build workflows, so gating here loses no coverage.

#![cfg(target_os = "linux")]

use std::{
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
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

/// The variable naming the file whose appearance releases the holder.
const HOLDER_RELEASE: &str = "WHITAKER_CHECK_LOCK_RELEASE";

/// How long a coordination step may take before the test reports a failure.
///
/// The waits below are bounded so a check that never reaches the lock, or a
/// holder that is never released, fails with a diagnosis instead of hanging
/// the suite.
const COORDINATION: Duration = Duration::from_secs(30);

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

/// Name the lock file the installation takes for a given data root.
///
/// It lives in the same bootstrap directory as the record; taking it is what
/// creates the directory, which is why the holder can install the record
/// afterwards without creating it first.
fn lock_path(data_root: &Path) -> Option<PathBuf> {
    Some(record_path(data_root)?.with_file_name("install.lock"))
}

/// Provision the machine while holding the installation lock.
///
/// It is ignored so a normal run never executes it; the parent starts it as a
/// separate process with the `HOLDER_*` variables set. The record is written
/// only after the parent releases the holder, which is what makes the
/// parent's waiting check observe an unprovisioned machine first and a
/// provisioned one second.
#[test]
#[ignore = "started as a separate process by the lock-wait test"]
fn lock_holder_child() {
    let (Some(data_root), Some(record), Some(ready), Some(release)) = (
        std::env::var(HOLDER_DATA_ROOT).ok(),
        std::env::var(HOLDER_RECORD).ok(),
        std::env::var(HOLDER_READY).ok(),
        std::env::var(HOLDER_RELEASE).ok(),
    ) else {
        return;
    };
    let dirs = HolderDirs {
        data_root: PathBuf::from(data_root),
    };
    // Taking the lock creates the bootstrap directory, which is also where the
    // record belongs, so the copy below needs no directory of its own.
    let target =
        record_path(&dirs.data_root).expect("the holder's project-data directory has a parent");

    with_install_lock(&dirs, || {
        std::fs::write(&ready, b"").expect("the ready marker is written while the lock is held");
        wait_until_exists(Path::new(&release), COORDINATION)
            .expect("the waiting check should reach the lock before the record is published");
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

/// Why a check never reached the installation lock.
enum NotWaiting {
    /// The check exited before it could wait.
    Exited(Output),
    /// The check's own status could not be read.
    Status(io::Error),
    /// The check was still running but held no lock descriptor in time.
    TimedOut(Child),
}

impl NotWaiting {
    /// Describe the failed handshake, releasing any child it left running.
    ///
    /// Taking `self` is what lets the timeout case reap the check it gave up
    /// on: the caller is about to fail the test, so the diagnosis must not
    /// leave a process behind.
    fn report(self, lock: &Path) -> String {
        match self {
            Self::Exited(output) => format!(
                "the check should wait for the lock, but it exited with {:?}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ),
            Self::Status(error) => format!("the check's status could not be read: {error}"),
            Self::TimedOut(mut child) => {
                stop(&mut child);
                format!(
                    "the check should reach the installation lock within {COORDINATION:?}, at {}",
                    lock.display()
                )
            }
        }
    }
}

/// Stop a child whose handshake ended in a failure being reported.
///
/// The caller is about to fail the test, so a kill that cannot be delivered —
/// because the child has already exited — changes nothing. The attempt still
/// matters: a surviving child would keep a temporary directory alive after
/// the test drops it.
fn stop(child: &mut Child) {
    // An error here means there was no process left to stop.
    child.kill().unwrap_or(());
}

/// Wait until `child` holds an open descriptor on `lock`.
///
/// A process blocked acquiring the installation lock has already opened the
/// lock file and keeps the descriptor open until the attempt returns, so the
/// descriptor's appearance proves the check reached the lock. That is evidence
/// of contention itself, unlike a threshold on the child's total runtime,
/// which a slow start can satisfy without any lock involvement.
fn wait_until_lock_is_held(
    mut child: Child,
    lock: &Path,
    timeout: Duration,
) -> Result<Child, NotWaiting> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        match child.try_wait() {
            // `try_wait` reaps an exited child, so `wait_with_output` returns
            // the status it cached along with the piped streams.
            Ok(Some(_)) => return Err(exit_of(child)),
            Ok(None) => {}
            Err(error) => return Err(NotWaiting::Status(error)),
        }
        if holds_lock_descriptor(child.id(), lock) {
            return Ok(child);
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(NotWaiting::TimedOut(child))
}

/// Turn an exited check into the failure that reports its output.
fn exit_of(child: Child) -> NotWaiting {
    match child.wait_with_output() {
        Ok(output) => NotWaiting::Exited(output),
        Err(error) => NotWaiting::Status(error),
    }
}

/// Report whether `pid` holds an open descriptor on `lock`.
fn holds_lock_descriptor(pid: u32, lock: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return false;
    };
    let Some(expected) = std::fs::canonicalize(lock).ok() else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        std::fs::read_link(entry.path())
            .ok()
            .and_then(|target| std::fs::canonicalize(target).ok())
            .is_some_and(|target| target == expected)
    })
}

/// Assert that the holder provisioned and the check linted without repairing.
fn assert_linted_without_repair(holder: &Output, check: &Output) {
    assert!(
        holder.status.success(),
        "the holder should provision and exit successfully, stderr: {}",
        String::from_utf8_lossy(&holder.stderr)
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert_eq!(
        check.status.code(),
        Some(23),
        "the check should lint once the record appeared, stderr: {stderr}"
    );
    assert!(
        !stderr.contains(REPAIR_NOTICE_PHRASE),
        "a check that found the installation ready must not install again: {stderr}"
    );
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
    let data_root = destination.data_root();
    let marker = tempfile::TempDir::new().expect("a marker directory should be created");
    let ready = marker.path().join("lock-held");
    let release = marker.path().join("release");
    let parked_record = marker.path().join("bootstrap.json");
    let destination_record = record_path(&data_root).expect("the destination has a parent");
    let lock = lock_path(&data_root).expect("the destination has a lock path");
    std::fs::rename(destination_record, &parked_record)
        .expect("the destination record should be parked");

    let mut holder = Command::new(std::env::current_exe().expect("the test binary is known"))
        .args(["--exact", "lock_holder_child", "--ignored"])
        .env(HOLDER_DATA_ROOT, &data_root)
        .env(HOLDER_RECORD, &parked_record)
        .env(HOLDER_READY, &ready)
        .env(HOLDER_RELEASE, &release)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the holder process should start");
    wait_until_exists(&ready, COORDINATION).expect("the holder should take the lock");

    let waiting = destination
        .command()
        .expect("the destination command should build")
        .arg("check")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the waiting check should start");
    let check = match wait_until_lock_is_held(waiting, &lock, COORDINATION) {
        Ok(check) => check,
        // Stop the holder before reporting, so a failed handshake does not
        // leave a process holding a temporary directory the test is dropping.
        Err(failure) => {
            stop(&mut holder);
            panic!("{}", failure.report(&lock));
        }
    };
    std::fs::write(&release, b"").expect("the release marker is written once the check waits");

    let check_output = check
        .wait_with_output()
        .expect("the waiting check should be waited for");
    let holder_output = holder
        .wait_with_output()
        .expect("the holder should be waited for");

    assert_linted_without_repair(&holder_output, &check_output);
}
