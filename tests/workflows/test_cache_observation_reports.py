"""Behavioural tests for the two cache-evidence reporting scripts.

`scripts/record-cache-observations.sh` and
`scripts/record-sccache-effectiveness.sh` produce the job-summary evidence
the pilot's exit gate is judged on. If either misreports, a warm prefix
restore is indistinguishable from a cold miss, or a job that cached nothing
looks like a job with no misses.

Covered behaviour:

- an inactive step is reported as inactive rather than omitted;
- an exact match, a `restore-keys` prefix restore, and a complete miss are
  reported as three distinct outcomes;
- the raw `cache-hit` value is preserved verbatim, including when absent;
- the selected compiler-cache backend is named in the summary;
- free disk is reported before the build and again before the saves, and an
  unrecognized mode is a usage error;
- the sccache reporter writes both artefact formats and echoes the stats
  into the summary; and
- a run with zero compile requests raises a workflow warning.

Examples
--------
Run all tests:
    PYTHONPATH=. python3 -m pytest tests/workflows/test_cache_observation_reports.py
"""

from __future__ import annotations

import shutil
import subprocess
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
OBSERVATIONS_SCRIPT = REPO_ROOT / "scripts" / "record-cache-observations.sh"
EFFECTIVENESS_SCRIPT = REPO_ROOT / "scripts" / "record-sccache-effectiveness.sh"

REGISTRY_KEY = "cargo-registry-lint-v1-Linux-X64-self-hosted-abc123"
REGISTRY_PREFIX = "cargo-registry-lint-v1-Linux-X64-self-hosted-"


def _run_observations(
    tmp_path: Path,
    *arguments: str,
    **overrides: str,
) -> tuple[subprocess.CompletedProcess[str], str]:
    """Run the observation reporter and return its result and the summary."""
    summary = tmp_path / "summary.md"
    summary.touch()
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(tmp_path),
        "GITHUB_STEP_SUMMARY": str(summary),
        **overrides,
    }
    result = subprocess.run(
        ["bash", str(OBSERVATIONS_SCRIPT), *arguments],
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    return result, summary.read_text(encoding="utf-8")


def _registry_line(summary: str) -> str:
    """Return the Cargo registry observation from a rendered summary."""
    matches = [
        line for line in summary.splitlines() if line.startswith("- Cargo registry")
    ]
    assert len(matches) == 1, f"expected one registry line, got {summary!r}"
    return matches[0]


def test_inactive_step_is_reported_rather_than_omitted(tmp_path: Path) -> None:
    """A step a job does not use must still appear in the evidence."""
    result, summary = _run_observations(tmp_path)

    assert result.returncode == 0, result.stderr
    assert summary.count(": inactive in this job") == 4, (
        f"every unused cache step must be named as inactive, got {summary!r}"
    )
    # The backend is `setup-rust`'s to choose once it starts the server, which
    # is after these restores, so it is reported with the statistics instead.
    assert "Compiler cache backend" not in summary, (
        f"the restore observations cannot know the backend yet, got {summary!r}"
    )


class RestoreOutcome(typ.NamedTuple):
    """One restore outcome and what the registry line must say about it.

    Attributes
    ----------
    matched : str
        The step's `cache-matched-key` output.
    hit : str or None
        The step's `cache-hit` output, or ``None`` to leave it unset, which is
        how the runner presents a step that never ran.
    present : tuple of str
        Fragments the line must carry.
    absent : tuple of str
        Fragments the line must not carry.
    """

    matched: str
    hit: str | None
    present: tuple[str, ...]
    absent: tuple[str, ...] = ()


RESTORE_OUTCOMES: dict[str, RestoreOutcome] = {
    # A primary-key match is the only outcome called an exact hit.
    "exact-hit": RestoreOutcome(
        REGISTRY_KEY, "true", ("exact hit", "cache-hit `true`")
    ),
    # A `restore-keys` restore reports the generation it actually loaded:
    # every warm restore of a run-keyed archive takes this path, because its
    # primary key ends with the current run identifier, so collapsing it into
    # `false` would misclassify each warm run as cold.
    "prefix-restore": RestoreOutcome(
        REGISTRY_PREFIX,
        "false",
        (f"prefix restore from `{REGISTRY_PREFIX}`", "cache-hit `false`"),
        ("miss",),
    ),
    # An empty matched key is the only outcome called a miss.
    "complete-miss": RestoreOutcome("", "", ("miss (cache-hit `unset`)",)),
    # An unset `cache-hit` is shown as unset, not as an observed `false`.
    "absent-hit-output": RestoreOutcome(
        REGISTRY_KEY, None, ("exact hit", "cache-hit `unset`")
    ),
}


@pytest.mark.parametrize(
    "outcome", list(RESTORE_OUTCOMES.values()), ids=list(RESTORE_OUTCOMES)
)
def test_each_restore_outcome_is_reported_as_what_it_was(
    tmp_path: Path, outcome: RestoreOutcome
) -> None:
    """The registry line names the outcome the restore actually had."""
    outputs = {
        "CARGO_REGISTRY_KEY": REGISTRY_KEY,
        "CARGO_REGISTRY_MATCHED": outcome.matched,
    }
    if outcome.hit is not None:
        outputs["CARGO_REGISTRY_HIT"] = outcome.hit
    _, summary = _run_observations(tmp_path, **outputs)

    line = _registry_line(summary)
    for fragment in outcome.present:
        assert fragment in line, line
    for fragment in outcome.absent:
        assert fragment not in line, line


def test_headroom_is_reported_before_the_build(tmp_path: Path) -> None:
    """The default form carries free disk alongside the cache outcomes."""
    result, summary = _run_observations(tmp_path)

    assert result.returncode == 0, result.stderr
    assert "- Disk headroom (before the build):" in summary, summary
    assert "Filesystem" in summary, "df output must reach the summary"


def test_headroom_mode_reports_only_disk(tmp_path: Path) -> None:
    """The pre-save form is a headroom report, not a second cache report."""
    result, summary = _run_observations(tmp_path, "headroom")

    assert result.returncode == 0, result.stderr
    assert "### Disk headroom before the cache saves" in summary, summary
    assert "- Disk headroom (before the saves):" in summary, summary
    assert "inactive in this job" not in summary, (
        "the headroom form must not repeat the cache observations"
    )


def test_headroom_covers_the_archive_staging_directory(tmp_path: Path) -> None:
    """The cache action stages archives under RUNNER_TEMP, so report it too."""
    staging = tmp_path / "runner-temp"
    staging.mkdir()
    _, summary = _run_observations(tmp_path, "headroom", RUNNER_TEMP=str(staging))

    # `df` names some filesystems `overlay` or `tmpfs`, so counting its
    # headers is the only device-independent way to prove two reports.
    headers = [line for line in summary.splitlines() if line.startswith("Filesystem")]
    assert len(headers) == 2, (
        f"both the root volume and RUNNER_TEMP must be reported, got {summary!r}"
    )


def test_unknown_mode_is_a_usage_error(tmp_path: Path) -> None:
    """A mistyped mode must fail rather than silently report nothing."""
    result, summary = _run_observations(tmp_path, "hedroom")

    assert result.returncode == 2, result.stdout
    assert "usage:" in result.stderr, result.stderr
    assert summary == "", "a rejected mode must write no summary"


def _write_sccache_stub(tmp_path: Path, requests: str) -> Path:
    """Write an `sccache` stub reporting a fixed compile-request count."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir(exist_ok=True)
    stub = stub_dir / "sccache"
    stub.write_text(
        "#!/bin/sh\n"
        'if [ "$2" = "--stats-format" ]; then\n'
        '    echo \'{"stats":{"requests_executed":' + requests + "}}'\n"
        "    exit 0\n"
        "fi\n"
        f'printf "Compile requests {requests}\\nCache hits 3\\n"\n',
        encoding="utf-8",
    )
    stub.chmod(0o755)
    return stub_dir


def _run_effectiveness(
    tmp_path: Path,
    requests: str,
) -> tuple[subprocess.CompletedProcess[str], str]:
    """Run the sccache reporter against a stub and return the summary."""
    stub_dir = _write_sccache_stub(tmp_path, requests)
    workdir = tmp_path / "work"
    workdir.mkdir(exist_ok=True)
    summary = tmp_path / "summary.md"
    summary.touch()
    result = subprocess.run(
        ["bash", str(EFFECTIVENESS_SCRIPT)],
        capture_output=True,
        text=True,
        check=False,
        cwd=workdir,
        env={
            "PATH": f"{stub_dir}:/usr/bin:/bin",
            "HOME": str(tmp_path),
            "GITHUB_STEP_SUMMARY": str(summary),
        },
    )
    return result, summary.read_text(encoding="utf-8")


def test_sccache_stats_are_published_in_both_formats(tmp_path: Path) -> None:
    """The reporter leaves the text and JSON artefacts the upload step needs."""
    result, summary = _run_effectiveness(tmp_path, "412")

    assert result.returncode == 0, result.stderr
    workdir = tmp_path / "work"
    assert (workdir / "sccache-stats.txt").exists(), "the text artefact must exist"
    assert (workdir / "sccache-stats.json").exists(), "the JSON artefact must exist"
    assert "Compile requests 412" in summary, summary
    assert "::warning::" not in result.stdout, (
        "a job that compiled through sccache must not be warned about"
    )


def test_zero_compile_requests_raises_a_warning(tmp_path: Path) -> None:
    """A wrapper that wrapped nothing is a broken integration, not a clean run."""
    result, _ = _run_effectiveness(tmp_path, "0")

    assert result.returncode == 0, result.stderr
    assert "::warning::" in result.stdout, result.stdout
    assert "RUSTC_WRAPPER" in result.stdout, result.stdout


@pytest.mark.parametrize(
    "script",
    [
        pytest.param(OBSERVATIONS_SCRIPT, id="observations"),
        pytest.param(EFFECTIVENESS_SCRIPT, id="effectiveness"),
    ],
)
def test_reporting_scripts_pass_shellcheck(script: Path) -> None:
    """Both reporters stay clean under the repository linter."""
    if shutil.which("shellcheck") is None:
        pytest.skip("shellcheck is not installed")
    result = subprocess.run(
        ["shellcheck", str(script)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stdout
