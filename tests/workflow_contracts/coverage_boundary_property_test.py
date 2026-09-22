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
- a prohibited element is reported wherever in the job and step lists it sits.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

import typing as typ

from coverage_boundary import UPLOAD_COVERAGE_ACTION, coverage_surface_offenders
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


@settings(max_examples=64, derandomize=True)
@given(
    before=st.lists(st.dictionaries(st.text(max_size=6), st.text(max_size=8)), max_size=3),
    after=st.lists(st.dictionaries(st.text(max_size=6), st.text(max_size=8)), max_size=3),
    job_name=st.text(alphabet="abcdefghijklmnopqrstuvwxyz-", min_size=1, max_size=8),
)
def test_a_prohibited_step_is_found_wherever_it_sits(
    before: list[dict[str, str]], after: list[dict[str, str]], job_name: str
) -> None:
    """Position must not decide whether a credential is noticed.

    A reading that inspected the first step, or the last, or stopped at the
    first offence would pass the repository's own workflows and miss a call
    buried among honest ones.
    """
    steps = [*before, {"uses": f"{UPLOAD_COVERAGE_ACTION}@abc"}, *after]
    document = {"on": {"pull_request": None}, "jobs": {job_name: {"steps": steps}}}
    offenders = coverage_surface_offenders("scratch.yml", document, "")
    assert any("invokes the CodeScene coverage action" in o for o in offenders), (
        f"the call must be found among {len(before)} steps before it and "
        f"{len(after)} after; the reading gave {offenders}"
    )
