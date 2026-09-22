"""The run-block reader tells a command it runs from one it mentions.

The measuring-lane contract uses this to require `make coverage`. A substring
test passed for `echo make coverage` and for a comment, so a lane could stop
running its tests while the contract stayed green.

Run via ``make test-workflow-contracts``.
"""

import pytest
from shell_commands import runs_command

COMMAND = "make coverage"


@pytest.mark.parametrize(
    "script",
    [
        pytest.param("make coverage", id="alone"),
        pytest.param("make  coverage", id="extra-whitespace"),
        pytest.param("set -euo pipefail; make coverage", id="after-a-semicolon"),
        pytest.param("cd crate && make coverage", id="after-and"),
        pytest.param("false || make coverage", id="after-or"),
        pytest.param("make coverage | tee log", id="in-a-pipeline"),
        pytest.param("(make coverage)", id="in-a-subshell"),
        pytest.param("RUSTFLAGS=-Dwarnings make coverage", id="after-an-assignment"),
        pytest.param("echo start\nmake coverage\necho done", id="on-its-own-line"),
        pytest.param("make \\\n  coverage", id="across-a-continuation"),
        pytest.param("make coverage # measure", id="before-a-comment"),
        pytest.param("make coverage COVERAGE_OUTPUT=x", id="with-arguments"),
    ],
)
def test_a_command_the_script_runs_is_found(script: str) -> None:
    """Each shape the shell executes as the command, so the rule accepts it."""
    assert runs_command(script, COMMAND), f"{script!r} runs {COMMAND!r}"


@pytest.mark.parametrize(
    "script",
    [
        pytest.param("echo make coverage", id="an-argument"),
        pytest.param("# make coverage", id="a-comment"),
        pytest.param("make test # then make coverage", id="a-trailing-comment"),
        pytest.param("make test # ; make coverage", id="an-operator-inside-a-comment"),
        pytest.param("make coverage-report", id="a-longer-target"),
        pytest.param("printf '%s' 'make coverage'", id="a-quoted-string"),
        pytest.param("make 'coverage", id="an-unbalanced-quote"),
        pytest.param("", id="nothing"),
    ],
)
def test_a_command_the_script_only_mentions_is_not_found(script: str) -> None:
    """The narrow half, and the reason the reader exists.

    Every row contains the words, and none runs them. An unbalanced quote is
    read as no command, since a requirement should fail rather than guess.
    """
    assert not runs_command(script, COMMAND), f"{script!r} does not run it"
