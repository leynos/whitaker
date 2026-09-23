"""The boundaries that read the timeout contracts' inputs, and what they refuse.

The nextest configuration and the tree's Rust sources are each read at one
function, and the queries over them take what those functions return. These
cases drive both boundaries over inputs the repository does not have: a file
that is missing, one that is a directory, one that is not UTF-8, and one that
is not TOML. Each must fail with the named error, naming the file, rather than
with whatever exception the failure happened to produce at collection.

They also drive the compile-contract discovery over sources built for it,
since over the tree alone a discovery that answered nothing new would agree
with a correct one.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib

import pytest
from compile_contract_allowance_test import (
    RustSourceError,
    compile_contract_tests,
    rust_source_texts,
)
from nextest_config import load_nextest_config, nextest_config_text
from nextest_durations import NextestConfigurationError

#: A configuration nextest would accept, for the one accepted case.
_VALID_CONFIG = '[profile.default]\nslow-timeout = "300s"\n'


@pytest.mark.parametrize(
    ("name", "content"),
    [
        pytest.param("absent.toml", None, id="missing"),
        pytest.param("a-directory", "directory", id="a-directory"),
        pytest.param("latin1.toml", b"\xff\xfe = 1\n", id="not-utf-8"),
        pytest.param("broken.toml", b"[profile.default\n", id="not-toml"),
    ],
)
def test_an_unusable_configuration_is_a_named_error(
    tmp_path: pathlib.Path, name: str, content: bytes | str | None
) -> None:
    """Each failure crosses the boundary as the one error, naming the file."""
    path = tmp_path / name
    if content == "directory":
        path.mkdir()
    elif isinstance(content, bytes):
        path.write_bytes(content)
    with pytest.raises(NextestConfigurationError, match=name):
        load_nextest_config(path)


def test_a_readable_configuration_parses(tmp_path: pathlib.Path) -> None:
    """The other direction, so the boundary refuses rather than always fails."""
    path = tmp_path / "nextest.toml"
    path.write_text(_VALID_CONFIG, encoding="utf-8")
    assert nextest_config_text(path) == _VALID_CONFIG, (
        "the boundary must return the file's text"
    )
    assert load_nextest_config(path) == {
        "profile": {"default": {"slow-timeout": "300s"}}
    }, "the boundary must return the parsed configuration"


def test_a_missing_source_root_is_a_named_error(tmp_path: pathlib.Path) -> None:
    """A root that is not there would otherwise read as a tree with no tests."""
    with pytest.raises(RustSourceError, match="not a directory"):
        rust_source_texts(tmp_path / "absent")


def test_an_undecodable_source_is_a_named_error(tmp_path: pathlib.Path) -> None:
    """One unreadable file means the discovery saw less than the tree holds."""
    (tmp_path / "bad.rs").write_bytes(b"fn \xff() {}\n")
    with pytest.raises(RustSourceError, match=r"bad\.rs"):
        rust_source_texts(tmp_path)


def test_build_output_and_hidden_files_are_not_sources(tmp_path: pathlib.Path) -> None:
    """Only the tree's own sources are read, and each under its path."""
    (tmp_path / "target").mkdir()
    (tmp_path / "target" / "generated.rs").write_text("fn a() {}\n", encoding="utf-8")
    (tmp_path / ".hidden.rs").write_text("fn b() {}\n", encoding="utf-8")
    (tmp_path / "lib.rs").write_text("fn c() {}\n", encoding="utf-8")
    assert rust_source_texts(tmp_path) == {tmp_path / "lib.rs": "fn c() {}\n"}, (
        "only the tree's own sources may be read"
    )


def test_the_discovery_attributes_each_call_to_its_own_test() -> None:
    """A query over the sources it is given, with no filesystem behind it.

    The second test calls `trybuild` and the first does not; a discovery
    that read a window of fixed size would credit the first with the
    second's call.
    """
    source = (
        "#[test]\nfn plain() {\n    assert!(true);\n}\n\n"
        "#[test]\nfn compiles() {\n    let t = trybuild::TestCases::new();\n}\n"
    )
    path = pathlib.Path("crate/tests/ui.rs")
    assert compile_contract_tests({path: source}) == {"compiles": path}, (
        "the call must be credited to the test that holds it"
    )


#: A helper declared above the test that calls it, and one declared below.
_HELPERS = """
fn build_before() {
    let t = trybuild::TestCases::new();
}

#[test]
fn calls_the_earlier_helper() {
    build_before();
}

#[test]
fn plain() {
    assert!(true);
}

#[test]
fn calls_the_later_helper() {
    build_after();
}

fn build_after() {
    let t = trybuild::TestCases::new();
}
"""


def test_a_helper_passes_its_call_to_the_test_that_calls_it() -> None:
    """Wherever the helper sits, the test calling it is found and no other.

    Splitting at `#[test]` credited `build_after`'s call to whichever test
    preceded it, which is the caller here only by the order of the file, and
    never found `calls_the_earlier_helper` at all.
    """
    path = pathlib.Path("crate/tests/ui.rs")
    assert compile_contract_tests({path: _HELPERS}) == {
        "calls_the_earlier_helper": path,
        "calls_the_later_helper": path,
    }, "both callers, and only they, must be found"


def test_a_brace_in_a_literal_does_not_end_a_body() -> None:
    """A literal `}` must not close the test before its call is read."""
    source = (
        '#[test]\nfn quoted() {\n    let s = "}";\n    trybuild::TestCases::new();\n}\n'
    )
    assert compile_contract_tests({pathlib.Path("t.rs"): source}) == {
        "quoted": pathlib.Path("t.rs")
    }, "a literal brace must not end the body"
