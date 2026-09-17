"""How a profile declaration is recognized, whatever it is spelt as.

``nextest_profile_test`` asserts that no workflow selects a profile
through the environment, and that assertion is only as good as the
sweep underneath it. This repository's own workflows declare the
variable nowhere, so the sweep agrees with the tree whether it reads
anything or not: parametrized over correct files it passes either way.

These cases drive the sweep directly, with documents the tree does not
contain, so each spelling and each scope is a case the reading either
covers or fails.

The spellings matter because YAML has two blank ones and they parse
differently. ``NEXTEST_PROFILE: ""`` is the empty string and a valueless
``NEXTEST_PROFILE:`` is None. Both are declarations at that scope, both
mask whatever the outer scopes hold, and both select the default profile
as surely as a named value selects one. A reader deciding by truth or by
``is not None`` handles the quoted spelling and walks past the valueless
one, which is the defect this module exists to refuse.

Every document here is parsed from YAML text rather than written as a
mapping, because the fact under test is what the parser produces. A
hand-built ``{"NEXTEST_PROFILE": None}`` would assume the parse the
cases are meant to establish.
"""

import typing as typ

import pytest
import yaml
from suite_lanes import PROFILE_VARIABLE, _scopes_declaring_profile

#: The three blank or valued spellings of a declaration, with what each
#: parses to. A declaration masks the outer scopes whichever it is.
DECLARATIONS: typ.Final[tuple[tuple[str, str, object], ...]] = (
    ("valueless", f"{PROFILE_VARIABLE}:", None),
    ("empty-string", f'{PROFILE_VARIABLE}: ""', ""),
    ("named", f"{PROFILE_VARIABLE}: ci", "ci"),
)


def _documents(text: str) -> dict[str, dict[str, typ.Any]]:
    """Return one parsed workflow, keyed as the sweep keys its own.

    Parameters
    ----------
    text : str
        A workflow document.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        The parsed document under a single file name.
    """
    return {"example.yml": yaml.safe_load(text)}


def _workflow_scope(declaration: str) -> str:
    """Return a workflow declaring the given line at workflow level."""
    return (
        f"env:\n  {declaration}\njobs:\n  test:\n    steps:\n      - run: make test\n"
    )


def _job_scope(declaration: str) -> str:
    """Return a workflow declaring the given line at job level."""
    return (
        "jobs:\n  test:\n"
        f"    env:\n      {declaration}\n"
        "    steps:\n      - run: make test\n"
    )


def _step_scope(declaration: str) -> str:
    """Return a workflow declaring the given line at step level."""
    return (
        "jobs:\n  test:\n    steps:\n      - run: make test\n"
        f"        env:\n          {declaration}\n"
    )


SCOPES: typ.Final[tuple[tuple[str, typ.Callable[[str], str], str], ...]] = (
    ("workflow", _workflow_scope, "example.yml: workflow level"),
    ("job", _job_scope, "example.yml:test: job level"),
    ("step", _step_scope, "example.yml:test: step 0"),
)


@pytest.mark.parametrize(
    ("declaration", "parses_to"),
    [
        pytest.param(declaration, parses_to, id=name)
        for name, declaration, parses_to in DECLARATIONS
    ],
)
def test_each_spelling_parses_as_recorded(declaration: str, parses_to: object) -> None:
    """The table's premise, established rather than assumed.

    The sweep decides by key membership because the two blank spellings
    parse to different values. That is a claim about the parser, so it
    is checked against the parser rather than stated in a comment.
    """
    parsed = yaml.safe_load(f"env:\n  {declaration}\n")["env"]
    assert parsed == {PROFILE_VARIABLE: parses_to}, (
        f"{declaration!r} must parse to {parses_to!r}; the sweep decides by "
        f"key membership precisely because these differ"
    )


@pytest.mark.parametrize(
    ("build", "label"),
    [pytest.param(build, label, id=name) for name, build, label in SCOPES],
)
@pytest.mark.parametrize(
    "declaration",
    [pytest.param(declaration, id=name) for name, declaration, _ in DECLARATIONS],
)
def test_a_declaration_is_found_at_every_scope_in_every_spelling(
    build: typ.Callable[[str], str], label: str, declaration: str
) -> None:
    """Nine cases, because a miss at any one of them is silent.

    A scope the sweep does not walk, or a spelling it does not
    recognize, puts every lane in that scope under a profile the lane
    reader still judges as ``default``. Nothing fails when that happens:
    both profiles carry a full set of values, so the comparison succeeds
    against the wrong ones.
    """
    assert _scopes_declaring_profile(_documents(build(declaration))) == [label], (
        f"{declaration!r} at {label} must be reported; it masks the outer "
        f"scopes whatever it holds"
    )


@pytest.mark.parametrize(
    "text",
    [
        pytest.param(
            "jobs:\n  test:\n    steps:\n      - run: make test\n",
            id="no-env-anywhere",
        ),
        pytest.param(
            _workflow_scope("OTHER_VARIABLE: ci"),
            id="another-variable-at-workflow-level",
        ),
        pytest.param(
            _job_scope(f"{PROFILE_VARIABLE}_SUFFIX: ci"),
            id="a-longer-name-with-the-same-prefix",
        ),
        pytest.param(
            "jobs:\n  test:\n    env:\n    steps:\n      - run: make test\n",
            id="a-valueless-env-block",
        ),
        pytest.param(
            "jobs:\n  test:\n    env:\n      - NEXTEST_PROFILE\n"
            "    steps:\n      - run: make test\n",
            id="env-as-a-list",
        ),
        pytest.param(
            "jobs:\n  - test\n",
            id="jobs-as-a-list",
        ),
        pytest.param(
            "jobs: test\n",
            id="jobs-as-a-string",
        ),
    ],
)
def test_nothing_else_is_reported_as_a_declaration(text: str) -> None:
    """The other direction, so the sweep discriminates rather than accuses.

    A sweep that reported any of these would fail the contract on a
    repository that selects no profile through the environment, and the
    contract would be removed rather than believed. The list case is
    here because ``env`` holding a sequence makes ``in`` a membership
    test over values, which would read the variable's own name as a
    declaration of it.

    The malformed ``jobs`` cases are here because the walk answers by
    raising rather than by reporting when it meets one: a list or a
    string has no ``items``, so a document YAML parses and GitHub
    rejects took the sweep down with an ``AttributeError`` instead of
    yielding no scopes.
    """
    assert _scopes_declaring_profile(_documents(text)) == [], (
        f"nothing but an `env` mapping whose keys include {PROFILE_VARIABLE} "
        f"is a declaration of it; a sweep that reports one of these accuses a "
        f"workflow that selects no profile through the environment"
    )


def test_every_scope_of_a_workflow_is_walked_at_once() -> None:
    """Three declarations in one document are three offences, not one.

    The sweep reports where it found each, so a fix that removes one and
    leaves the others still fails. Reporting the first would let a lane
    stay masked behind a fix that looked complete.
    """
    text = (
        f"env:\n  {PROFILE_VARIABLE}: ci\n"
        "jobs:\n  test:\n"
        f'    env:\n      {PROFILE_VARIABLE}: ""\n'
        "    steps:\n      - run: make test\n"
        f"        env:\n          {PROFILE_VARIABLE}:\n"
    )
    assert _scopes_declaring_profile(_documents(text)) == [
        "example.yml: workflow level",
        "example.yml:test: job level",
        "example.yml:test: step 0",
    ], (
        f"every scope that declares {PROFILE_VARIABLE} must be reported, in "
        f"workflow, job then step order; stopping at the first leaves the "
        f"remaining lanes masked behind a fix that looks complete"
    )
