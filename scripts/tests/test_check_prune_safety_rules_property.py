"""Property tests for the prune-safety checker's pure rules.

The parameterised examples in ``test_check_prune_safety_rules`` pin the
contract; these properties cover the broader input space the naming and
parsing rules accept. Run with ``make test-prune-safety``.
"""

from __future__ import annotations

from hypothesis import given
from hypothesis import strategies as st

from prune_safety_support import rules

CRATE = st.from_regex(r"[A-Za-z0-9_-]+", fullmatch=True)
ENTRY = st.from_regex(r"[A-Za-z0-9_.@-]+", fullmatch=True)
TARGET = st.sampled_from(
    [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ]
)
TOOLCHAIN = st.sampled_from(["stable", "1.88.0", "nightly-2026-05-28"])
SHA = st.from_regex(r"[0-9a-f]{7,40}", fullmatch=True)


@given(crate=CRATE, target=TARGET)
def test_canonical_library_names_round_trip(crate: str, target: str) -> None:
    """A canonical staged name yields exactly the crate it was built from."""
    prefix, extension = rules.library_affixes(target)
    assert rules.crate_of_entry(f"{prefix}{crate}{extension}", target) == crate


@given(entry=ENTRY, target=TARGET)
def test_names_without_the_targets_affixes_survive_whole(
    entry: str, target: str
) -> None:
    """A name without the target's exact affixes is never pruned."""
    prefix, extension = rules.library_affixes(target)
    if not (entry.startswith(prefix) and entry.endswith(extension)):
        assert rules.crate_of_entry(entry, target) == entry


@given(entries=st.lists(CRATE, max_size=10), known=st.lists(CRATE, max_size=10))
def test_surviving_crates_are_sorted_and_exclude_known(
    entries: list[str], known: list[str]
) -> None:
    """The prune leaves one sorted copy of each crate it does not know."""
    names = [f"lib{entry}.so" for entry in entries]
    left = rules.surviving_crates(names, known, "x86_64-unknown-linux-gnu")
    assert left == sorted(set(left))
    assert set(left).isdisjoint(known)


@given(names=st.lists(CRATE, min_size=1, max_size=5))
def test_parse_lint_crates_returns_every_name(names: list[str]) -> None:
    """Every crate in ``LINT_CRATES`` is returned, in declaration order."""
    literals = ", ".join(f'"{name}"' for name in names)
    source = f"pub const LINT_CRATES: &[&str] = &[{literals}];"
    assert rules.parse_lint_crates(source) == names


@given(sha=SHA, toolchain=TOOLCHAIN, target=TARGET)
def test_archive_names_round_trip(sha: str, toolchain: str, target: str) -> None:
    """An archive name splits back into the parts it was built from."""
    name = f"whitaker-lints-{sha}-{toolchain}-{target}.tar.zst"
    assert rules.parse_archive_name(name) == {
        "sha": sha,
        "toolchain": toolchain,
        "target": target,
    }
