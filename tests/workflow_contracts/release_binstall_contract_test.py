"""A tagged release proves `cargo binstall` installs from its own archive.

cargo-binstall extracts only regular files and directories from a tar
archive, and skips anything else without an error. The installer archives
were GNU sparse entries, so binstall found no binary and fell back to a
source build that nothing reported (#461). `release.yml`'s `verify-binstall`
job installs the published version with the compile and third-party
strategies disabled, so an archive binstall cannot use fails the release run
instead of compiling quietly. These tests hold the job to that shape.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from shell_commands import runs_unconditionally
from ubicloud_workflow_support import load_workflow

#: The one command that proves the archive installs: every strategy but the
#: release's own archive disabled, at the version the release tag names.
BINSTALL_COMMAND: typ.Final[str] = (
    "cargo binstall --no-confirm --disable-strategies compile,quick-install "
    "whitaker-installer@${RELEASE_TAG#v}"
)

#: The exact cargo-binstall release the job installs, so a new binstall
#: cannot change what the check means without a reviewed edit here.
BINSTALL_TOOL: typ.Final[str] = "cargo-binstall@1.16.6"

#: The job's condition: after a successful publish, and not after a cancel.
VERIFY_CONDITION: typ.Final[str] = (
    "${{ !cancelled() && needs.publish.result == 'success' }}"
)


def _verify_job(workflow: dict[str, typ.Any]) -> dict[str, typ.Any]:
    """Return the `verify-binstall` job, failing when it is absent."""
    jobs = workflow.get("jobs") or {}
    assert "verify-binstall" in jobs, "release.yml has no verify-binstall job"
    return jobs["verify-binstall"]


def binstall_violations(job: dict[str, typ.Any]) -> list[str]:
    """Return why a job does not prove the published archive installs.

    >>> binstall_violations({"needs": "publish", "if": VERIFY_CONDITION,
    ...     "steps": [{"uses": "x", "with": {"tool": BINSTALL_TOOL}},
    ...               {"run": BINSTALL_COMMAND}]})
    []
    """
    violations: list[str] = []
    if job.get("needs") != "publish":
        violations.append(f"the job must need publish, not {job.get('needs')!r}")
    if job.get("if") != VERIFY_CONDITION:
        violations.append(f"the job condition must be {VERIFY_CONDITION!r}")
    steps = job.get("steps") or []
    tools = [(step.get("with") or {}).get("tool") for step in steps]
    if BINSTALL_TOOL not in tools:
        violations.append(f"no step installs {BINSTALL_TOOL}")
    installs = [
        step
        for step in steps
        if runs_unconditionally(str(step.get("run", "")), BINSTALL_COMMAND)
    ]
    if not installs:
        violations.append(f"no step runs {BINSTALL_COMMAND!r} as its sole command")
    violations.extend(
        f"the install step is conditional: {step['if']!r}"
        for step in installs
        if "if" in step
    )
    return violations


def test_the_release_verifies_its_archive_with_binstall() -> None:
    """The checked-in release job installs from the archive or fails."""
    violations = binstall_violations(_verify_job(load_workflow("release.yml")))
    assert not violations, f"release.yml verify-binstall: {violations}"


_GOOD: typ.Final = {
    "needs": "publish",
    "if": VERIFY_CONDITION,
    "steps": [
        {"uses": "taiki-e/install-action", "with": {"tool": BINSTALL_TOOL}},
        {"run": BINSTALL_COMMAND},
    ],
}


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param(
            _GOOD
            | {
                "steps": [
                    _GOOD["steps"][0],
                    {
                        "run": BINSTALL_COMMAND.replace(
                            "compile,quick-install", "quick-install"
                        )
                    },
                ]
            },
            "sole command",
            id="compile-fallback-left-enabled",
        ),
        pytest.param(
            _GOOD
            | {"steps": [_GOOD["steps"][0], {"run": f"{BINSTALL_COMMAND} || true"}]},
            "sole command",
            id="a-failure-swallowed",
        ),
        pytest.param(
            _GOOD
            | {"steps": [_GOOD["steps"][0], {"if": "false", "run": BINSTALL_COMMAND}]},
            "conditional",
            id="a-conditional-install",
        ),
        pytest.param(
            _GOOD
            | {"steps": [{"with": {"tool": "cargo-binstall"}}, _GOOD["steps"][1]]},
            "no step installs",
            id="an-unpinned-binstall",
        ),
        pytest.param(
            _GOOD | {"needs": "build-installer"},
            "must need publish",
            id="runs-before-publish",
        ),
        pytest.param(_GOOD | {"if": "false"}, "condition must be", id="never-runs"),
    ],
)
def test_a_job_that_cannot_prove_the_archive_is_refused(
    job: dict[str, typ.Any], expected: str
) -> None:
    """Each way the check could pass without installing from the archive fails."""
    violations = binstall_violations(job)
    assert any(expected in violation for violation in violations), violations


def test_the_reviewed_shape_is_accepted() -> None:
    """The narrow half: the shape the release uses passes."""
    violations = binstall_violations(_GOOD)
    assert violations == [], f"the reviewed shape was refused: {violations}"
