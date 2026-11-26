"""The feature reader's spellings agree on every generated input.

``mutation_features`` normalizes the ways cargo can spell one feature
selection. The properties here state the relation rather than the examples:
the separate and attached spellings of a feature name read as the same token,
a ``-C`` or ``--cargo-arg`` smuggled selection reads as the direct one, and a
list with no feature flag produces no selection. They complement the
parameterized tables in ``mutation_testing_test`` because the spelling space
is open-ended.

Run via ``make test-workflow-contracts``.
"""

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from mutation_features import driver_selections, feature_tokens, resolved_cargo

#: A feature name as cargo accepts it: non-empty, no leading option dash, and
#: no comma or whitespace, which the reader treats as separators.
_FEATURE_NAME = st.text(
    alphabet=st.characters(
        whitelist_categories=("Ll", "Lu"),
        whitelist_characters="_-0123456789",
    ),
    min_size=1,
    max_size=16,
).filter(lambda name: not name.startswith("-"))

#: Arguments that name no feature, so a list built from them has no selection.
_PLAIN_ARGUMENT = st.sampled_from(
    ["cargo", "test", "--locked", "--release", "--workspace"]
)


@settings(max_examples=50)
@given(name=_FEATURE_NAME)
def test_attached_and_separate_feature_spellings_agree(name: str) -> None:
    """`--features x`, `--features=x`, `-F x` and `-Fx` normalize alike."""
    assert feature_tokens(["--features", name]) == [f"--features {name}"]
    assert feature_tokens([f"--features={name}"]) == [f"--features {name}"]
    assert feature_tokens(["-F", name]) == [f"-F {name}"]
    assert feature_tokens([f"-F{name}"]) == [f"-F {name}"]


@settings(max_examples=50)
@given(name=_FEATURE_NAME)
def test_a_smuggled_selection_agrees_with_the_direct_one(name: str) -> None:
    """Every cargo-argument spelling reads as the selection it forwards."""
    direct = feature_tokens(["--features", name])
    assert feature_tokens(["-C", f"--features={name}"]) == direct
    assert feature_tokens([f"-C--features={name}"]) == direct
    assert feature_tokens(["--cargo-arg", f"--features={name}"]) == direct
    assert feature_tokens([f"--cargo-test-arg=--features={name}"]) == direct


@settings(max_examples=50)
@given(arguments=st.lists(_PLAIN_ARGUMENT, max_size=8))
def test_arguments_without_a_feature_flag_select_nothing(arguments: list[str]) -> None:
    """An option list that names no feature has no selection to compare."""
    assert feature_tokens(arguments) == []
    assert driver_selections(arguments) == []


@settings(max_examples=50)
@given(name=_FEATURE_NAME)
def test_every_spelling_that_reaches_the_driver_is_reported(name: str) -> None:
    """The driver is reported under each spelling that reaches it."""
    assert driver_selections(["--all-features"]) != []
    assert driver_selections(["--features", f"{name},dylint-driver"]) != []
    assert driver_selections(["-C", f"--features=dylint-driver,{name}"]) != []


@settings(max_examples=50)
@given(name=_FEATURE_NAME)
def test_resolved_cargo_refuses_a_definition_with_a_selection(name: str) -> None:
    """A CARGO definition may select the executable, never a feature."""
    assert resolved_cargo("$(or $(shell command -v cargo 2>/dev/null),cargo)") == (
        "cargo"
    )
    with pytest.raises(ValueError, match="feature selection"):
        resolved_cargo(f"cargo --features {name}")