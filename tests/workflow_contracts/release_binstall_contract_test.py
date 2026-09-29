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

#: The step that proves the extracted binary runs, not merely that it exists.
VERSION_COMMAND: typ.Final[str] = "whitaker-installer --version"

#: The exact cargo-binstall release the job installs, so a new binstall
#: cannot change what the check means without a reviewed edit here.
BINSTALL_TOOL: typ.Final[str] = "cargo-binstall@1.16.6"

#: The reviewed, SHA-pinned action that installs binstall. An unrelated action
#: naming the same tool would satisfy a check on the tool alone.
BINSTALL_ACTION: typ.Final[str] = (
    "taiki-e/install-action@18b1216eba7f8039b0f8d131d5473787f0edce68"
)

#: The job's condition: after a successful publish, and not after a cancel.
VERIFY_CONDITION: typ.Final[str] = (
    "${{ !cancelled() && needs.publish.result == 'success' }}"
)


def _verify_job(workflow: dict[str, typ.Any]) -> dict[str, typ.Any]:
    """Return the `verify-binstall` job, failing when it is absent."""
    jobs = workflow.get("jobs") or {}
    assert "verify-binstall" in jobs, "release.yml has no verify-binstall job"
    return jobs["verify-binstall"]


def _header_violations(job: dict[str, typ.Any]) -> list[str]:
    """Return why a job would not run after, and only after, a publish."""
    violations: list[str] = []
    if job.get("needs") != "publish":
        violations.append(f"the job must need publish, not {job.get('needs')!r}")
    if job.get("if") != VERIFY_CONDITION:
        violations.append(f"the job condition must be {VERIFY_CONDITION!r}")
    return violations


def _is_pinned_installer(step: dict[str, typ.Any]) -> bool:
    """Report whether a step uses the pinned action with the fallback disabled."""
    return (
        step.get("uses") == BINSTALL_ACTION
        and (step.get("with") or {}).get("fallback") == "none"
    )


def _tool_violations(steps: list[dict[str, typ.Any]]) -> list[str]:
    """Return why no step installs binstall with the pinned action."""
    installers = [
        step for step in steps if (step.get("with") or {}).get("tool") == BINSTALL_TOOL
    ]
    if installers and all(_is_pinned_installer(step) for step in installers):
        return []
    return [
        f"no step installs {BINSTALL_TOOL} with the pinned action and fallback none"
    ]


def _command_violations(steps: list[dict[str, typ.Any]], command: str) -> list[str]:
    """Return why no unconditional step runs `command` as its sole command."""
    runs = [
        step
        for step in steps
        if runs_unconditionally(str(step.get("run", "")), command)
    ]
    if not runs:
        return [f"no step runs {command!r} as its sole command"]
    return [
        f"the {command!r} step is conditional: {step['if']!r}"
        for step in runs
        if "if" in step
    ]


def binstall_violations(job: dict[str, typ.Any]) -> list[str]:
    """Return why a job does not prove the published archive installs.

    Parameters
    ----------
    job
        A workflow job mapping, as loaded from `release.yml`.

    Returns
    -------
    list[str]
        One message per way the job fails to install from the archive; empty
        when the job has the reviewed shape.

    Examples
    --------
    >>> binstall_violations({"needs": "publish", "if": VERIFY_CONDITION,
    ...     "steps": [{"uses": BINSTALL_ACTION,
    ...                "with": {"tool": BINSTALL_TOOL, "fallback": "none"}},
    ...               {"run": BINSTALL_COMMAND},
    ...               {"run": VERSION_COMMAND}]})
    []
    """
    steps = job.get("steps") or []
    return (
        _header_violations(job)
        + _tool_violations(steps)
        + _command_violations(steps, BINSTALL_COMMAND)
        + _command_violations(steps, VERSION_COMMAND)
    )


def test_the_release_verifies_its_archive_with_binstall() -> None:
    """The checked-in release job installs from the archive or fails."""
    violations = binstall_violations(_verify_job(load_workflow("release.yml")))
    assert not violations, f"release.yml verify-binstall: {violations}"


_GOOD: typ.Final = {
    "needs": "publish",
    "if": VERIFY_CONDITION,
    "steps": [
        {
            "uses": BINSTALL_ACTION,
            "with": {"tool": BINSTALL_TOOL, "fallback": "none"},
        },
        {"run": BINSTALL_COMMAND},
        {"run": VERSION_COMMAND},
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
            _GOOD
            | {
                "steps": [
                    _GOOD["steps"][0] | {"uses": "someone/else@v1"},
                    _GOOD["steps"][1],
                ]
            },
            "pinned action",
            id="an-unrelated-action",
        ),
        pytest.param(
            _GOOD
            | {
                "steps": [
                    _GOOD["steps"][0]
                    | {"with": {"tool": BINSTALL_TOOL, "fallback": "cargo-install"}},
                    _GOOD["steps"][1],
                ]
            },
            "fallback none",
            id="a-fallback-left-enabled",
        ),
        pytest.param(
            _GOOD | {"needs": "build-installer"},
            "must need publish",
            id="runs-before-publish",
        ),
        pytest.param(
            _GOOD | {"steps": _GOOD["steps"][:2]},
            "whitaker-installer --version",
            id="the-installed-binary-is-never-run",
        ),
        pytest.param(
            _GOOD
            | {"steps": [*_GOOD["steps"][:2], {"if": "false", "run": VERSION_COMMAND}]},
            "conditional",
            id="a-conditional-version-check",
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
