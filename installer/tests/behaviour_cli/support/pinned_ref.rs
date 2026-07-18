//! Pinned-ref setup and assertions for installer CLI behaviour scenarios.

use super::{CliWorld, ensure_required_toolchain_available, output_for_assertions, setup_temp_dir};

/// The ref used by the pinned-install CLI scenarios.
const SCENARIO_REF: &str = "v0.2.5";

pub(crate) fn configure_dry_run_with_pinned_ref(cli_world: &CliWorld) {
    let Some(channel) = ensure_required_toolchain_available(cli_world) else {
        return;
    };

    let target_dir = setup_temp_dir(cli_world);
    cli_world.args.replace(vec![
        "--dry-run".to_owned(),
        "--toolchain".to_owned(),
        channel,
        "--target-dir".to_owned(),
        target_dir,
        "--ref".to_owned(),
        SCENARIO_REF.to_owned(),
    ]);
}

pub(crate) fn configure_ref_in_workspace(cli_world: &CliWorld) {
    // The harness runs the installer from the Whitaker workspace root, so a
    // `--ref` must be refused. `--skip-deps` keeps the refusal ahead of any
    // dependency installation, and no toolchain is required to reach it.
    cli_world.args.replace(vec![
        "--ref".to_owned(),
        SCENARIO_REF.to_owned(),
        "--skip-deps".to_owned(),
    ]);
}

pub(crate) fn assert_pinned_ref_output_is_shown(cli_world: &CliWorld) {
    let Some(output) = output_for_assertions(cli_world) else {
        return;
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("Pinned ref: {SCENARIO_REF}")),
        "expected dry-run output to report the pinned ref, stderr: {stderr}"
    );
}

pub(crate) fn assert_ref_unsupported_message_is_shown(cli_world: &CliWorld) {
    let Some(output) = output_for_assertions(cli_world) else {
        return;
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("the current directory is itself a Whitaker workspace"),
        "expected a ref-unsupported refusal, stderr: {stderr}"
    );
}
