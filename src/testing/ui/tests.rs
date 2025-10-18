use super::{HarnessError, run_with_runner};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

#[rstest]
#[case(
    "  ",
    "ui",
    HarnessError::EmptyCrateName,
    "crate name validation should fail"
)]
#[case(
    "lint",
    "   ",
    HarnessError::EmptyDirectory,
    "empty directories should be rejected"
)]
fn rejects_invalid_inputs(
    #[case] crate_name: &str,
    #[case] directory: &str,
    #[case] expected: HarnessError,
    #[case] panic_message: &str,
) {
    let Err(error) = run_with_runner(crate_name, directory, |_, _| Ok(())) else {
        panic!("{panic_message}");
    };

    assert_eq!(error, expected);
}

#[test]
fn rejects_absolute_directories() {
    let path = Utf8PathBuf::from("/tmp/ui");
    let Err(error) = run_with_runner("lint", path.clone(), |_, _| Ok(())) else {
        panic!("absolute directories should be rejected");
    };

    assert_eq!(error, HarnessError::AbsoluteDirectory { directory: path });
}

#[test]
fn propagates_runner_failures() {
    let Err(error) = run_with_runner("lint", "ui", |crate_name, directory| {
        assert_eq!(crate_name, "lint");
        assert_eq!(directory, Utf8Path::new("ui"));
        Err("diff mismatch".to_string())
    }) else {
        panic!("runner failures should bubble up");
    };

    assert_eq!(
        error,
        HarnessError::RunnerFailure {
            crate_name: "lint".to_string(),
            directory: Utf8PathBuf::from("ui"),
            message: "diff mismatch".to_string(),
        },
    );
}
