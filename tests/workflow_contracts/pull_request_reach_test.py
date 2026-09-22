"""The pull-request lane as a closure over triggers and local calls.

Every boundary rule ranges over `pull_request_closure`, so a workflow the
closure misses escapes all of them at once. The probe below is the shape
measured on episodic: a `workflow_call`-only workflow, called from a
pull-request job with `secrets: inherit`, curling CodeScene with the inherited
credential. A reading that enumerated triggers alone passed it.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_boundary import coverage_surface_offenders
from pull_request_reach import (
    declares_trigger,
    is_reachable_by_a_pull_request,
    local_call,
    pull_request_closure,
)
from ubicloud_workflow_support import parse_workflow

#: A reusable workflow reaching CodeScene with whatever secret it was handed.
#: It names no CodeScene action and runs no `cs-coverage`.
PROBE: typ.Final[str] = (
    "on:\n  workflow_call:\n"
    "jobs:\n  probe:\n    runs-on: ubuntu-latest\n    steps:\n"
    "      - run: |\n"
    '          curl -H "Authorization: ${{ secrets.CS_ACCESS_TOKEN }}" \\\n'
    "            https://api.codescene.io/v2/projects/1\n"
)


def _caller(reference: str) -> str:
    """Return a pull-request workflow calling one reusable workflow."""
    return (
        "on:\n  pull_request:\n"
        f"jobs:\n  call:\n    uses: {reference}\n    secrets: inherit\n"
    )


def _parsed(text: str) -> dict[str, typ.Any]:
    """Parse one synthetic workflow strictly."""
    parsed = parse_workflow(text)
    assert isinstance(parsed, dict), "the synthetic workflow must parse to a mapping"
    return parsed


def test_the_probe_is_reported_once_the_closure_reaches_it() -> None:
    """The measurement the closure rests on, in both directions.

    The trigger-only reading reaches `ci.yml` alone. The closure adds the
    probe, and the boundary then reports the caller's `secrets: inherit` and
    the probe's credential, and its host.
    """
    texts = {
        "ci.yml": _caller("./.github/workflows/probe.yml"),
        "probe.yml": PROBE,
    }
    documents = {name: _parsed(text) for name, text in texts.items()}
    trigger_only = sorted(
        name
        for name, document in documents.items()
        if is_reachable_by_a_pull_request(document)
    )
    assert trigger_only == ["ci.yml"], "the premise: triggers alone miss the probe"
    closure = pull_request_closure(documents)
    assert closure == frozenset({"ci.yml", "probe.yml"}), (
        f"the called workflow runs on a pull request; the closure read {closure}"
    )
    probe_offences = coverage_surface_offenders(
        "probe.yml", documents["probe.yml"], PROBE
    )
    assert any("raw text names codescene.io" in o for o in probe_offences), (
        probe_offences
    )
    assert any("parsed value" in o for o in probe_offences), probe_offences
    caller_offences = coverage_surface_offenders(
        "ci.yml", documents["ci.yml"], texts["ci.yml"]
    )
    assert any("secrets: inherit" in o for o in caller_offences), caller_offences


@pytest.mark.parametrize(
    "prefix",
    [pytest.param("./", id="dot-slash"), pytest.param("$/", id="dollar-slash")],
)
def test_the_closure_follows_both_documented_spellings(prefix: str) -> None:
    """GitHub documents `./` and `$/` for a same-repository call.

    Parametrized rather than combined, so each fails on its own; `$/` is the
    spelling GitHub recommends, and a reader knowing only `./` would drop it.
    """
    documents = {
        "ci.yml": _parsed(_caller(f"{prefix}.github/workflows/probe.yml")),
        "probe.yml": _parsed(PROBE),
    }
    assert pull_request_closure(documents) == frozenset(documents), (
        f"a call spelt with {prefix!r} reaches the called workflow"
    )


def test_the_closure_is_transitive() -> None:
    """A call two levels down is as reachable as one level down."""
    documents = {
        "ci.yml": _parsed(_caller("./.github/workflows/middle.yml")),
        "middle.yml": _parsed(
            "on: workflow_call\njobs:\n  a:\n    uses: ./.github/workflows/probe.yml\n"
        ),
        "probe.yml": _parsed(PROBE),
    }
    assert pull_request_closure(documents) == frozenset(documents), (
        "every workflow on the call chain from a pull request is in the lane"
    )


def test_a_workflow_nothing_calls_is_not_in_the_lane() -> None:
    """The closure is narrow as well as transitive.

    Sweeping in every `workflow_call` document would hold workflows the lane
    never runs to the lane's rules, and fail a repository that complies.
    """
    documents = {
        "ci.yml": _parsed("on: pull_request\njobs:\n  a:\n    steps: []\n"),
        "probe.yml": _parsed(PROBE),
    }
    assert pull_request_closure(documents) == frozenset({"ci.yml"})


def test_a_call_to_a_missing_file_is_not_followed() -> None:
    """A reference with no document here adds nothing, and does not raise."""
    documents = {"ci.yml": _parsed(_caller("./.github/workflows/absent.yml"))}
    assert pull_request_closure(documents) == frozenset({"ci.yml"})


@pytest.mark.parametrize(
    ("reference", "expected"),
    [
        pytest.param("./.github/workflows/probe.yml", "probe.yml", id="dot-slash"),
        pytest.param("$/.github/workflows/probe.yml", "probe.yml", id="dollar-slash"),
        pytest.param(".github/workflows/probe.yml", "probe.yml", id="bare"),
        pytest.param(".github//workflows/./probe.yml", "probe.yml", id="odd-spelling"),
        pytest.param(
            "leynos/shared-actions/.github/workflows/probe.yml@abc",
            None,
            id="another-repository",
        ),
        pytest.param("./.github/actions/probe", None, id="a-local-action"),
        pytest.param("./.github/workflows/sub/probe.yml", None, id="a-subdirectory"),
        pytest.param("$/elsewhere/.github/workflows/probe.yml", None, id="dollar-elsewhere"),
    ],
)
def test_a_local_call_is_recognized_by_shape(
    reference: str, expected: str | None
) -> None:
    """What counts as a call into this repository's own workflows.

    Read as a path rather than matched against a list of prefixes, so any
    spelling that resolves under the workflow directory is local, and GitHub's
    documented `$/` same-commit prefix with it. The last four rows are the
    narrow half: another repository, a local action, and paths GitHub would
    not accept as a reusable workflow all stay out.
    """
    assert local_call(reference) == expected


@pytest.mark.parametrize(
    ("declaration", "reachable"),
    [
        pytest.param("on: pull_request\n", True, id="a-scalar-trigger"),
        pytest.param("on: [push, pull_request]\n", True, id="a-sequence-trigger"),
        pytest.param("on:\n  pull_request:\n", True, id="a-mapping-trigger"),
        pytest.param('"on": pull_request\n', True, id="a-quoted-key"),
        pytest.param(
            "on:\n  pull_request_target:\n", True, id="the-privileged-variant"
        ),
        pytest.param("on:\n  workflow_run:\n", True, id="a-resumed-run"),
        pytest.param("on:\n  push:\n", False, id="a-push-lane"),
        pytest.param("on:\n  schedule:\n", False, id="a-scheduled-lane"),
    ],
)
def test_the_trigger_reading_accepts_every_shape_on_takes(
    declaration: str, reachable: bool
) -> None:
    """`on:` has three shapes, and two keys once PyYAML has read it.

    A bare `on` parses as the boolean True; a quoted one stays the string. A
    reading that knew only the mapping form, or only one key, would call a
    lane unreachable and exempt it from the whole rule.
    """
    parsed = _parsed(f"{declaration}jobs:\n  a:\n    steps: []\n")
    assert is_reachable_by_a_pull_request(parsed) is reachable, (
        f"{declaration!r} must read as reachable={reachable}"
    )
    assert declares_trigger(parsed, "pull_request") is (
        "pull_request" in declaration and "pull_request_target" not in declaration
    )
