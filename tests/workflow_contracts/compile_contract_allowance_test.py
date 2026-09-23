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

import collections.abc as cabc
import pathlib
import typing as typ

import pytest
from nextest_profile_test import REQUIRED_OVERRIDES
from rust_functions import compile_contracts_in
from nextest_config import load_nextest_config
from ubicloud_workflow_support import REPOSITORY_ROOT


class RustSourceError(OSError):
    """Raised when the tree's Rust sources cannot be read.

    The discovery below is only as wide as what it was given, so an
    unreadable file or a missing root means it saw fewer tests than the
    tree holds. Named, so that reads as a failure to read rather than as
    a tree with fewer compile contracts.
    """


def rust_source_texts(
    root: pathlib.Path = REPOSITORY_ROOT,
) -> dict[pathlib.Path, str]:
    """Return every Rust source under a root, by path, skipping build output.

    The one place this module touches the filesystem; the discovery is a
    query over what this returns. The whole tree rather than a list of
    directories: a compile contract added under a crate nobody listed
    would otherwise carry no allowance while every assertion over the
    list still passed.

    Parameters
    ----------
    root : pathlib.Path
        The tree to read; the repository by default.

    Returns
    -------
    dict[pathlib.Path, str]
        Each source's path and text, in path order.

    Raises
    ------
    RustSourceError
        If the root is not a directory, or a source cannot be read or is
        not UTF-8.
    """
    # `rglob` yields nothing for a missing root, which would read as a
    # tree without compile contracts rather than as no tree at all.
    if not root.is_dir():
        message = f"{root} is not a directory, so no Rust source was read"
        raise RustSourceError(message)
    texts: dict[pathlib.Path, str] = {}
    for path in sorted(root.rglob("*.rs")):
        if "target" in path.parts or path.name.startswith("."):
            continue
        try:
            texts[path] = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            message = f"cannot read the Rust source {path}: {error}"
            raise RustSourceError(message) from error
    return texts


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
    match slow_timeout:
        case str() as period:
            return period
        case {"period": str() as period}:
            return period
        case _:
            return None


def compile_contract_tests(
    sources: cabc.Mapping[pathlib.Path, str],
) -> dict[str, pathlib.Path]:
    """Return every test in the given sources that drives `trybuild`, by name.

    Found from the call itself rather than from a list, because the
    failure this guards against is a binary nobody remembered: whitaker
    had two, one named in an override and one not, and the unnamed one
    ran under the base allowance until it drifted past 300 s on the
    Windows lane and cancelled the run.

    A call is attributed through the function body that holds it, and a
    helper in the same file passes it on to every test that calls it,
    wherever the helper is declared. A helper in another file is not
    followed, which is one reason the assertion below treats an empty
    result as a failure rather than as nothing to check.
    """
    found: dict[str, pathlib.Path] = {}
    for path, text in sources.items():
        found.update(dict.fromkeys(compile_contracts_in(text), path))
    return found


@pytest.fixture(name="parsed_nextest", scope="module")
def parsed_nextest_fixture() -> dict[str, typ.Any]:
    """Return the nextest configuration, read once at the boundary."""
    return load_nextest_config()


@pytest.fixture(name="rust_sources", scope="module")
def rust_sources_fixture() -> dict[pathlib.Path, str]:
    """Return the tree's Rust sources, read once at the boundary."""
    return rust_source_texts()


@pytest.mark.parametrize(
    "profile", sorted(REQUIRED_OVERRIDES), ids=sorted(REQUIRED_OVERRIDES)
)
def test_every_compile_contract_test_carries_the_long_allowance(
    profile: str,
    parsed_nextest: dict[str, typ.Any],
    rust_sources: dict[pathlib.Path, str],
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
    discovered = compile_contract_tests(rust_sources)
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
        for override in (parsed_nextest["profile"][profile].get("overrides") or [])
        if _period(override.get("slow-timeout")) == "10m"
    ]
    for name, path in sorted(discovered.items()):
        assert any(f"test({name})" in one for one in long_filters), (
            f"[profile.{profile}] grants no long allowance to the compile "
            f"contract {name!r} in {path.relative_to(REPOSITORY_ROOT)}; it "
            f"would run under the base per-test budget, which is sized for "
            f"a test rather than for a nested cargo build"
        )
