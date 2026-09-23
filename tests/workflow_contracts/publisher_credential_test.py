"""CV-005: the publisher binds its credential on the upload step, and only there.

The guard `env.CS_ACCESS_TOKEN != ''` reads the same whether or not the step
binds the variable, because GitHub evaluates a missing property as an empty
string. Delete the binding and the upload skips on every run while the guard
contract still passes. So the binding and the input that carries it are
asserted positively, and every wider or other scope is refused.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_boundary import UPLOAD_COVERAGE_ACTION, action_of
from publisher_credential import binds_the_credential, credential_scopes
from ubicloud_workflow_support import job_steps, load_workflow

#: The one workflow that uploads coverage to CodeScene.
PUBLISHER_WORKFLOW: typ.Final[str] = "coverage-main.yml"

#: The binding and input the upload step carries today.
BOUND: typ.Final[str] = "${{ secrets.CS_ACCESS_TOKEN }}"
PASSED: typ.Final[str] = "${{ env.CS_ACCESS_TOKEN }}"


def _is_upload(step: dict[str, typ.Any]) -> bool:
    """Return whether a step is the CodeScene upload."""
    return action_of(step) == UPLOAD_COVERAGE_ACTION


def test_every_upload_binds_the_credential_and_passes_it_on() -> None:
    """The positive half, over a list checked for content first."""
    document = load_workflow(PUBLISHER_WORKFLOW)
    uploads = [
        step
        for job in (document.get("jobs") or {}).values()
        for step in job_steps(job)
        if _is_upload(step)
    ]
    assert uploads, f"{PUBLISHER_WORKFLOW} must carry the CodeScene upload"
    unbound = [step.get("name") for step in uploads if not binds_the_credential(step)]
    assert not unbound, (
        f"each upload must bind CS_ACCESS_TOKEN to {BOUND} and pass {PASSED} as "
        f"access-token; these do not: {unbound}"
    )


def test_no_other_scope_declares_the_credential() -> None:
    """The negative half: the upload step is the only scope that holds it."""
    scopes = credential_scopes(load_workflow(PUBLISHER_WORKFLOW), _is_upload)
    assert not scopes, f"only the upload step may bind the credential: {scopes}"


@pytest.mark.parametrize(
    "step",
    [
        pytest.param({"with": {"access-token": PASSED}}, id="no-binding"),
        pytest.param(
            {
                "env": {"CS_ACCESS_TOKEN": "${{ secrets.OTHER }}"},
                "with": {"access-token": PASSED},
            },
            id="another-secret",
        ),
        pytest.param(
            {
                "env": {"CS_ACCESS_TOKEN": "secrets.CS_ACCESS_TOKEN"},
                "with": {"access-token": PASSED},
            },
            id="an-unwrapped-reference",
        ),
        pytest.param({"env": {"CS_ACCESS_TOKEN": BOUND}}, id="no-input"),
        pytest.param(
            {
                "env": {"CS_ACCESS_TOKEN": BOUND},
                "with": {"access-token": "${{ env.OTHER }}"},
            },
            id="another-input-value",
        ),
        pytest.param(
            {"env": {"CS_ACCESS_TOKEN": BOUND}, "with": {"token": PASSED}},
            id="another-input-name",
        ),
    ],
)
def test_a_missing_or_wrong_binding_is_refused(step: dict[str, object]) -> None:
    """Each half of the binding can be broken on its own, and each is caught."""
    assert not binds_the_credential(step), f"{step} does not carry the credential"


def test_the_binding_tolerates_expression_whitespace() -> None:
    """The narrow half: GitHub reads `${{secrets.X}}` and `${{ secrets.X }}` alike."""
    step = {
        "env": {"CS_ACCESS_TOKEN": "${{secrets.CS_ACCESS_TOKEN}}"},
        "with": {"access-token": "${{  env.CS_ACCESS_TOKEN  }}"},
    }
    assert binds_the_credential(step)


@pytest.mark.parametrize(
    ("document", "expected"),
    [
        pytest.param(
            {"env": {"CS_ACCESS_TOKEN": BOUND}, "jobs": {}},
            ["workflow"],
            id="workflow-env",
        ),
        pytest.param(
            {"jobs": {"up": {"env": {"CS_ACCESS_TOKEN": BOUND}, "steps": []}}},
            ["job up"],
            id="job-env",
        ),
        pytest.param(
            {
                "jobs": {
                    "up": {
                        "steps": [
                            {"run": "make coverage", "env": {"CS_ACCESS_TOKEN": BOUND}}
                        ]
                    }
                }
            },
            ["job up step 0"],
            id="the-coverage-step",
        ),
        pytest.param(
            {
                "jobs": {
                    "up": {
                        "steps": [
                            {
                                "uses": f"{UPLOAD_COVERAGE_ACTION}@abc",
                                "env": {"CS_ACCESS_TOKEN": BOUND},
                            }
                        ]
                    }
                }
            },
            [],
            id="the-upload-step-itself",
        ),
    ],
)
def test_a_wider_or_other_scope_is_found(
    document: dict[str, object], expected: list[str]
) -> None:
    """A binding anywhere but the upload step puts the secret in more hands."""
    assert credential_scopes(document, _is_upload) == expected
