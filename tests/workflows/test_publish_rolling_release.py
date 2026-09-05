"""Contracts for republishing the rolling release without an empty interval.

The rolling tag is what every consumer resolves to fetch prebuilt lint
libraries and the Dylint host tools. The previous publish deleted the release
and the tag and then recreated both, so for six to seven seconds each publish
the tag did not exist. Chutoro's `Install Whitaker` step began at 02:47:02 on
2026-09-05, the same second a delete began, failed to fetch
`cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz`, and fell back to building
the Dylint tools from source.

These tests run the real script against a stubbed `gh` and assert the commands
it issues. Asserting the commands rather than the script's own log output is
deliberate: a log line saying an asset was preserved proves nothing about
whether a delete was sent.
"""

from __future__ import annotations

import os
import stat
import subprocess
import typing as typ

import pytest

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from pathlib import Path

SCRIPT = "scripts/publish-rolling-release.sh"

#: A dependency archive whose name is stable across publishes. This is the
#: asset class that broke a consumer, so it has its own assertions.
DEPENDENCY_ARCHIVE = "cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz"

_GH_STUB = """#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$GH_LOG"
if [[ "$1 $2" == "release view" ]]; then
    if [[ "${RELEASE_EXISTS}" != "true" ]]; then
        exit 1
    fi
    if [[ -n "${PUBLISHED_NAMES}" ]]; then
        printf '%s\\n' ${PUBLISHED_NAMES}
    fi
fi
exit 0
"""


class PublishRun(typ.NamedTuple):
    """One recorded execution of the publish script."""

    returncode: int
    stdout: str
    commands: list[str]

    def matching(self, *fragments: str) -> list[str]:
        """Return the recorded commands containing every fragment."""
        return [
            command
            for command in self.commands
            if all(fragment in command for fragment in fragments)
        ]


@pytest.fixture
def publish(tmp_path: Path, repo_root: Path) -> typ.Callable[..., PublishRun]:
    """Return a callable running the publish script under a stubbed `gh`."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    stub = stub_dir / "gh"
    stub.write_text(_GH_STUB, encoding="utf-8")
    stub.chmod(stub.stat().st_mode | stat.S_IXUSR)
    log = tmp_path / "gh.log"

    def _run(
        *,
        local_assets: list[str],
        published: list[str],
        release_exists: bool = True,
    ) -> PublishRun:
        workspace = tmp_path / "workspace"
        dist = workspace / "dist"
        dist.mkdir(parents=True, exist_ok=True)
        listing = dist / "release-assets.txt"
        listing.write_text(
            "".join(f"dist/{name}\n" for name in local_assets), encoding="utf-8"
        )
        for name in local_assets:
            (dist / name).write_text("payload", encoding="utf-8")
        log.write_text("", encoding="utf-8")

        result = subprocess.run(  # noqa: S603 - fixed argument vector
            ["bash", str(repo_root / SCRIPT)],  # noqa: S607 - PATH lookup intended
            cwd=workspace,
            capture_output=True,
            text=True,
            check=False,
            env=os.environ
            | {
                "PATH": f"{stub_dir}{os.pathsep}{os.environ['PATH']}",
                "GH_LOG": str(log),
                "RELEASE_EXISTS": "true" if release_exists else "false",
                "PUBLISHED_NAMES": " ".join(published),
                "GITHUB_REPOSITORY": "leynos/whitaker",
                "RELEASE_SHA": "0123456789abcdef0123456789abcdef01234567",
                "RELEASE_SHORT_SHA": "0123456",
            },
        )
        commands = [
            line for line in log.read_text(encoding="utf-8").splitlines() if line
        ]
        return PublishRun(result.returncode, result.stdout + result.stderr, commands)

    return _run


@pytest.fixture(scope="session")
def repo_root() -> Path:
    """Return the repository root."""
    from pathlib import Path as _Path

    return _Path(__file__).resolve().parents[2]


def test_the_script_is_executable(repo_root: Path) -> None:
    """A shebang is inert without the executable bit Git records."""
    result = subprocess.run(  # noqa: S603 - fixed argument vector
        ["git", "ls-files", "-s", "--", SCRIPT],  # noqa: S607 - PATH lookup intended
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.stdout.split()[0] == "100755"


def test_a_publish_never_deletes_the_release_or_the_tag(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """The empty interval came from destroying the release; nothing may do that."""
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=[
            "whitaker-lints-old1234-linux.tar.zst",
            "manifest-linux.json",
            DEPENDENCY_ARCHIVE,
        ],
    )

    assert run.returncode == 0, run.stdout
    assert not run.matching("release delete "), (
        "deleting the release empties the tag for the whole republish; that is "
        "the failure this script exists to remove"
    )
    assert not run.matching("--cleanup-tag"), (
        "removing the tag makes every consumer resolving it fail, not only "
        "those fetching an asset"
    )


def test_an_unchanged_dependency_archive_is_left_alone(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """The asset that broke a consumer must not be touched when unchanged.

    When the dependency manifest is unchanged the archives are not rebuilt, so
    they are absent from this run's asset list. They are already published and
    must stay exactly as they are: re-uploading them would delete first, and
    deleting them as superseded would be worse still.
    """
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=[
            "whitaker-lints-old1234-linux.tar.zst",
            "manifest-linux.json",
            DEPENDENCY_ARCHIVE,
        ],
    )

    assert run.returncode == 0, run.stdout
    assert not run.matching(DEPENDENCY_ARCHIVE), (
        f"{DEPENDENCY_ARCHIVE} was neither rebuilt nor superseded, so no "
        "command may name it"
    )


def test_a_new_archive_uploads_without_clobbering(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """An asset whose name is absent needs no delete, so it must not use one.

    `gh release upload --clobber` deletes before it uploads, by its own
    documentation, so using it for a name that is not published would open a
    window for no reason.
    """
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=["manifest-linux.json"],
    )

    assert run.returncode == 0, run.stdout
    uploads = run.matching("release upload", "whitaker-lints-0123456-linux.tar.zst")
    assert uploads, "the new lint archive must be uploaded"
    assert not any("--clobber" in command for command in uploads), (
        "a name that is not published needs no delete before its upload"
    )


def test_manifests_are_replaced_after_the_archives_they_name(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """A manifest must never point at an archive that is not published yet.

    A manifest names the archive it describes. Replacing it after that archive
    exists means a consumer reads either the old manifest with the old archive
    or the new manifest with the new one, and never a manifest referring to
    something absent.
    """
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=["whitaker-lints-old1234-linux.tar.zst", "manifest-linux.json"],
    )

    assert run.returncode == 0, run.stdout
    archive_index = next(
        index
        for index, command in enumerate(run.commands)
        if "release upload" in command
        and "whitaker-lints-0123456-linux.tar.zst" in command
    )
    manifest_index = next(
        index
        for index, command in enumerate(run.commands)
        if "release upload" in command and "manifest-linux.json" in command
    )
    assert archive_index < manifest_index, (
        "the manifest was replaced before the archive it names was published"
    )


def test_the_tag_moves_after_every_upload(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """A consumer resolving the tag must find everything the tag promises."""
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=["whitaker-lints-old1234-linux.tar.zst", "manifest-linux.json"],
    )

    assert run.returncode == 0, run.stdout
    tag_moves = [
        index
        for index, command in enumerate(run.commands)
        if "git/refs/tags/rolling" in command
    ]
    assert len(tag_moves) == 1, "the tag must move exactly once"
    uploads = [
        index
        for index, command in enumerate(run.commands)
        if "release upload" in command
    ]
    assert uploads and max(uploads) < tag_moves[0], (
        "the tag moved while assets were still being uploaded"
    )


def test_superseded_lint_archives_are_removed_last(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """Old archives go only once the new generation is fully published."""
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=["whitaker-lints-old1234-linux.tar.zst", "manifest-linux.json"],
    )

    assert run.returncode == 0, run.stdout
    deletions = run.matching("delete-asset", "whitaker-lints-old1234-linux.tar.zst")
    assert deletions, "a superseded lint archive must be removed"
    delete_index = run.commands.index(deletions[0])
    uploads = [
        index
        for index, command in enumerate(run.commands)
        if "release upload" in command
    ]
    assert max(uploads) < delete_index, (
        "a superseded archive was removed before the new generation was complete"
    )


def test_an_absent_release_is_created_whole(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """With nothing published there is no interval to protect."""
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=[],
        release_exists=False,
    )

    assert run.returncode == 0, run.stdout
    creations = run.matching("release create", "rolling")
    assert len(creations) == 1, "the release must be created exactly once"
    assert not run.matching("release upload"), (
        "creation carries the assets, so no separate upload is needed"
    )


def test_an_empty_asset_list_refuses_to_publish(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """Publishing nothing would empty the release by another route."""
    run = publish(local_assets=[], published=["manifest-linux.json"])

    assert run.returncode != 0
    assert "empty" in run.stdout.lower()
    assert not run.matching("release upload")


def test_the_tag_moves_even_when_the_release_is_absent(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """A deleted release can leave its tag behind at an older commit.

    `gh release create` binds to an existing tag rather than repointing it, so
    without an explicit move the tag would sit at the previous commit while the
    release notes named this one, and every consumer would fetch the wrong
    generation while the run reported success.
    """
    run = publish(
        local_assets=["whitaker-lints-0123456-linux.tar.zst", "manifest-linux.json"],
        published=[],
        release_exists=False,
    )

    assert run.returncode == 0, run.stdout
    moves = run.matching("git/refs/tags/rolling")
    assert len(moves) == 1, "the bootstrap path must move the tag exactly once"
    creation = next(
        index
        for index, command in enumerate(run.commands)
        if "release create" in command
    )
    assert creation < run.commands.index(moves[0]), (
        "the tag must be repointed after the release binds to it"
    )


def test_a_rebuilt_archive_never_disagrees_with_its_checksum(
    publish: typ.Callable[..., PublishRun],
) -> None:
    """A stale checksum beside a new archive is a hard failure, not a retry.

    Clobbering both together would publish the new archive while the old
    checksum was still up. A consumer verifying the digest in that instant sees
    a mismatch and stops. Withdrawing the checksum first leaves only states a
    caller can retry: an absent checksum, never a wrong one.
    """
    archive = "cargo-dylint-x86_64-unknown-linux-gnu-v6.0.2.tgz"
    run = publish(
        local_assets=[archive, f"{archive}.sha256", "manifest-linux.json"],
        published=[archive, f"{archive}.sha256", "manifest-linux.json"],
    )

    assert run.returncode == 0, run.stdout
    withdrawal = run.matching("delete-asset", f"{archive}.sha256")
    assert withdrawal, "the superseded checksum must be withdrawn before the archive"
    withdrawal_index = run.commands.index(withdrawal[0])
    archive_upload = next(
        index
        for index, command in enumerate(run.commands)
        if "release upload" in command
        and archive in command
        and ".sha256" not in command
    )
    checksum_upload = next(
        index
        for index, command in enumerate(run.commands)
        if "release upload" in command and f"{archive}.sha256" in command
    )
    assert withdrawal_index < archive_upload < checksum_upload, (
        "the order must be withdraw the checksum, replace the archive, then "
        f"publish the new checksum; got {run.commands}"
    )
