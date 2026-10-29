"""`verify-installed-bootstrap.sh` fails when any property it proves breaks.

The script runs only after a release publishes, so each check is exercised
here against PATH stubs (see `release_script_harness`): a cold run, a warm run
that must not rebuild, two simultaneous cold runs, a relocated data root, a
deleted snapshot that must fail clearly, and a denied `github.com`. The
Windows counterpart cannot run here, so a static test holds its scenarios to
the bash script's.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import shutil
import typing as typ
from pathlib import Path

import pytest
from release_script_harness import SCRIPTS, Sandbox, make_sandbox, run_script

SCRIPT: typ.Final[str] = "verify-installed-bootstrap.sh"


@pytest.fixture
def sandbox(tmp_path: Path) -> Sandbox:
    """Provide a sandbox with an installed snapshot and no driver yet."""
    return make_sandbox(tmp_path)


def test_a_healthy_installation_passes(sandbox: Sandbox) -> None:
    """Every property holds, and the denial lands in the scratch hosts file."""
    result = run_script(sandbox, SCRIPT)
    assert result.returncode == 0, result.stderr
    assert "0.0.0.0 github.com" in sandbox.hosts.read_text(encoding="utf-8")
    assert (sandbox.data_root / "whitaker" / "snapshot").is_dir(), (
        "snapshot not restored"
    )


def test_a_warm_run_does_not_rebuild_the_driver(sandbox: Sandbox) -> None:
    """Only the cold run and the concurrent cold runs build a driver."""
    run_script(sandbox, SCRIPT)
    builds = [line for line in sandbox.call_lines() if line == "driver-build"]
    # Cold, one for the concurrent pair, and one for the relocated run, which
    # reuses that driver; so two builds in total.
    assert len(builds) == 2, builds


def test_an_existing_denial_is_not_reapplied(sandbox: Sandbox) -> None:
    """A host that already denies github.com is left alone."""
    sandbox.hosts.write_text("0.0.0.0 github.com\n", encoding="utf-8")
    result = run_script(sandbox, SCRIPT)
    assert result.returncode == 0, result.stderr
    assert not any(line.startswith("sudo") for line in sandbox.call_lines())


@pytest.mark.parametrize(
    ("toggles", "message"),
    [
        pytest.param(
            {"FAKE_DENIAL_INEFFECTIVE": "1"},
            "canary still succeeds",
            id="denial-ineffective",
        ),
        pytest.param(
            {"FAKE_SUCCEED": "1"}, "succeeded on a std::fs violation", id="lint-passes"
        ),
        pytest.param(
            {"FAKE_NO_DIAGNOSTIC": "1"},
            "without a no_std_fs_operations diagnostic",
            id="crash-not-diagnostic",
        ),
        pytest.param(
            {"FAKE_REBUILD_ALWAYS": "1"},
            "warm run rebuilt the driver",
            id="warm-rebuilds",
        ),
        pytest.param(
            {"FAKE_CONCURRENT_FAILS": "1"},
            "concurrent run did not report the diagnostic",
            id="concurrent-fails",
        ),
        pytest.param(
            {"FAKE_IGNORE_XDG": "1"},
            "relocated installation did not lint",
            id="relocation-ignored",
        ),
        pytest.param(
            {"FAKE_SUCCEED_WITHOUT_SNAPSHOT": "1"},
            "ran without its Clippy snapshot",
            id="runs-without-snapshot",
        ),
        pytest.param(
            {"FAKE_SILENT_WITHOUT_SNAPSHOT": "1"},
            "missing snapshot did not fail clearly",
            id="no-fallback-message-missing",
        ),
        pytest.param(
            {"FAKE_LINT_WITHOUT_SNAPSHOT": "1"},
            "lints ran although the snapshot was missing",
            id="lints-despite-missing-snapshot",
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


def test_a_missing_snapshot_fails_before_any_lint_runs(sandbox: Sandbox) -> None:
    """An installation without its snapshot is refused up front."""
    shutil.rmtree(sandbox.data_root / "whitaker" / "snapshot")
    result = run_script(sandbox, SCRIPT)
    assert result.returncode == 1
    assert "is missing" in result.stderr
    assert "driver-build" not in sandbox.call_lines()


def test_the_snapshot_is_restored_after_the_deleted_snapshot_check(
    sandbox: Sandbox,
) -> None:
    """A failing deleted-snapshot check still puts the generation back."""
    run_script(sandbox, SCRIPT, FAKE_SUCCEED_WITHOUT_SNAPSHOT="1")
    assert (sandbox.data_root / "whitaker" / "snapshot").is_dir()


def _scenarios(path: Path) -> dict[str, bool]:
    """Report which scenario headings a verification script announces."""
    text = path.read_text(encoding="utf-8")
    return {
        "cold": "Cold run" in text,
        "warm": "Warm run" in text,
        "concurrent": "Two simultaneous cold runs" in text,
        "deleted": "Deleted snapshot generation" in text,
        "canary": "canary still succeeds" in text,
        "diagnostic": "no_std_fs_operations" in text,
        "no-fallback": "never falls back to cloning public Clippy" in text,
        "warm-rebuild": "the warm run rebuilt the driver" in text,
        "lints-ran": "lints ran although the snapshot was missing" in text,
    }


def test_the_powershell_script_mirrors_the_bash_scenarios() -> None:
    """The Windows script checks every property the bash script does."""
    bash = _scenarios(SCRIPTS / "verify-installed-bootstrap.sh")
    powershell = _scenarios(SCRIPTS / "verify-installed-bootstrap.ps1")
    assert all(bash.values()), f"the bash script lacks a scenario: {bash}"
    assert powershell == bash, f"PowerShell {powershell} differs from bash {bash}"
    # Relocation is Linux-only by design: the PowerShell script says so.
    ps_text = (SCRIPTS / "verify-installed-bootstrap.ps1").read_text(encoding="utf-8")
    assert "Data" in ps_text, "the PowerShell script names the data root"
    assert "relocation" in ps_text, "the PowerShell script explains the relocation gap"
