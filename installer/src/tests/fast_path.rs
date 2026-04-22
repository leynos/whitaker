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
