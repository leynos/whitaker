"""The Makefile keys the Clippy snapshot and driver paths by the pinned commit.

`make dylint-env` feeds `GITHUB_ENV` in CI. The Windows lane once exported
`DYLINT_CLIPPY_REPO` as the bare snapshot root because the commit lookup's
quoting broke under Windows' `$(shell)`, and every Dylint driver build there
then failed with "DYLINT_CLIPPY_REPO must point to a Git repository". These
tests pin the lookup, including a CRLF checkout of the manifest.
"""

from __future__ import annotations

import os
import subprocess
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MANIFEST = REPO_ROOT / "installer" / "clippy-snapshot.toml"


def _pinned_commit() -> str:
    """Return the commit the committed manifest pins."""
    for line in MANIFEST.read_text(encoding="utf-8").splitlines():
        if line.startswith("commit = "):
            return line.split('"')[1]
    pytest.fail("the manifest pins no commit")


def _dylint_env(manifest: Path, home: Path) -> dict[str, str]:
    """Run `make dylint-env` against `manifest` and parse its output."""
    env = {
        key: value
        for key, value in os.environ.items()
        if key not in {"DYLINT_CLIPPY_REPO", "DYLINT_DRIVER_PATH"}
    }
    env["HOME"] = str(home)
    result = subprocess.run(  # noqa: S603 - fixed argument vector
        [  # noqa: S607 - PATH lookup intended
            "make",
            "--no-print-directory",
            "dylint-env",
            f"CLIPPY_SNAPSHOT_MANIFEST={manifest}",
        ],
        capture_output=True,
        text=True,
        env=env,
        cwd=REPO_ROOT,
        check=True,
    )
    return dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)


@pytest.mark.parametrize("line_ending", ["\n", "\r\n"], ids=["lf", "crlf"])
def test_dylint_env_names_the_pinned_generation_and_driver_slot(
    tmp_path: Path, line_ending: str
) -> None:
    """Both paths end in the pinned commit, whatever the manifest's line endings."""
    manifest = tmp_path / "clippy-snapshot.toml"
    text = MANIFEST.read_text(encoding="utf-8")
    manifest.write_bytes(text.replace("\n", line_ending).encode("utf-8"))
    commit = _pinned_commit()

    variables = _dylint_env(manifest, tmp_path / "home")

    assert variables["DYLINT_CLIPPY_REPO"].endswith(f"/whitaker-clippy-snapshot/{commit}")
    assert variables["DYLINT_DRIVER_PATH"].endswith(f"/clippy-{commit}")


def test_the_commit_lookup_avoids_double_quotes_in_shell() -> None:
    """Windows runners mangle double quotes inside `$(shell)`."""
    makefile = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    lookup: typ.Final = next(
        line for line in makefile.splitlines() if line.startswith("CLIPPY_SNAPSHOT_COMMIT :=")
    )
    assert '"' not in lookup, lookup
