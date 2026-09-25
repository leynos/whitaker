"""CV-005: the publisher's credential reaches the upload's input, and no `env`.

The upload is a composite action whose nested steps inherit the calling step's
`env`, so the credential is bound in no `env` anywhere. A check step publishes
only whether it exists, and the upload takes it as its `access-token` input.
Deleting the credential would satisfy the prohibition while the upload skipped
on every run, so the check and the input are asserted positively, and the
credential's mentions are held to exactly those two.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_boundary import UPLOAD_COVERAGE_ACTION, action_of
from publisher_credential import (
    CHECK_COMMAND,
    checks_availability,
    credential_mentions,
    credential_scopes,
    passes_the_credential,
)
from ubicloud_workflow_support import job_steps, load_workflow

#: The one workflow that uploads coverage to CodeScene.
PUBLISHER_WORKFLOW: typ.Final[str] = "coverage-main.yml"

#: The input the upload step carries.
PASSED: typ.Final[str] = "${{ secrets.CS_ACCESS_TOKEN }}"


def _is_upload(step: dict[str, typ.Any]) -> bool:
    """Return whether a step is the CodeScene upload."""
    return action_of(step) == UPLOAD_COVERAGE_ACTION


def _publisher_steps() -> list[list[dict[str, typ.Any]]]:
    """Return each publisher job's steps, one list per job."""
    jobs = load_workflow(PUBLISHER_WORKFLOW).get("jobs") or {}
    return [job_steps(job) for job in jobs.values() if isinstance(job, dict)]


def test_every_upload_takes_the_credential_as_its_input() -> None:
    """The positive half for the upload, over a list checked for content first."""
    uploads = [step for steps in _publisher_steps() for step in steps if _is_upload(step)]
    assert uploads, f"{PUBLISHER_WORKFLOW} must carry the CodeScene upload"
    unpassed = [step.get("name") for step in uploads if not passes_the_credential(step)]
    assert not unpassed, f"each upload must pass {PASSED} as access-token: {unpassed}"


def test_every_upload_follows_an_availability_check_in_its_job() -> None:
    """The guard reads the check's output, so the check must run first."""
    unchecked = [
        step.get("name")
        for steps in _publisher_steps()
        for index, step in enumerate(steps)
        if _is_upload(step) and not any(map(checks_availability, steps[:index]))
    ]
    assert not unchecked, (
        f"each upload must follow a step running exactly {CHECK_COMMAND!r} with "
        f"no if or env: {unchecked}"
    )


def test_no_env_names_the_credential() -> None:
    """The negative half: no workflow, job or step `env` holds it."""
    scopes = credential_scopes(load_workflow(PUBLISHER_WORKFLOW))
    assert not scopes, f"no env may name the credential: {scopes}"


def test_the_credential_appears_exactly_where_it_is_used() -> None:
    """Named in the check's command and the upload's input, and nowhere else."""
    mentions = credential_mentions(load_workflow(PUBLISHER_WORKFLOW))
    assert mentions == sorted([CHECK_COMMAND, PASSED]), mentions


@pytest.mark.parametrize(
    "step",
    [
        pytest.param({"run": CHECK_COMMAND}, id="no-id"),
        pytest.param({"id": "other", "run": CHECK_COMMAND}, id="another-id"),
        pytest.param(
            {"id": "codescene_token", "run": 'echo "available=true" >> "$GITHUB_OUTPUT"'},
            id="another-command",
        ),
        pytest.param(
            {"id": "codescene_token", "run": CHECK_COMMAND, "if": "always()"},
            id="a-condition",
        ),
        pytest.param(
            {"id": "codescene_token", "run": CHECK_COMMAND, "env": {"A": "b"}},
            id="an-env",
        ),
    ],
)
def test_a_changed_check_is_refused(step: dict[str, object]) -> None:
    """Each part of the check can be broken on its own, and each is caught."""
    assert not checks_availability(step), f"{step} is not the availability check"


@pytest.mark.parametrize(
    "step",
    [
        pytest.param({}, id="no-input"),
        pytest.param({"with": {"access-token": "${{ env.CS_ACCESS_TOKEN }}"}}, id="env"),
        pytest.param({"with": {"access-token": "${{ secrets.OTHER }}"}}, id="other"),
        pytest.param({"with": {"token": PASSED}}, id="another-input-name"),
        pytest.param(
            {"with": {"access-token": "secrets.CS_ACCESS_TOKEN"}}, id="unwrapped"
        ),
    ],
)
def test_a_missing_or_wrong_input_is_refused(step: dict[str, object]) -> None:
    """The upload's input must read the secret itself."""
    assert not passes_the_credential(step), f"{step} does not pass the credential"


def test_the_input_tolerates_expression_whitespace() -> None:
    """The narrow half: GitHub reads `${{secrets.X}}` and `${{ secrets.X }}` alike."""
    assert passes_the_credential({"with": {"access-token": "${{secrets.CS_ACCESS_TOKEN}}"}})


@pytest.mark.parametrize(
    ("document", "expected"),
    [
        pytest.param(
            {"env": {"CS_ACCESS_TOKEN": PASSED}, "jobs": {}},
            ["workflow"],
            id="workflow-env",
        ),
        pytest.param(
            {"jobs": {"up": {"env": {"T": "${{ secrets.cs_access_token }}"}}}},
            ["job up"],
            id="job-env-under-another-name",
        ),
        pytest.param(
            {
                "jobs": {
                    "up": {
                        "steps": [
                            {"run": "make coverage"},
                            {
                                "uses": f"{UPLOAD_COVERAGE_ACTION}@abc",
                                "env": {"CS_ACCESS_TOKEN": PASSED},
                            },
                        ]
                    }
                }
            },
            ["job up step 1"],
            id="the-upload-step-itself",
        ),
        pytest.param(
            {"jobs": {"up": {"env": {"A": "b"}, "steps": [{"env": {"C": "d"}}]}}},
            [],
            id="unrelated-env",
        ),
    ],
)
def test_an_env_naming_the_credential_is_found(
    document: dict[str, object], expected: list[str]
) -> None:
    """An `env` at any scope, the upload's included, puts the secret in more hands."""
    assert credential_scopes(document) == expected
