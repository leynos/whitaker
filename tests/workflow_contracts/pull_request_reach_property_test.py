"""The closure agrees with an independent reachability oracle on any call graph.

`pull_request_closure` is a worklist traversal. The oracle below answers the
same question another way, by Warshall's transitive closure over per-node
reachability sets, so a defect in the traversal's bookkeeping (a node visited twice, a
cycle followed forever, a missing target followed, a caller's callee dropped)
shows up as a disagreement rather than being shared by both.

Generated graphs have trigger roots, branching calls, cycles, self-calls,
disconnected workflows and calls to files that do not exist.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from hypothesis import given, settings
from hypothesis import strategies as st
from pull_request_reach import pull_request_closure

#: The most workflows a generated graph holds. Calls may name up to two more,
#: which have no document and so stand for missing targets.
_MOST_WORKFLOWS: typ.Final[int] = 6
_MISSING_TARGETS: typ.Final[int] = 2

_NODE: typ.Final[st.SearchStrategy[tuple[bool, list[int]]]] = st.tuples(
    st.booleans(),
    st.lists(
        st.integers(min_value=0, max_value=_MOST_WORKFLOWS + _MISSING_TARGETS - 1),
        max_size=3,
    ),
)


def _name(index: int) -> str:
    """Return the file name generated for one node."""
    return f"w{index}.yml"


def _documents(graph: list[tuple[bool, list[int]]]) -> dict[str, dict[str, object]]:
    """Return workflow documents realizing a call graph."""
    return {
        _name(index): {
            True: "pull_request" if is_root else "workflow_call",
            "jobs": {
                f"call-{position}": {"uses": f"./.github/workflows/{_name(target)}"}
                for position, target in enumerate(calls)
            },
        }
        for index, (is_root, calls) in enumerate(graph)
    }


def _oracle(graph: list[tuple[bool, list[int]]]) -> frozenset[str]:
    """Return the reachable workflows by Warshall's transitive closure."""
    size = len(graph)
    reaches = [
        {index} | {target for target in calls if target < size}
        for index, (_, calls) in enumerate(graph)
    ]
    # Warshall's step for each intermediate node: whatever reaches `middle`
    # also reaches everything `middle` reaches.
    for middle in range(size):
        reaches = [row | reaches[middle] if middle in row else row for row in reaches]
    return frozenset(
        _name(end)
        for index, (is_root, _) in enumerate(graph)
        if is_root
        for end in reaches[index]
    )


@settings(max_examples=200, derandomize=True)
@given(graph=st.lists(_NODE, max_size=_MOST_WORKFLOWS))
def test_the_closure_matches_the_oracle(graph: list[tuple[bool, list[int]]]) -> None:
    """Every generated graph, reachability answered two independent ways."""
    closure = pull_request_closure(_documents(graph))
    expected = _oracle(graph)
    assert closure == expected, (
        f"graph {graph}: the traversal reached {sorted(closure)}, the oracle "
        f"{sorted(expected)}"
    )
