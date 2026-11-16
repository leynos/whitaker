"""`check-release-published.sh` skips only for a release that is really absent.

`verify-binstall-manifest` skips its install when the version in the manifest
has no release yet, so a version-bump pull request is not red. If every
failed lookup counted as "absent", a rate limit would skip the check green and
verify nothing. These tests run the script against a stub `gh` that returns
each outcome.

Run via ``make test-workflow-contracts``.
"""

import os
import stat
import subprocess
import typing as typ
from pathlib import Path

import pytest

SCRIPT: typ.Final = (
    Path(__file__).resolve().parents[2] / "scripts" / "check-release-published.sh"
)


def _run(tmp_path: Path, stub_body: str) -> subprocess.CompletedProcess[str]:
    """Run the script with a `gh` stub that executes `stub_body`."""
    gh = tmp_path / "gh"
    gh.write_text(f"#!/usr/bin/env bash\n{stub_body}\n", encoding="utf-8")
    gh.chmod(gh.stat().st_mode | stat.S_IEXEC)
    env = {
        "PATH": f"{tmp_path}{os.pathsep}{os.environ['PATH']}",
        "GITHUB_REPOSITORY": "owner/repo",
    }
    return subprocess.run(
        ["bash", str(SCRIPT), "1.2.3"],
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )


def test_an_existing_release_is_published(tmp_path: Path) -> None:
    """A successful lookup reports the release as published."""
    result = _run(tmp_path, 'echo \'{"tag_name":"v1.2.3"}\'')
    assert result.returncode == 0, result.stderr
    assert result.stdout == "published=true\n"


def test_a_confirmed_404_is_not_published(tmp_path: Path) -> None:
    """Only a 404 reports the release as unpublished, and succeeds."""
    result = _run(tmp_path, 'echo "gh: Not Found (HTTP 404)" >&2; exit 1')
    assert result.returncode == 0, result.stderr
    assert result.stdout == "published=false\n"


@pytest.mark.parametrize(
    "message",
    [
        "gh: API rate limit exceeded (HTTP 403)",
        "gh: Bad credentials (HTTP 401)",
        "gh: Server Error (HTTP 502)",
        "error connecting to api.github.com",
    ],
)
def test_any_other_failure_fails_the_check(tmp_path: Path, message: str) -> None:
    """A lookup that failed for another reason is an error, never a skip."""
    result = _run(tmp_path, f'echo "{message}" >&2; exit 1')
    assert result.returncode == 1
    assert "published=" not in result.stdout
    assert message in result.stderr
