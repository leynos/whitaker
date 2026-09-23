"""CV-005: the publisher uploads only from the trunk, and never cancels.

`coverage-main.yml` answers `workflow_dispatch`, which can name any branch, so
its trigger filter does not confine the upload. The step's own condition must,
and it is read as a conjunction so that a trailing `|| ...` cannot make the ref
test optional while still containing it.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_boundary import UPLOAD_COVERAGE_ACTION, action_of
from publisher_guard import (
    CREDENTIAL_PRESENT_CONJUNCT,
    MAIN_REF_CONJUNCT,
    cancelling_scopes,
    guard_conjuncts,
    is_confined_to_main,
    requires,
)
from ubicloud_workflow_support import job_steps, load_workflow

#: The one workflow that uploads coverage to CodeScene.
PUBLISHER_WORKFLOW: typ.Final[str] = "coverage-main.yml"

#: The guard the publisher's upload step carries today.
DEPLOYED_GUARD: typ.Final[str] = (
    "env.CS_ACCESS_TOKEN != '' && github.ref == 'refs/heads/main'"
)


def _upload_steps() -> list[dict[str, typ.Any]]:
    """Return every CodeScene upload step in the publisher."""
    jobs = load_workflow(PUBLISHER_WORKFLOW).get("jobs") or {}
    return [
        step
        for job in jobs.values()
        if isinstance(job, dict)
        for step in job_steps(job)
        if action_of(step) == UPLOAD_COVERAGE_ACTION
    ]


def test_every_upload_is_confined_to_the_trunk() -> None:
    """The deployed guard, read the way the synthetic cases below prove.

    Asserted over a list checked for content first, so deleting the upload
    cannot satisfy it; the boundary contract separately requires the upload.
    """
    uploads = _upload_steps()
    assert uploads, f"{PUBLISHER_WORKFLOW} must carry the CodeScene upload"
    unguarded = [
        step.get("name") for step in uploads if not is_confined_to_main(step.get("if"))
    ]
    assert not unguarded, (
        f"each upload must run only on refs/heads/main, including when "
        f"dispatched; these are not: {unguarded}"
    )


@pytest.mark.parametrize(
    "conjunct",
    [
        pytest.param(MAIN_REF_CONJUNCT, id="the-trunk"),
        pytest.param(CREDENTIAL_PRESENT_CONJUNCT, id="the-credential"),
    ],
)
def test_every_upload_guard_holds_both_conjuncts(conjunct: str) -> None:
    """The whole guard, not only its ref half.

    The ref test keeps a dispatch from another branch from uploading; the
    credential test skips the upload where the secret is absent, as on a fork
    or after a rotation, instead of failing the publisher. Each is required as
    a conjunct, so neither can be dropped or made optional by an `||`.
    """
    uploads = _upload_steps()
    assert uploads, f"{PUBLISHER_WORKFLOW} must carry the CodeScene upload"
    missing = [
        step.get("name") for step in uploads if not requires(step.get("if"), conjunct)
    ]
    assert not missing, f"each upload guard must require {conjunct!r}: {missing}"


@pytest.mark.parametrize(
    ("condition", "conjunct"),
    [
        pytest.param(
            "github.ref == 'refs/heads/main'",
            CREDENTIAL_PRESENT_CONJUNCT,
            id="no-credential-test",
        ),
        pytest.param(
            "env.CS_ACCESS_TOKEN == ''", CREDENTIAL_PRESENT_CONJUNCT, id="the-inverse"
        ),
        pytest.param(
            f"{MAIN_REF_CONJUNCT} || {CREDENTIAL_PRESENT_CONJUNCT}",
            CREDENTIAL_PRESENT_CONJUNCT,
            id="an-optional-credential-test",
        ),
    ],
)
def test_a_guard_missing_a_required_conjunct_is_refused(
    condition: str, conjunct: str
) -> None:
    """The negative half for the credential test, which the ref cases lack."""
    assert not requires(condition, conjunct), f"{condition!r} lacks {conjunct!r}"


def test_the_publisher_never_cancels_a_run() -> None:
    """A cancelled publisher abandons its upload and its cache writes."""
    scopes = cancelling_scopes(load_workflow(PUBLISHER_WORKFLOW))
    assert not scopes, f"{PUBLISHER_WORKFLOW} must queue, not cancel: {scopes}"


@pytest.mark.parametrize(
    "condition",
    [
        pytest.param(DEPLOYED_GUARD, id="the-deployed-guard"),
        pytest.param(f"${{{{ {DEPLOYED_GUARD} }}}}", id="wrapped"),
        pytest.param(
            "github.ref  ==  'refs/heads/main'  &&  env.CS_ACCESS_TOKEN != ''",
            id="reordered-and-spaced",
        ),
        pytest.param(
            "env.A != '||' && github.ref == 'refs/heads/main'", id="a-quoted-operator"
        ),
    ],
)
def test_a_conjunction_with_the_ref_test_is_confined(condition: str) -> None:
    """The accepting half, so the reader discriminates rather than refuses."""
    assert is_confined_to_main(condition), f"{condition!r} confines the step"


@pytest.mark.parametrize(
    "condition",
    [
        pytest.param(
            f"{DEPLOYED_GUARD} || github.event_name == 'workflow_dispatch'",
            id="a-trailing-disjunction",
        ),
        pytest.param(
            "github.ref == 'refs/heads/main' && env.CS_ACCESS_TOKEN != '' "
            "|| github.event_name == 'workflow_dispatch'",
            id="a-disjunction-after-the-ref-test",
        ),
        pytest.param("env.CS_ACCESS_TOKEN != ''", id="no-ref-test"),
        pytest.param("github.ref != 'refs/heads/main'", id="the-negated-test"),
        pytest.param(
            "github.ref == 'refs/heads/main-backup'", id="a-longer-branch-name"
        ),
        pytest.param(None, id="no-condition"),
    ],
)
def test_a_condition_that_admits_another_ref_is_refused(condition: object) -> None:
    """The ref test must be a conjunct, not a substring.

    The first row is the mutation this reading exists for: it contains the
    ref test verbatim and makes it optional. The second is why `||` is refused
    outright rather than left to the split: with the ref test first, the split
    alone leaves it standing as a conjunct of its own.
    """
    assert not is_confined_to_main(condition), f"{condition!r} admits another ref"


def test_a_disjunction_has_no_conjuncts() -> None:
    """`None`, not a list, so no caller can read a disjunction as a conjunction."""
    assert guard_conjuncts("a || b") is None


@pytest.mark.parametrize(
    ("document", "expected"),
    [
        pytest.param(
            {"concurrency": {"group": "g", "cancel-in-progress": True}},
            ["workflow"],
            id="workflow-level",
        ),
        pytest.param(
            {
                "jobs": {
                    "up": {"concurrency": {"group": "g", "cancel-in-progress": "true"}}
                }
            },
            ["job up"],
            id="job-level-as-a-string",
        ),
        pytest.param(
            {"concurrency": {"group": "g", "cancel-in-progress": "${{ x }}"}},
            ["workflow"],
            id="an-expression",
        ),
        pytest.param(
            {"concurrency": {"group": "g", "cancel-in-progress": False}},
            [],
            id="queues",
        ),
        pytest.param({"concurrency": "g"}, [], id="a-group-name-only"),
        pytest.param({}, [], id="no-group"),
    ],
)
def test_a_cancelling_concurrency_group_is_found(
    document: dict[str, object], expected: list[str]
) -> None:
    """Fail closed: only an explicit false, or no setting, reads as queueing."""
    assert cancelling_scopes(document) == expected
