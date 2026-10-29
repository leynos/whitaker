//! The producer build bundles exactly the pinned history, and only that.

use super::*;
use crate::clippy_snapshot::test_fixtures::{DEFAULT_HISTORY, DEFAULT_SELECTED, FixtureRepo, git};

/// Build from the fixture's work tree as the upstream source.
fn build(
    repo: &FixtureRepo,
    manifest: &ClippySnapshotManifest,
) -> Result<BuiltBundle, ClippySnapshotError> {
    build_bundle(&BuildRequest {
        manifest,
        source: repo.work.as_str(),
        work_dir: &repo.path("work-dir"),
        output_dir: &repo.path("dist"),
        token: None,
    })
}

#[test]
fn builds_a_self_verified_bundle_and_sidecar() {
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    let manifest = repo.manifest(repo.tip(), &repo.commits[DEFAULT_SELECTED]);

    let built = build(&repo, &manifest).expect("the producer build succeeds");

    assert_eq!(
        built.artefact.bundle.file_name(),
        Some(manifest.bundle_asset_name().as_str())
    );
    let sidecar = std::fs::read_to_string(&built.sidecar).expect("read sidecar");
    assert_eq!(
        sidecar,
        format!(
            "{}  {}\n",
            built.artefact.sha256,
            manifest.bundle_asset_name()
        )
    );
    let heads = git(
        &repo.work,
        &["bundle", "list-heads", built.artefact.bundle.as_str()],
    );
    assert_eq!(
        heads,
        format!("{tip} refs/heads/main\n{tip} HEAD", tip = repo.tip())
    );
}

/// Pinning an older `main` commit moves `main` back to it in the bundle.
#[test]
fn pins_main_to_an_older_first_parent_commit() {
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    let pinned = repo.commits[5].clone();
    let manifest = repo.manifest(&pinned, &repo.commits[DEFAULT_SELECTED]);

    let built = build(&repo, &manifest).expect("the producer build succeeds");

    let heads = git(
        &repo.work,
        &["bundle", "list-heads", built.artefact.bundle.as_str()],
    );
    assert!(
        heads.lines().all(|line| line.starts_with(&pinned)),
        "{heads}"
    );
}

#[test]
fn refuses_a_commit_that_is_not_on_mains_first_parent_history() {
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    git(
        &repo.work,
        &["checkout", "--quiet", "-b", "side", &repo.commits[4]],
    );
    std::fs::write(repo.work.join("side.txt"), "side\n").expect("write side file");
    git(&repo.work, &["add", "--all"]);
    git(&repo.work, &["commit", "--quiet", "--message", "side"]);
    let side = git(&repo.work, &["rev-parse", "HEAD"]);
    git(&repo.work, &["checkout", "--quiet", "main"]);
    git(
        &repo.work,
        &["merge", "--quiet", "--no-ff", "--no-edit", "side"],
    );
    let manifest = repo.manifest(&side, &repo.commits[DEFAULT_SELECTED]);

    let error = build(&repo, &manifest).expect_err("a side-branch commit is refused");

    assert!(
        error
            .to_string()
            .contains("not on main's first-parent history"),
        "{error}"
    );
}

/// The retry token reaches GitHub URLs and nothing else.
#[rstest::rstest]
#[case::upstream("https://github.com/rust-lang/rust-clippy", true)]
#[case::mirror("https://mirror.example.invalid/rust-clippy", false)]
#[case::lookalike_host("https://github.com.example.invalid/rust-clippy", false)]
fn the_retry_header_is_scoped_to_github(#[case] url: &str, #[case] is_sent: bool) {
    let output = std::process::Command::new("git")
        .args(["config", "--get-urlmatch", "http.extraHeader", url])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", GITHUB_HEADER_KEY)
        .env("GIT_CONFIG_VALUE_0", "Authorization: Bearer token")
        .output()
        .expect("run git config");
    let header = String::from_utf8_lossy(&output.stdout);
    assert_eq!(header.contains("Bearer token"), is_sent, "{url}: {header}");
}

/// Regression: a relative `--output-dir` was handed to Git running inside the
/// mirror, so the bundle was written somewhere else or not at all.
#[cfg(unix)]
#[test]
fn writes_the_bundle_into_a_relative_output_directory() {
    use crate::clippy_snapshot::test_fixtures::{build_tree_scratch, relative_to_cwd, utf8};
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    let manifest = repo.manifest(repo.tip(), &repo.commits[DEFAULT_SELECTED]);
    let scratch = build_tree_scratch();
    let output = utf8(scratch.path()).join("relative-dist");
    let relative_output = relative_to_cwd(&output);
    let relative_work = relative_to_cwd(&utf8(scratch.path()).join("w"));

    let built = build_bundle(&BuildRequest {
        manifest: &manifest,
        source: repo.work.as_str(),
        work_dir: &relative_work,
        output_dir: &relative_output,
        token: None,
    })
    .expect("the producer build succeeds with relative directories");

    assert!(output.join(manifest.bundle_asset_name()).is_file());
    assert!(built.artefact.bundle.is_absolute());
}
