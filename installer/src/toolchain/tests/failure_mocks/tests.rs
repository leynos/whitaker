use super::*;
use rstest::{fixture, rstest};

#[test]
fn component_failure_match_rejects_superstring_messages() {
    let err = InstallerError::ToolchainComponentInstallFailed {
        toolchain: "nightly-2025-09-18".to_owned(),
        components: expected_components(&[]),
        message: format!("{COMPONENT_INSTALL_FAILURE_MESSAGE}: while syncing index"),
    };

    assert!(
        !is_component_install_failed(&err, "nightly-2025-09-18", &[]),
        "component failure helper should require the exact mocked failure marker"
    );
}

#[fixture]
fn channel() -> ToolchainChannel<'static> {
    ToolchainChannel("nightly-2025-09-18")
}

struct FailureErrorCase<'a> {
    err: InstallerError,
    failure: InstallFailure,
    additional_components: &'a [&'a str],
    expected_panic_message: Option<&'a str>,
}

#[rstest]
#[case::matching_toolchain_install(
    FailureErrorCase {
        err: InstallerError::ToolchainInstallFailed {
            toolchain: "nightly-2025-09-18".to_owned(),
            message: TOOLCHAIN_INSTALL_FAILURE_MESSAGE.to_owned(),
        },
        failure: InstallFailure::ToolchainInstall,
        additional_components: &[],
        expected_panic_message: None,
    }
)]
#[case::matching_component_add(
    FailureErrorCase {
        err: InstallerError::ToolchainComponentInstallFailed {
            toolchain: "nightly-2025-09-18".to_owned(),
            components: expected_components(&[CRANELIFT_COMPONENT]),
            message: COMPONENT_INSTALL_FAILURE_MESSAGE.to_owned(),
        },
        failure: InstallFailure::ComponentAdd,
        additional_components: &[CRANELIFT_COMPONENT],
        expected_panic_message: None,
    }
)]
#[case::matching_toolchain_unusable(
    FailureErrorCase {
        err: InstallerError::ToolchainNotInstalled {
            toolchain: "nightly-2025-09-18".to_owned(),
        },
        failure: InstallFailure::ToolchainUnusableAfterInstall,
        additional_components: &[],
        expected_panic_message: None,
    }
)]
#[case::toolchain_install_mismatch(
    FailureErrorCase {
        err: InstallerError::ToolchainNotInstalled {
            toolchain: "nightly-2025-09-18".to_owned(),
        },
        failure: InstallFailure::ToolchainInstall,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainInstallFailed for channel nightly-2025-09-18 while exercising toolchain installation failure",
        ),
    }
)]
#[case::toolchain_install_wrong_channel(
    FailureErrorCase {
        err: InstallerError::ToolchainInstallFailed {
            toolchain: "stable".to_owned(),
            message: TOOLCHAIN_INSTALL_FAILURE_MESSAGE.to_owned(),
        },
        failure: InstallFailure::ToolchainInstall,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainInstallFailed for channel nightly-2025-09-18 while exercising toolchain installation failure",
        ),
    }
)]
#[case::toolchain_install_wrong_message(
    FailureErrorCase {
        err: InstallerError::ToolchainInstallFailed {
            toolchain: "nightly-2025-09-18".to_owned(),
            message: "unexpected error".to_owned(),
        },
        failure: InstallFailure::ToolchainInstall,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainInstallFailed for channel nightly-2025-09-18 while exercising toolchain installation failure",
        ),
    }
)]
#[case::component_add_mismatch(
    FailureErrorCase {
        err: InstallerError::ToolchainNotInstalled {
            toolchain: "nightly-2025-09-18".to_owned(),
        },
        failure: InstallFailure::ComponentAdd,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainComponentInstallFailed for channel nightly-2025-09-18 while exercising component installation failure for rust-src, rustc-dev, llvm-tools-preview",
        ),
    }
)]
#[case::component_add_wrong_channel(
    FailureErrorCase {
        err: InstallerError::ToolchainComponentInstallFailed {
            toolchain: "stable".to_owned(),
            components: expected_components(&[]),
            message: COMPONENT_INSTALL_FAILURE_MESSAGE.to_owned(),
        },
        failure: InstallFailure::ComponentAdd,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainComponentInstallFailed for channel nightly-2025-09-18 while exercising component installation failure",
        ),
    }
)]
#[case::component_add_wrong_components(
    FailureErrorCase {
        err: InstallerError::ToolchainComponentInstallFailed {
            toolchain: "nightly-2025-09-18".to_owned(),
            components: "rust-src".to_owned(),
            message: COMPONENT_INSTALL_FAILURE_MESSAGE.to_owned(),
        },
        failure: InstallFailure::ComponentAdd,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainComponentInstallFailed for channel nightly-2025-09-18 while exercising component installation failure",
        ),
    }
)]
#[case::component_add_wrong_message(
    FailureErrorCase {
        err: InstallerError::ToolchainComponentInstallFailed {
            toolchain: "nightly-2025-09-18".to_owned(),
            components: expected_components(&[]),
            message: "unexpected error".to_owned(),
        },
        failure: InstallFailure::ComponentAdd,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainComponentInstallFailed for channel nightly-2025-09-18 while exercising component installation failure",
        ),
    }
)]
#[case::toolchain_unusable_mismatch(
    FailureErrorCase {
        err: InstallerError::ToolchainInstallFailed {
            toolchain: "nightly-2025-09-18".to_owned(),
            message: TOOLCHAIN_INSTALL_FAILURE_MESSAGE.to_owned(),
        },
        failure: InstallFailure::ToolchainUnusableAfterInstall,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainNotInstalled for channel nightly-2025-09-18 while exercising toolchain unusable after installation",
        ),
    }
)]
#[case::toolchain_unusable_wrong_channel(
    FailureErrorCase {
        err: InstallerError::ToolchainNotInstalled {
            toolchain: "stable".to_owned(),
        },
        failure: InstallFailure::ToolchainUnusableAfterInstall,
        additional_components: &[],
        expected_panic_message: Some(
            "ToolchainNotInstalled for channel nightly-2025-09-18 while exercising toolchain unusable after installation",
        ),
    }
)]
fn assert_failure_error_handles_expected_outcomes(
    channel: ToolchainChannel<'_>,
    #[case] case: FailureErrorCase<'_>,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_failure_error(
            case.err,
            channel,
            FailureSetup {
                failure: case.failure,
                additional_components: case.additional_components,
            },
        );
    }));

    match case.expected_panic_message {
        Some(expected_message) => {
            let panic_payload = result.expect_err("mismatched installer error should panic");
            let message = panic_message(panic_payload);
            assert!(
                message.contains(expected_message),
                "expected panic containing '{expected_message}', got '{message}'"
            );
        }
        None => result.expect("matching installer error should not panic"),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
        })
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}
