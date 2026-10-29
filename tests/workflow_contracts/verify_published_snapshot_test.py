"""`verify-published-snapshot.sh` fails when any property it proves breaks.

The script runs only after a release publishes, so each check is exercised
here against PATH stubs (see `release_script_harness`). Every negative case
makes one stub violate one property and requires the script to name it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import hashlib
import subprocess
import typing as typ
from pathlib import Path

import pytest
from release_script_harness import ASSET, SCRIPTS, Sandbox, make_sandbox, run_script

SCRIPT: typ.Final[str] = "verify-published-snapshot.sh"


@pytest.fixture
def sandbox(tmp_path: Path) -> Sandbox:
    """Provide a sandbox holding a valid published release."""
    return make_sandbox(tmp_path)


def test_a_healthy_release_passes(sandbox: Sandbox) -> None:
    """The published snapshot installs and the tampered copy is refused."""
    result = run_script(sandbox, SCRIPT)
    assert result.returncode == 0, result.stderr
    assert "github.com" in sandbox.hosts.read_text(encoding="utf-8")
    assert not (sandbox.work / "tampered").exists()
    installs = [line for line in sandbox.call_lines() if " install " in line]
    assert any("--bundle" in line for line in installs), "no tampered install ran"


@pytest.mark.parametrize(
    ("toggles", "message"),
    [
        pytest.param(
            {"FAKE_ACCEPT_TAMPERED": "1"},
            "a tampered bundle was installed",
            id="accepted",
        ),
        pytest.param(
            {"FAKE_WRONG_REJECTION": "1"},
            "not rejected by its digest",
            id="wrong-rejection",
        ),
        pytest.param(
            {"FAKE_DENIAL_INEFFECTIVE": "1"},
            "canary still succeeds",
            id="denial-ineffective",
        ),
        pytest.param(
            {"FAKE_LEAK_GENERATION": "1"},
            "left a generation behind",
            id="leaked-generation",
        ),
    ],
)
def test_a_violated_property_fails_the_script(
    sandbox: Sandbox, toggles: dict[str, str], message: str
) -> None:
    """Each broken stub behaviour is reported by the matching check."""
    result = run_script(sandbox, SCRIPT, **toggles)
    assert result.returncode == 1, result.stdout
    assert message in result.stderr


def test_a_sidecar_that_does_not_match_the_bundle_fails(sandbox: Sandbox) -> None:
    """A mismatched published sidecar is caught before any tampering."""
    sidecar = sandbox.release / f"{ASSET}.sha256"
    sidecar.write_text(f"{'0' * 64}  {ASSET}\n", encoding="utf-8")
    # The stub installer would reject the mismatch first, so let it accept
    # the pair and leave the script's own `sha256sum --check` to catch it.
    result = run_script(sandbox, SCRIPT, FAKE_ACCEPT_TAMPERED="1")
    assert result.returncode == 1
    assert "published sidecar does not match" in result.stderr


def test_a_bundle_too_short_to_tamper_is_reported(sandbox: Sandbox) -> None:
    """A bundle shorter than the tamper offset fails with the script's reason.

    Regression: ``od`` failed past the end of the file and ``set -e`` aborted
    the script before its own message.
    """
    bundle = sandbox.release / ASSET
    bundle.write_bytes(b"short bundle")
    digest = hashlib.sha256(bundle.read_bytes()).hexdigest()
    (sandbox.release / f"{ASSET}.sha256").write_text(
        f"{digest}  {ASSET}\n", encoding="utf-8"
    )
    result = run_script(sandbox, SCRIPT)
    assert result.returncode == 1, result.stdout
    assert "shorter than 4096 bytes" in result.stderr


def test_a_missing_work_directory_argument_is_a_usage_error(sandbox: Sandbox) -> None:
    """The script refuses to run without its WORK_DIR argument."""
    result = subprocess.run(
        ["bash", str(SCRIPTS / SCRIPT)],
        capture_output=True,
        text=True,
        cwd=sandbox.repository,
        check=False,
    )
    assert result.returncode == 2
    assert "usage" in result.stderr
