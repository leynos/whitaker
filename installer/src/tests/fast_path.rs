//! Tests for fast-path installer helper behaviour.

use super::*;
use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use temp_env::with_var_unset;
use whitaker_installer::crate_name::CrateName;
use whitaker_installer::test_support::{TEST_STAGE_SUITE_ENV, env_test_guard};
use whitaker_installer::toolchain::Toolchain;

struct FastPathFixture {
    args: InstallArgs,
    dirs: TestBaseDirs,
    toolchain: Toolchain,
    target_dir: Utf8PathBuf,
    requested_crates: Vec<CrateName>,
}

impl FastPathFixture {
    fn context(&self) -> FastPathContext<'_> {
        FastPathContext {
            args: &self.args,
            dirs: &self.dirs,
            requested_crates: &self.requested_crates,
            toolchain: &self.toolchain,
            target_dir: &self.target_dir,
        }
    }
}

#[fixture]
fn fast_path_fixture() -> FastPathFixture {
    FastPathFixture {
        args: InstallArgs::default(),
        dirs: TestBaseDirs {
            home_dir: Some("/tmp".into()),
            bin_dir: Some("/tmp/bin".into()),
            data_dir: Some("/tmp".into()),
        },
        toolchain: Toolchain::with_override(Utf8Path::new("."), "nightly-2025-09-18"),
        target_dir: Utf8PathBuf::from("/tmp/target"),
        requested_crates: vec![],
    }
}

#[rstest]
#[case::without_cranelift(false, &[])]
#[case::with_cranelift(true, &["rustc-codegen-cranelift"])]
fn resolve_additional_components_parametrised(#[case] cranelift: bool, #[case] expected: &[&str]) {
    let args = InstallArgs {
        cranelift,
        ..InstallArgs::default()
    };

    assert_eq!(super::resolve_additional_components(&args), expected);
}

#[rstest]
fn fast_path_context_holds_supplied_values(fast_path_fixture: FastPathFixture) {
    let ctx = fast_path_fixture.context();

    assert!(std::ptr::eq(ctx.args, &fast_path_fixture.args));
    assert_eq!(ctx.dirs.home_dir(), Some(PathBuf::from("/tmp")));
    assert_eq!(ctx.toolchain.channel(), "nightly-2025-09-18");
    assert_eq!(ctx.target_dir, &Utf8PathBuf::from("/tmp/target"));
    assert!(ctx.requested_crates.is_empty());
}

#[rstest]
fn try_fast_path_installation_returns_none_when_prebuilt_disabled(
    mut fast_path_fixture: FastPathFixture,
) {
    let _guard = env_test_guard();
    fast_path_fixture.args = InstallArgs {
        is_build_only: true,
        lint: vec!["module_max_lines".to_owned()],
        ..InstallArgs::default()
    };
    fast_path_fixture.requested_crates = vec![CrateName::from("module_max_lines")];

    with_var_unset(TEST_STAGE_SUITE_ENV, || {
        let ctx = fast_path_fixture.context();
        let mut stderr = Vec::new();
        let result = try_fast_path_installation(&ctx, &mut stderr).expect("should not error");
        assert!(result.is_none());
    });
}

#[rstest]
fn try_fast_path_installation_returns_some_build_path_when_staged_suite_enabled(
    mut fast_path_fixture: FastPathFixture,
) {
    use temp_env::with_var;
    use whitaker_installer::test_support::TEST_STAGE_SUITE_ENV;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    fast_path_fixture.target_dir =
        Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).expect("temp dir is valid UTF-8");
    // Suite-only request satisfies is_suite_only_request
    fast_path_fixture.requested_crates = vec![CrateName::from("whitaker_suite")];
    // Disable the prebuilt path so only the staged-suite branch fires
    fast_path_fixture.args = InstallArgs {
        is_build_only: true,
        ..InstallArgs::default()
    };

    with_var(TEST_STAGE_SUITE_ENV, Some("1"), || {
        let ctx = fast_path_fixture.context();
        let mut stderr = Vec::new();
        let result = try_fast_path_installation(&ctx, &mut stderr).expect("should not error");
        assert!(
            result.is_some(),
            "expected Some((path, InstallMode::Build)), got None"
        );
        let (_, mode) = result.expect("staged suite should produce a build-mode fast path");
        assert_eq!(mode, InstallMode::Build);
    });
}
