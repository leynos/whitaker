"""Read the feature selections in a cargo or cargo-mutants argument list.

Whitaker's scheduled mutation run fails in a way no build gate shows: with
``--all-features``, Cargo unifies the ``dylint-driver`` feature into the lint
crates' integration-test binaries, and those binaries cannot link the
compiler's ``rustc_private`` libraries. Every shard of run 33961579896 failed
its baseline with 19 duplicate-dependency errors before testing a mutant.

The flags can arrive through several spellings, including one level down
inside a ``--cargo-arg``/``--cargo-test-arg`` value, so the contract tests
share this reader rather than each re-implementing it. It is test support
only: no production code depends on it, and it is not a parser for the full
cargo command line.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import shlex
from collections.abc import Iterator, Sequence
from typing import Final

#: cargo-mutants forwards these to the cargo invocations it runs.
CARGO_ARGUMENT_FLAGS: Final[tuple[str, ...]] = (
    "-C",
    "--cargo-arg",
    "--cargo-test-arg",
)
CARGO_ARGUMENT_PREFIXES: Final[tuple[str, ...]] = tuple(
    f"{flag}=" for flag in CARGO_ARGUMENT_FLAGS
)

#: Feature-selection flags. cargo-mutants' `-F` also spells `--re`, but a
#: regex naming the driver is worth reporting as a selection too.
FEATURE_FLAGS: Final[tuple[str, ...]] = ("--features", "-F")

#: Turning the default set off changes the selection as much as naming a
#: feature does, and it takes no value.
NO_DEFAULT_FEATURES: Final[str] = "--no-default-features"


def expand_cargo_arguments(arguments: Sequence[str]) -> Iterator[str]:
    """Yield the arguments plus the cargo arguments they smuggle.

    `--cargo-arg` and `--cargo-test-arg` are handed to cargo, so a feature
    flag can hide one level down. The expansion recurses because a smuggled
    argument may itself be another cargo argument.

    Parameters
    ----------
    arguments : Sequence[str]
        One shell-lexed argument list, such as a caller's ``extra-args``.

    Yields
    ------
    str
        Each ordinary argument in order, with the contents of each cargo
        argument expanded in its place.

    >>> list(expand_cargo_arguments(["--all-features"]))
    ['--all-features']
    >>> list(expand_cargo_arguments(["-C", "--features=dylint-driver"]))
    ['--features=dylint-driver']
    >>> list(expand_cargo_arguments(["--cargo-test-arg=--all-features"]))
    ['--all-features']
    >>> list(expand_cargo_arguments(["-C--all-features"]))
    ['--all-features']
    """
    index = 0
    while index < len(arguments):
        argument = arguments[index]
        if argument in CARGO_ARGUMENT_FLAGS:
            index += 1
            if index < len(arguments):
                yield from expand_cargo_arguments(shlex.split(arguments[index]))
        elif argument.startswith(CARGO_ARGUMENT_PREFIXES):
            yield from expand_cargo_arguments(shlex.split(argument.partition("=")[2]))
        elif argument.startswith("-C") and len(argument) > 2:
            yield from expand_cargo_arguments(shlex.split(argument.removeprefix("-C")))
        else:
            yield argument
        index += 1


def feature_names(value: str) -> list[str]:
    """Return the feature names in a comma- or space-separated value.

    Parameters
    ----------
    value : str
        A feature list as cargo accepts it: names separated by commas, spaces
        or both.

    Returns
    -------
    list[str]
        The non-empty names, in order.

    >>> feature_names("foo, dylint-driver")
    ['foo', 'dylint-driver']
    >>> feature_names("")
    []
    """
    return [name for name in re.split(r"[,\s]+", value) if name]


def feature_tokens(arguments: Sequence[str]) -> list[str]:
    """Return each feature selection as a normalized `flag value` token.

    Normalizing lets two argument lists be compared for parity without
    depending on whether a flag carried its value attached or separate.

    Parameters
    ----------
    arguments : Sequence[str]
        One shell-lexed argument list.

    Returns
    -------
    list[str]
        Each selection as ``flag value``, or just the flag when it takes no
        value.

    >>> feature_tokens(["cargo", "test", "--locked"])
    []
    >>> feature_tokens(["--all-features"])
    ['--all-features']
    >>> feature_tokens(["--no-default-features"])
    ['--no-default-features']
    >>> feature_tokens(["--features", "foo,dylint-driver"])
    ['--features foo,dylint-driver']
    >>> feature_tokens(["--features=foo"])
    ['--features foo']
    >>> feature_tokens(["-F", "foo"])
    ['-F foo']
    >>> feature_tokens(["-Ffoo"])
    ['-F foo']
    """
    expanded = list(expand_cargo_arguments(arguments))
    tokens: list[str] = []
    index = 0
    while index < len(expanded):
        argument = expanded[index]
        if argument == "--all-features":
            tokens.append(argument)
            index += 1
            continue
        if argument == NO_DEFAULT_FEATURES:
            tokens.append(argument)
            index += 1
            continue
        if argument in FEATURE_FLAGS:
            probe = index + 1
            while probe < len(expanded) and not expanded[probe].startswith("-"):
                probe += 1
            tokens.append(f"{argument} {' '.join(expanded[index + 1 : probe])}".strip())
            index = probe
            continue
        if argument.startswith("--features="):
            tokens.append(f"--features {argument.partition('=')[2]}")
        elif argument.startswith("-F") and len(argument) > 2:
            tokens.append(f"-F {argument.removeprefix('-F')}")
        index += 1
    return tokens


def resolved_cargo(definition: str) -> str:
    """Return the text that stands for a Makefile's ``CARGO`` definition.

    The contract tests substitute ``cargo`` for ``$(CARGO)`` so that a recipe
    reads as the command Make runs. A definition that appends arguments would
    disappear in that substitution, so any feature selection it carries is
    refused rather than silently accepted.

    Parameters
    ----------
    definition : str
        The right-hand side of the Makefile's ``CARGO ?=`` assignment.

    Returns
    -------
    str
        The literal ``cargo`` the substitution uses.

    Raises
    ------
    ValueError
        If the definition carries a feature selection.

    >>> resolved_cargo("$(or $(shell command -v cargo 2>/dev/null),cargo)")
    'cargo'
    >>> resolved_cargo("cargo --all-features")
    Traceback (most recent call last):
    ...
    ValueError: CARGO must select the cargo executable without a feature selection: ['--all-features']
    """
    selections = feature_tokens(definition.split())
    if selections:
        raise ValueError(
            "CARGO must select the cargo executable without a feature "
            f"selection: {selections!r}"
        )
    return "cargo"


def driver_selections(arguments: Sequence[str]) -> list[str]:
    """Return a message for each selection that enables `dylint-driver`.

    Every offending spelling is reported, so a caller sees all the reasons at
    once rather than repairing one and meeting the next. `--all-features` is
    rejected outright: it enables the driver on every lint crate without
    naming it.

    Parameters
    ----------
    arguments : Sequence[str]
        One shell-lexed argument list.

    Returns
    -------
    list[str]
        One message per selection that names or enables `dylint-driver`,
        empty when none does.

    >>> driver_selections(["--features", "an-unrelated-feature"])
    []
    >>> driver_selections(["--all-features"])
    ['--all-features enables every feature, including dylint-driver']
    >>> driver_selections(["--features=dylint-driver"])
    ['--features names dylint-driver']
    >>> driver_selections(["-C", "--features", "dylint-driver"])
    ['--features names dylint-driver']
    >>> driver_selections(["-Fdylint-driver"])
    ['-F names dylint-driver']
    """
    selections: list[str] = []
    for token in feature_tokens(arguments):
        flag, _, value = token.partition(" ")
        if flag == "--all-features":
            selections.append(
                "--all-features enables every feature, including dylint-driver"
            )
        elif "dylint-driver" in feature_names(value):
            selections.append(f"{flag} names dylint-driver")
    return selections
