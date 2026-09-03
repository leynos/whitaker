"""Property tests for the cache ownership and key-family invariants.

The concrete contract tests assert that today's workflows obey the cache
design. These tests assert that the helpers deciding whether a workflow
obeys it hold over generated step lists, key families, and orderings, so a
future edit cannot slip past the checks by reordering steps, splitting a
path block, or introducing a key family that is a prefix of another.

Invariants covered:

- pairwise-disjoint paths never report an ownership conflict;
- a path claimed by two steps is always reported, naming both steps;
- the reported conflicts do not depend on step order;
- a rendered key belongs to a family exactly when some family prefixes it;
  and
- the longest matching family wins, so a family that prefixes another never
  captures the more specific one.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

from __future__ import annotations

import typing as typ

from hypothesis import given
from hypothesis import strategies as st
from ubicloud_workflow_support import duplicate_path_owners, key_family

PATHS = (
    st.text(
        alphabet="abcdefghijklmnopqrstuvwxyz-/~.",
        min_size=1,
        max_size=12,
    )
    .map(str.strip)
    .filter(bool)
)

STEP_NAMES = st.text(
    alphabet="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz ",
    min_size=1,
    max_size=10,
)

KEY_FAMILIES = st.text(alphabet="abcdefghij-", min_size=1, max_size=8)


def _step(name: str, paths: list[str]) -> dict[str, typ.Any]:
    """Build a cache step claiming the supplied paths, as YAML would."""
    return {"name": name, "with": {"path": "\n".join(paths)}}


@given(
    names=st.lists(STEP_NAMES, min_size=0, max_size=5, unique=True),
    data=st.data(),
)
def test_disjoint_claims_never_conflict(
    names: list[str],
    data: st.DataObject,
) -> None:
    """Steps whose paths are pairwise disjoint have exactly one owner each."""
    pool = data.draw(st.lists(PATHS, min_size=len(names), max_size=15, unique=True))
    steps = [_step(name, [path]) for name, path in zip(names, pool, strict=False)]

    assert duplicate_path_owners(steps) == {}, (
        "disjoint claims must never be reported as a conflict"
    )


@given(
    shared=PATHS,
    names=st.lists(STEP_NAMES, min_size=2, max_size=2, unique=True),
    extra=st.lists(PATHS, max_size=4, unique=True),
)
def test_a_shared_path_is_always_reported(
    shared: str,
    names: list[str],
    extra: list[str],
) -> None:
    """Two steps claiming one path are always caught, whatever else they claim."""
    first, second = names
    others = [path for path in extra if path != shared]
    steps = [
        _step(first, [shared, *others]),
        _step(second, [*others, shared]),
    ]

    conflicts = duplicate_path_owners(steps)

    assert shared in conflicts, f"{shared} must be reported as doubly owned"
    assert sorted(conflicts[shared]) == sorted([first, second]), (
        f"both claimants must be named, got {conflicts[shared]}"
    )
    assert all(path in conflicts for path in others), (
        "a path both steps claim must also be reported"
    )


@given(
    claims=st.lists(
        st.tuples(STEP_NAMES, st.lists(PATHS, min_size=1, max_size=3)),
        min_size=2,
        max_size=5,
    ),
    permutation=st.randoms(use_true_random=False),
)
def test_conflicts_do_not_depend_on_step_order(
    claims: list[tuple[str, list[str]]],
    permutation: typ.Any,
) -> None:
    """Ownership is a property of the step set, not of its declaration order."""
    steps = [_step(name, paths) for name, paths in claims]
    shuffled = list(steps)
    permutation.shuffle(shuffled)

    original = {
        path: sorted(owners) for path, owners in duplicate_path_owners(steps).items()
    }
    reordered = {
        path: sorted(owners) for path, owners in duplicate_path_owners(shuffled).items()
    }

    assert original == reordered, "reordering steps must not change the conflicts"


@given(
    families=st.lists(KEY_FAMILIES, min_size=1, max_size=6, unique=True),
    suffix=st.text(alphabet="abcdefghij-", max_size=8),
    data=st.data(),
)
def test_a_rendered_key_resolves_to_its_longest_family(
    families: list[str],
    suffix: str,
    data: st.DataObject,
) -> None:
    """A key belongs to the most specific family that prefixes it."""
    chosen = data.draw(st.sampled_from(families))
    key = chosen + suffix

    resolved = key_family(key, families)

    assert resolved is not None, f"{key} must resolve to a family"
    assert key.startswith(resolved), f"{resolved} must prefix {key}"
    longest = max(
        (family for family in families if key.startswith(family)),
        key=len,
    )
    assert resolved == longest, (
        f"the most specific family must win, expected {longest}, got {resolved}"
    )


@given(
    families=st.lists(KEY_FAMILIES, min_size=1, max_size=6, unique=True),
    key=st.text(max_size=10),
)
def test_an_unclaimed_key_resolves_to_no_family(
    families: list[str],
    key: str,
) -> None:
    """A key no family prefixes is unreviewed, never silently attributed."""
    resolved = key_family(key, families)

    if any(key.startswith(family) for family in families):
        assert resolved is not None, f"{key} is claimed and must resolve"
    else:
        assert resolved is None, f"{key} must not be attributed to {resolved}"
