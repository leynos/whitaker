//! Each structural check rejects the repository shape it exists to catch.

use super::*;
use crate::clippy_snapshot::test_fixtures::{
    DEFAULT_HISTORY, DEFAULT_SELECTED, FixtureRepo, git, utf8,
};
use camino::Utf8PathBuf;
use rstest::rstest;

/// A bare clone of the default fixture, damaged by `damage`.
fn damaged(damage: Damage) -> (FixtureRepo, Utf8PathBuf, ClippySnapshotManifest) {
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    let manifest = repo.manifest(repo.tip(), &repo.commits[DEFAULT_SELECTED]);
    let bare = repo.path("snapshot.git");
    git(
        &utf8(repo.dir.path()),
        &[
            "clone",
            "--quiet",
            "--bare",
            repo.work.as_str(),
            bare.as_str(),
        ],
    );
    git(&bare, &["remote", "remove", "origin"]);
    damage.apply(&repo, &bare);
    (repo, bare, manifest)
}

/// Repository shapes a snapshot must not have.
#[derive(Debug, Clone, Copy)]
enum Damage {
    None,
    HeadOnMaster,
    Shallow,
    Alternates,
    Remote,
    CorruptObject,
}

impl Damage {
    /// Apply this damage to the bare clone at `bare`.
    fn apply(self, repo: &FixtureRepo, bare: &Utf8Path) {
        match self {
            Self::None => {}
            Self::HeadOnMaster => {
                git(bare, &["branch", "master", "main"]);
                git(bare, &["symbolic-ref", "HEAD", "refs/heads/master"]);
                git(bare, &["branch", "--delete", "main"]);
            }
            Self::Shallow => {
                let parent = &repo.commits[DEFAULT_SELECTED - 1];
                std::fs::write(bare.join("shallow"), format!("{parent}\n")).expect("write shallow");
            }
            Self::Alternates => {
                let objects = repo.work.join(".git/objects");
                std::fs::write(bare.join("objects/info/alternates"), format!("{objects}\n"))
                    .expect("write alternates");
            }
            Self::Remote => {
                git(
                    bare,
                    &["remote", "add", "upstream", "https://example.invalid/x"],
                );
            }
            Self::CorruptObject => corrupt_loose_object(bare),
        }
    }
}

/// Unpack the history and overwrite one loose object with garbage.
fn corrupt_loose_object(bare: &Utf8Path) {
    git(bare, &["repack", "-a", "-d", "--quiet"]);
    let packs: Vec<_> = std::fs::read_dir(bare.join("objects/pack"))
        .expect("list packs")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "pack"))
        .collect();
    let pack = packs.first().expect("one pack").clone();
    let mut permissions = std::fs::metadata(&pack)
        .expect("pack metadata")
        .permissions();
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "the test deliberately damages a pack Git wrote read-only"
    )]
    permissions.set_readonly(false);
    std::fs::set_permissions(&pack, permissions).expect("make pack writable");
    let mut bytes = std::fs::read(&pack).expect("read pack");
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xff;
    std::fs::write(&pack, bytes).expect("write pack");
}

#[test]
fn accepts_an_undamaged_snapshot() {
    let (_repo, bare, manifest) = damaged(Damage::None);
    verify_repository(&bare, &manifest).expect("an undamaged snapshot verifies");
}

#[rstest]
#[case::head_on_master(Damage::HeadOnMaster, "HEAD is `refs/heads/master`")]
#[case::shallow(Damage::Shallow, "its history is shallow")]
#[case::alternates(Damage::Alternates, "objects/info/alternates")]
#[case::remote(Damage::Remote, "it has remotes (upstream)")]
#[case::corrupt_object(Damage::CorruptObject, "git fsck --full failed")]
fn rejects_damaged_snapshots(#[case] damage: Damage, #[case] expected: &str) {
    let (_repo, bare, manifest) = damaged(damage);

    let error = verify_repository(&bare, &manifest).expect_err("damage is detected");

    assert!(
        error.to_string().contains(expected),
        "`{error}` should mention `{expected}`"
    );
}

#[test]
fn rejects_a_work_tree() {
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    let manifest = repo.manifest(repo.tip(), &repo.commits[DEFAULT_SELECTED]);

    let error = verify_repository(&repo.work, &manifest).expect_err("work trees are rejected");

    assert!(
        error.to_string().contains("not a bare repository"),
        "{error}"
    );
}
