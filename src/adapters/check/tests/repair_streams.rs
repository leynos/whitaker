//! Which stream a production repair gives the installer's markers.
//!
//! The repair seam keeps the marker stream apart from diagnostics, and the
//! tests on that seam prove it. What they cannot prove is the stream the
//! *production* caller selects: `check` owns only diagnostics, so an installer
//! marker must never reach standard output, which belongs to whatever the user
//! pipes from `check`. That selection is a write to a process-wide descriptor,
//! so it is observed the only way it can be — by re-running this test binary
//! as a child and reading the child's own standard streams.

use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use whitaker_installer::dirs::BaseDirs;

use super::{
    super::repair::{
        HostEnv,
        REPAIR_NOTICE,
        Readiness,
        RepairRequest,
        RepairStreams,
        repair_production,
    },
    fixtures::{ScriptedSteps, temp_dirs},
};
use crate::diagnostics::DiagnosticSettings;

/// The variable naming the data root the child repairs into.
const CHILD_DATA_ROOT: &str = "WHITAKER_CHECK_REPAIR_STREAM_DATA_ROOT";

/// The machine marker the fake installer writes to its marker stream.
const INSTALLER_MARKER: &str = "whitaker-installer: marker-stream-probe";

/// The word framing the caller's diagnostics buffer in the child's output.
const DIAGNOSTICS_FRAME: &str = "diagnostics-buffer";

/// Data root the child's repair reads, named by the parent.
struct ChildDirs {
    /// The project-data directory, which holds no installation record.
    data_root: PathBuf,
}

impl BaseDirs for ChildDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data_root.clone()) }
}

/// Repair through the production selection, for the parent to observe.
///
/// Ignored so a normal run never executes it: the parent starts this binary
/// again with a filter naming this test. The marker stream is whatever
/// [`repair_production`] selects, so writing the marker there and leaving it to
/// the real descriptors is the whole observation. The caller's diagnostics
/// buffer is echoed inside a frame, so the parent can also see that the
/// repair's own notices stayed out of the marker stream.
#[test]
#[ignore = "started as a separate process by the marker-stream test"]
fn repair_production_child() {
    let Some(data_root) = std::env::var_os(CHILD_DATA_ROOT).map(PathBuf::from) else {
        return;
    };
    let dirs = ChildDirs { data_root };
    let mut steps =
        ScriptedSteps::ready_to_install(Readiness::Repairable).writing(INSTALLER_MARKER);
    let mut diagnostics = Vec::new();

    // The installer's markers reach the stream the caller supplied. The child
    // sends that stream to its own standard error, which is what the parent
    // inspects, so the markers are observable exactly as production writes
    // them.
    let mut markers = std::io::stderr();
    let outcome = repair_production(
        HostEnv {
            dirs: &dirs,
            inherited_path: None,
        },
        RepairRequest {
            settings: DiagnosticSettings::default(),
            is_ci: false,
        },
        RepairStreams {
            installer: &mut markers,
            diagnostics: &mut diagnostics,
        },
        &mut steps,
    );

    let mut stderr = std::io::stderr();
    writeln!(stderr, "begin {DIAGNOSTICS_FRAME}").expect("the frame can be opened");
    stderr
        .write_all(&diagnostics)
        .expect("the buffer can be echoed");
    writeln!(stderr, "end {DIAGNOSTICS_FRAME}").expect("the frame can be closed");
    outcome.expect("the scripted install completes");
}

/// Extract the framed diagnostics buffer from the child's standard error.
fn diagnostics_frame(stderr: &str) -> &str {
    let open = format!("begin {DIAGNOSTICS_FRAME}\n");
    let close = format!("end {DIAGNOSTICS_FRAME}\n");
    stderr
        .split_once(&open)
        .and_then(|(_, rest)| rest.split_once(&close))
        .map_or("", |(frame, _)| frame)
}

/// Check that a production repair keeps installer markers off standard output.
///
/// A marker on the child's standard output is the failure the observation
/// exists to catch: it would be mixed into whatever the user pipes from
/// `check`. The marker must instead arrive on standard error, and the repair's
/// own notice must reach the caller-supplied diagnostics rather than that
/// stream.
#[test]
fn a_production_repair_puts_installer_markers_on_standard_error() {
    let (root, dirs) = temp_dirs();
    let data_root = dirs
        .whitaker_data()
        .expect("a temporary directory names a data root");

    let output = Command::new(std::env::current_exe().expect("the test binary is known"))
        .arg("repair_production_child")
        .arg("--ignored")
        .env(CHILD_DATA_ROOT, &data_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("the child process should start");
    drop(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the child must complete its repair, stderr: {stderr}"
    );
    assert!(
        !stdout.contains(INSTALLER_MARKER),
        "an installer marker must never reach standard output: {stdout}"
    );
    assert!(
        stderr.contains(INSTALLER_MARKER),
        "the marker must reach the stream the production caller selected: {stderr}"
    );
    let frame = diagnostics_frame(&stderr);
    assert!(
        frame.contains(REPAIR_NOTICE),
        "the repair's own notice must reach the caller's diagnostics, got {frame:?}"
    );
    assert!(
        !frame.contains(INSTALLER_MARKER),
        "a machine marker must stay out of the diagnostics stream: {frame:?}"
    );
}
