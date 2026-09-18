"""Every `trybuild` compile contract carries a build-sized allowance.

Split from ``nextest_profile_test`` so neither module outgrows the
400-line limit ``AGENTS.md`` sets. That module pins the profiles and
their overrides against the values the guide records; this one asks a
different question of the same configuration, and asks it of the tree
rather than of a list: which tests in this repository invoke a nested
cargo build, and does each of them have an allowance sized for one.

The discovery is the point. A handwritten set stops recognizing a
binary silently, and the binary then loses its allowance while every
assertion over the set still passes. That is how
``whitaker-installer``'s compile contract came to run under the base
allowance until it drifted past it on the Windows lane.
"""

from __future__ import annotations

import pathlib
import re
import typing as typ

import pytest
from nextest_profile_test import NEXTEST, REQUIRED_OVERRIDES
from ubicloud_workflow_support import REPOSITORY_ROOT


#: The directory holding the crates a `trybuild::TestCases` may live in.
#: Discovered rather than listed: a handwritten set stops recognizing a
#: binary silently, and the binary then loses its allowance while every
#: assertion over the set still passes.
_TRYBUILD_CALL: typ.Final[re.Pattern[str]] = re.compile(r"trybuild::TestCases::new\(\)")

#: The name a `#[test]` function declares. Applied to one function's
#: own text, never to a window of fixed size: an earlier test's window
#: reaches into the next function and claims its call.
_TEST_FUNCTION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(?P<name>\w+)\s*\(",
    re.MULTILINE,
)


def _rust_sources() -> typ.Iterator[pathlib.Path]:
    """Yield every Rust source in the tree, skipping build output.

    The whole tree rather than a list of directories: a compile contract
    added under a crate nobody listed would otherwise carry no allowance
    while every assertion over the list still passed.
    """
    for path in sorted(REPOSITORY_ROOT.rglob("*.rs")):
        if "target" not in path.parts and not path.name.startswith("."):
            yield path


def _named_compile_contracts(text: str) -> typ.Iterator[str]:
    """Yield the name of each test in one file whose body drives `trybuild`.

    Each `#[test]` attribute starts a region that ends where the next
    one begins, and a call is attributed to the function whose region
    holds it. A window of fixed size would reach into the next function
    and claim its call.
    """
    for region in text.split("#[test]")[1:]:
        if not _TRYBUILD_CALL.search(region):
            continue
        name = _TEST_FUNCTION.search(region)
        if name is not None:
            yield name["name"]


def _period(slow_timeout: object) -> str | None:
    """Return the period a `slow-timeout` names, whatever shape it takes.

    nextest accepts both `slow-timeout = "10m"` and the table form, and
    the two mean the same period. Reading only the table would drop a
    real allowance written the short way, and a contract that stops
    counting an override that exists is worse than one that fails: it
    answers wrongly instead of loudly.

    `None` for anything that names no period, which is how an override
    that declares no `slow-timeout` at all reaches this.

    Examples
    --------
    >>> _period({"period": "10m", "terminate-after": 1})
    '10m'
    >>> _period("10m")
    '10m'
    >>> _period(None) is None
    True
    """
    if isinstance(slow_timeout, str):
        return slow_timeout
    if isinstance(slow_timeout, dict):
        period = slow_timeout.get("period")
        return period if isinstance(period, str) else None
    return None


def _compile_contract_tests() -> dict[str, pathlib.Path]:
    """Return every test that drives `trybuild`, by name.

    Found from the call itself rather than from a list, because the
    failure this guards against is a binary nobody remembered: whitaker
    had two, one named in an override and one not, and the unnamed one
    ran under the base allowance until it drifted past 300 s on the
    Windows lane and cancelled the run.

    A call inside a helper rather than inside the test body would be
    missed, which is why the assertion below treats an empty result as a
    failure rather than as nothing to check.
    """
    found: dict[str, pathlib.Path] = {}
    for path in _rust_sources():
        text = path.read_text(encoding="utf-8")
        found.update(dict.fromkeys(_named_compile_contracts(text), path))
    return found


@pytest.mark.parametrize(
    "profile", sorted(REQUIRED_OVERRIDES), ids=sorted(REQUIRED_OVERRIDES)
)
def test_every_compile_contract_test_carries_the_long_allowance(
    profile: str,
) -> None:
    """A `trybuild` test costs what a build costs, not what a test costs.

    Each one invokes a nested cargo build, so the base per-test
    allowance is the wrong order of magnitude for it. The set is
    discovered from the `trybuild::TestCases` calls in the tree rather
    than written down here, because the fault this exists to catch is a
    binary nobody remembered: `whitaker-installer`'s test is in a binary
    named `ui` but is not itself named `ui`, so the
    `(binary(ui) & test(=ui))` clause never reached it and it ran under
    the base 300 s allowance, passing at 235 s and timing out at 300.216 s
    on the Windows lane.

    The assertion is that the test's own name appears in a filter that
    grants the long allowance, which is what a `test(...)` clause matches
    on. Matching by binary would not do: the two binaries here are both
    called `ui`.
    """
    discovered = _compile_contract_tests()
    assert discovered, (
        "no trybuild::TestCases call was found; the discovery has stopped "
        "recognizing compile-contract tests and would sweep an empty set"
    )
    # `get` on both keys rather than indexing: nextest allows an override
    # with no `filter`, which selects on platform alone, and such an
    # override names no test whatever its allowance. An empty string
    # matches no `test(...)` clause below, which is the same answer a
    # crash would have hidden.
    long_filters = [
        override.get("filter", "")
        for override in (NEXTEST["profile"][profile].get("overrides") or [])
        if _period(override.get("slow-timeout")) == "10m"
    ]
    for name, path in sorted(discovered.items()):
        assert any(f"test({name})" in one for one in long_filters), (
            f"[profile.{profile}] grants no long allowance to the compile "
            f"contract {name!r} in {path.relative_to(REPOSITORY_ROOT)}; it "
            f"would run under the base per-test budget, which is sized for "
            f"a test rather than for a nested cargo build"
        )
