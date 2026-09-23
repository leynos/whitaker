"""Properties of the coverage-boundary scanner over shapes nobody wrote.

The contract tests assert what the scanner says about this repository's
workflows and about documents written to exercise one rule at a time. Neither
says anything about a document the scanner was not designed for, and a
workflow-contract suite has an unusual failure mode: a reader that raises on a
malformed document takes the whole suite down and reports a parse failure where
the question was whether a lane holds a credential.

Invariants covered:

- the scan never raises, whatever nested mapping, list or scalar it is handed,
  including at the places a workflow declares a job, a step and an input;
- a document with no prohibited element is never accused;
- every prohibited step surface is reported wherever in the job and step lists
  it sits;
- a credential or host named only in the raw text is reported wherever in the
  text it sits.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

import typing as typ

from coverage_boundary import (
    COVERAGE_COMMAND,
    CREDENTIAL_ENVIRONMENT_KEY,
    GENERATE_COVERAGE_ACTION,
    PUBLISH_ARTEFACT_ACTION,
    UPLOAD_COVERAGE_ACTION,
    coverage_surface_offenders,
)
from hypothesis import given, settings
from hypothesis import strategies as st

#: Any parsed YAML value, nested a little. The scanner takes whatever
#: `yaml.safe_load` produced, and that is not always the mapping a workflow
#: should be.
_ANY_VALUE: typ.Final[st.SearchStrategy[object]] = st.recursive(
    st.none()
    | st.booleans()
    | st.integers()
    | st.text(max_size=8)
    | st.just("${{ github.workspace }}"),
    lambda children: st.lists(children, max_size=3)
    | st.dictionaries(st.text(max_size=6), children, max_size=3),
    max_leaves=12,
)


#: A document shaped like a workflow, with an arbitrary value at each place a
#: reader reaches into: the `jobs` block, one job, its `steps` list, one step,
#: and a step's `with` block. Generating free-form mappings instead would
#: almost never put a hostile value where a reader looks, and the property
#: would pass with every guard removed; it did, until this strategy replaced
#: it.
_WORKFLOW_SHAPED: typ.Final[st.SearchStrategy[dict[str, object]]] = st.fixed_dictionaries(
    {
        "on": st.just({"pull_request": None}),
        "jobs": _ANY_VALUE
        | st.dictionaries(
            st.text(alphabet="abcdefghijklmnopqrstuvwxyz-", min_size=1, max_size=6),
            _ANY_VALUE
            | st.fixed_dictionaries(
                {
                    "steps": _ANY_VALUE
                    | st.lists(
                        _ANY_VALUE
                        | st.fixed_dictionaries(
                            {"uses": st.text(max_size=12), "with": _ANY_VALUE}
                        ),
                        max_size=3,
                    )
                }
            ),
            max_size=2,
        ),
    }
)


@settings(max_examples=128, derandomize=True)
@given(document=_WORKFLOW_SHAPED)
def test_the_scan_answers_rather_than_raises(document: dict[str, object]) -> None:
    """A malformed document is a finding's absence, not an exception.

    `jobs` holding a list, a step list holding a string, a `with` block that is
    a scalar: all are documents YAML parses and GitHub rejects, and all reach
    this scanner through the sweep over every file in the directory. Raising
    there would report a parse failure in place of the boundary question, and
    would do it for every workflow after the first bad one.
    """
    offenders = coverage_surface_offenders("scratch.yml", document, "")
    assert isinstance(offenders, list), "the scan must answer with a list"
    assert all(isinstance(offence, str) for offence in offenders), (
        "each offence must name itself in text a failure message can carry"
    )


#: An ordinary step: a command with no prohibited element in it.
_CLEAN_STEP: typ.Final[st.SearchStrategy[dict[str, str]]] = st.builds(
    lambda target: {"run": f"make {target}"},
    st.text(alphabet="abcdefghijklmnopqrstuvwxyz-", min_size=1, max_size=8),
)

#: Paths that can carry the report, one per reason the reader refuses them.
_UNSAFE_PATHS: typ.Final[tuple[str, ...]] = (
    "lcov.info",
    ".",
    "../workspace",
    "**/*.info",
    "${{ github.workspace }}",
    "/home/runner",
)

#: The credential as a workflow reads it, rather than as prose names it.
_CREDENTIAL_REFERENCE: typ.Final[str] = f"${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"

#: Every step-shaped surface the boundary prohibits, with the words the scan
#: reports it by. Each is a separate draw, so a detector that fires only for
#: one surface, or only at one position, fails for the others.
_PROHIBITED_STEPS: typ.Final[
    st.SearchStrategy[tuple[dict[str, object], str]]
] = st.one_of(
    st.just(
        ({"uses": f"{UPLOAD_COVERAGE_ACTION}@abc"}, "invokes the CodeScene coverage action")
    ),
    st.just(
        ({"run": f"{COVERAGE_COMMAND} check --format lcov"}, f"runs a {COVERAGE_COMMAND}")
    ),
    st.sampled_from(_UNSAFE_PATHS).map(
        lambda path: (
            {"uses": f"{PUBLISH_ARTEFACT_ACTION}@abc", "with": {"path": path}},
            "publishes the coverage report",
        )
    ),
    st.just(
        (
            {"uses": f"{GENERATE_COVERAGE_ACTION}@abc", "with": {"with-ratchet": "true"}},
            "without declining its own archive",
        )
    ),
    st.just(
        (
            {"run": "make test", "env": {"TOKEN": _CREDENTIAL_REFERENCE}},
            "parsed value references",
        )
    ),
)

_JOB_NAME: typ.Final[st.SearchStrategy[str]] = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyz-", min_size=1, max_size=8
)


def _jobs(
    clean_jobs: list[list[dict[str, str]]], index: int, steps: list[dict[str, object]]
) -> dict[str, object]:
    """Return a `jobs` block with `steps` as the job at `index` among clean ones."""
    ordered = [*clean_jobs[:index], steps, *clean_jobs[index:]]
    return {f"job-{position}": {"steps": job} for position, job in enumerate(ordered)}


@settings(max_examples=64, derandomize=True)
@given(
    clean_jobs=st.lists(st.lists(_CLEAN_STEP, max_size=3), max_size=3),
    steps=st.lists(_CLEAN_STEP, max_size=4),
    index=st.integers(min_value=0, max_value=3),
)
def test_a_document_with_nothing_prohibited_is_never_accused(
    clean_jobs: list[list[dict[str, str]]], steps: list[dict[str, str]], index: int
) -> None:
    """The narrow half, so the detectors discriminate rather than accuse."""
    document = {"on": {"pull_request": None}, "jobs": _jobs(clean_jobs, index, steps)}
    offenders = coverage_surface_offenders("scratch.yml", document, "")
    assert not offenders, f"a clean document must pass; the reading gave {offenders}"


#: Where a step is placed: the clean steps around it, the clean jobs around its
#: own, and its job's position among them.
_PLACEMENT: typ.Final[st.SearchStrategy[dict[str, typ.Any]]] = st.fixed_dictionaries(
    {
        "before": st.lists(_CLEAN_STEP, max_size=3),
        "after": st.lists(_CLEAN_STEP, max_size=3),
        "clean_jobs": st.lists(st.lists(_CLEAN_STEP, max_size=2), max_size=3),
        "job_index": st.integers(min_value=0, max_value=3),
    }
)


@settings(max_examples=128, derandomize=True)
@given(surface=_PROHIBITED_STEPS, placement=_PLACEMENT)
def test_every_prohibited_step_is_found_wherever_it_sits(
    surface: tuple[dict[str, object], str], placement: dict[str, typ.Any]
) -> None:
    """Neither the surface nor its position may decide whether it is noticed.

    A reading that inspected the first step or the first job, or stopped at
    the first offence, or knew only the CodeScene action, would pass the
    repository's own workflows and miss a surface buried among honest steps.
    """
    step, expected = surface
    steps = [*placement["before"], step, *placement["after"]]
    document = {
        "on": {"pull_request": None},
        "jobs": _jobs(placement["clean_jobs"], placement["job_index"], steps),
    }
    offenders = coverage_surface_offenders("scratch.yml", document, "")
    assert any(expected in offence for offence in offenders), (
        f"{step} must be reported as {expected!r} at {placement}; the reading "
        f"gave {offenders}"
    )


#: A line of workflow text that names nothing prohibited.
_CLEAN_LINE: typ.Final[st.SearchStrategy[str]] = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyz #:-_", max_size=24
)


@settings(max_examples=64, derandomize=True)
@given(
    mention=st.sampled_from(
        (
            (f"# {CREDENTIAL_ENVIRONMENT_KEY} lives in main", "raw text references"),
            ("# see https://codescene.io/projects", "raw text names"),
        )
    ),
    before=st.lists(_CLEAN_LINE, max_size=6),
    after=st.lists(_CLEAN_LINE, max_size=6),
)
def test_a_raw_text_only_mention_is_found_wherever_it_sits(
    mention: tuple[str, str], before: list[str], after: list[str]
) -> None:
    """The raw-text reader, on its own and at any line.

    The parsed document is clean, so only the raw scan can report this. It is
    the shape of a credential or host in a comment, which the parser drops.
    """
    line, expected = mention
    raw = "\n".join([*before, line, *after])
    document = {
        "on": {"pull_request": None},
        "jobs": {"a": {"steps": [{"run": "make test"}]}},
    }
    offenders = coverage_surface_offenders("scratch.yml", document, raw)
    assert any(expected in offence for offence in offenders), (
        f"{line!r} must be reported as {expected!r}; the reading gave {offenders}"
    )
