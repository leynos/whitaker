//! Property test of the bootstrap lock protocol through the real binary.
//!
//! Each case is a short schedule of rounds. A round optionally perturbs the
//! driver slot, then launches several concurrent `whitaker-installer run`
//! processes against the stub `cargo`, and compares what happened with a
//! reference model that tracks one fact: whether the slot is ready. Only
//! schedule-independent invariants are asserted, because whether concurrent
//! runs overlap depends on timing.
//!
//! The case count is small on purpose: every case starts real processes and
//! each stub build sleeps for a second, so a case costs several seconds.
#![cfg(target_os = "linux")]

mod bootstrap_support;

use bootstrap_support::{Installation, TOOLCHAIN, git};
use camino::Utf8PathBuf;
use proptest::prelude::*;
use std::process::{Child, Stdio};

/// What happens to the driver slot before a round starts.
#[derive(Clone, Copy, Debug)]
enum Perturbation {
    /// Leave the slot alone.
    None,
    /// Delete the ready marker, leaving the driver unmarked.
    DeleteMarker,
    /// Replace the driver with identical bytes, leaving the marker stale.
    ReplaceDriver,
    /// Move the whole data root; readiness must survive a relocation.
    Relocate,
}

/// One round of concurrent runs.
#[derive(Clone, Debug)]
struct Round {
    perturbation: Perturbation,
    processes: usize,
    fails_build: bool,
    lint_status: i32,
}

/// Generate one bounded round.
fn round_strategy() -> impl Strategy<Value = Round> {
    (
        prop_oneof![
            Just(Perturbation::None),
            Just(Perturbation::DeleteMarker),
            Just(Perturbation::ReplaceDriver),
            Just(Perturbation::Relocate),
        ],
        1..=4usize,
        any::<bool>(),
        prop_oneof![Just(0), Just(3)],
    )
        .prop_map(
            |(perturbation, processes, fails_build, lint_status)| Round {
                perturbation,
                processes,
                fails_build,
                lint_status,
            },
        )
}

/// The slot directory holding the lock, marker, and toolchain directory.
fn slot(installation: &Installation) -> Utf8PathBuf {
    installation
        .layout()
        .driver_dir("6.3.0", &installation.commit)
}

/// Apply `perturbation`; return whether the slot is still ready afterwards.
fn perturb(
    installation: &mut Installation,
    perturbation: Perturbation,
    was_ready: bool,
    round_index: usize,
) -> bool {
    let slot_dir = slot(installation);
    match perturbation {
        Perturbation::None => was_ready,
        Perturbation::Relocate => {
            installation.relocate(&format!("moved-{round_index}"));
            was_ready
        }
        Perturbation::DeleteMarker => {
            // A missing marker is the state being forced, so absence is fine.
            let _ = std::fs::remove_file(slot_dir.join(format!(".{TOOLCHAIN}.ready")));
            false
        }
        Perturbation::ReplaceDriver => {
            let dir = slot_dir.join(TOOLCHAIN);
            if dir.join("dylint-driver").exists() {
                let partial = dir.join("dylint-driver.partial");
                std::fs::write(&partial, "#!/bin/sh\n").expect("write the replacement");
                std::fs::rename(&partial, dir.join("dylint-driver")).expect("persist it");
            }
            false
        }
    }
}

/// Spawn every process of `round`, then wait for all of them.
fn run_round(installation: &Installation, round: &Round) -> Vec<std::process::Output> {
    let children: Vec<Child> = (0..round.processes)
        .map(|_| {
            let mut command = installation.run_command(&["--all"]);
            command
                .env("STUB_EXIT", round.lint_status.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            if round.fails_build {
                command.env("STUB_FAIL_BUILD", "1");
            }
            command.spawn().expect("spawn whitaker-installer")
        })
        .collect();
    children
        .into_iter()
        .map(|child| child.wait_with_output().expect("wait for a run"))
        .collect()
}

/// Drive `rounds` against one installation, checking the model each round.
fn check_schedule(rounds: &[Round]) -> Result<(), TestCaseError> {
    let mut installation = Installation::new();
    let mut is_ready = false;
    for (index, round) in rounds.iter().enumerate() {
        is_ready = perturb(&mut installation, round.perturbation, is_ready, index);
        let before = installation.builds();
        let outputs = run_round(&installation, round);
        let new_builds = installation.builds() - before;
        let is_success = is_ready || !round.fails_build;
        let stderr = || {
            outputs
                .iter()
                .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
                .collect::<Vec<_>>()
                .join("\n--\n")
        };
        if is_success {
            // Cold construction is serialized: one build if the slot was not
            // ready, none if it was, whatever the process count.
            prop_assert_eq!(
                new_builds,
                usize::from(!is_ready),
                "round {}: {:?}",
                index,
                stderr()
            );
            for output in &outputs {
                prop_assert_eq!(
                    output.status.code(),
                    Some(round.lint_status),
                    "round {}: {}",
                    index,
                    stderr()
                );
            }
            is_ready = true;
        } else {
            // A failed build leaves the slot retryable and every run failing.
            prop_assert_eq!(new_builds, 0, "round {}", index);
            for output in &outputs {
                prop_assert!(!output.status.success(), "round {}: {}", index, stderr());
            }
            check_recovery(&installation, round.lint_status, index)?;
            is_ready = true;
        }
    }
    if is_ready {
        // A ready slot is warm: a further run builds nothing.
        let before = installation.builds();
        let output = installation.run(&["--all"]);
        prop_assert!(output.status.code().is_some());
        prop_assert_eq!(installation.builds(), before);
    }
    check_environment_and_snapshot(&installation)
}

/// Prove a failed build left the slot retryable, on the unchanged slot.
///
/// A later generated round might relocate or perturb the slot first, or
/// there might be none, so recovery is checked here rather than left to the
/// schedule: a successful retry builds once and keeps the child's status, and
/// a further run is warm.
fn check_recovery(
    installation: &Installation,
    lint_status: i32,
    index: usize,
) -> Result<(), TestCaseError> {
    let before = installation.builds();
    let retry = installation
        .run_command(&["--all"])
        .env("STUB_EXIT", lint_status.to_string())
        .output()
        .expect("retry after the failed build");
    prop_assert_eq!(
        retry.status.code(),
        Some(lint_status),
        "round {} retry: {}",
        index,
        String::from_utf8_lossy(&retry.stderr)
    );
    prop_assert_eq!(
        installation.builds() - before,
        1,
        "round {}: the retry builds once",
        index
    );
    let warm = installation.run(&["--all"]);
    prop_assert!(warm.status.code().is_some(), "round {}: warm run", index);
    prop_assert_eq!(
        installation.builds() - before,
        1,
        "round {}: the slot is ready after the retry",
        index
    );
    Ok(())
}

/// Check what every schedule must leave behind, however it went.
fn check_environment_and_snapshot(installation: &Installation) -> Result<(), TestCaseError> {
    // Every child saw a driver slot for this snapshot under some data root
    // (relocation moves the root), never the stale value.
    let suffix = format!("/drivers/dylint-6.3.0/clippy-{}", installation.commit);
    for line in installation
        .recorded()
        .iter()
        .filter(|l| l.starts_with("DRIVER="))
    {
        prop_assert!(line.ends_with(&suffix), "{}", line);
    }
    // Cold builds verify the snapshot and must leave it at the pinned commit.
    let snapshot = installation.layout().snapshot(&installation.commit);
    let head = git(&snapshot, &["rev-parse", "HEAD"]);
    prop_assert_eq!(head.trim(), installation.commit.as_str());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]

    /// Bounded schedules of concurrent runs agree with the readiness model.
    #[test]
    fn schedules_of_concurrent_runs_agree_with_the_readiness_model(
        rounds in proptest::collection::vec(round_strategy(), 1..=3)
    ) {
        check_schedule(&rounds)?;
    }
}
