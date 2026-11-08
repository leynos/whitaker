"""Each trunk writer compiles under the environment its reader declares.

`trunk_writer.COMPILE_ENVIRONMENT` names the variables that can change what a
compiler invocation produces. A writer declaring them differently from its
reader would fill `main`'s scope with shapes the pull request never asks for,
or run doctests under laxer flags than the pull request does. The checked-in
pairs are held to it here, and each refusal is proved on a synthetic case.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from trunk_writer import (
    TRUNK_WRITERS,
    compile_environment,
    environment_violations,
)
from ubicloud_workflow_support import UBICLOUD_JOBS, load_job, load_workflow


def _environment_of(job_name: str) -> dict[str, str]:
    """Return the compile environment a checked-in Ubicloud job runs under."""
    return compile_environment(
        load_job(job_name), load_workflow(UBICLOUD_JOBS[job_name])
    )


@pytest.mark.parametrize(("reader", "writer"), sorted(TRUNK_WRITERS.items()))
def test_each_writer_declares_its_readers_compile_environment(
    reader: str, writer: str
) -> None:
    """`coverage-upload` must deny rustdoc warnings as `coverage-check` does."""
    violations = environment_violations(
        _environment_of(reader), _environment_of(writer)
    )
    assert not violations, f"{writer} as {reader}'s trunk writer: {violations}"


_WORKFLOW: typ.Final = {
    "env": {"RUSTFLAGS": "-D warnings", "RUSTDOCFLAGS": "-D warnings"}
}


@pytest.mark.parametrize(
    ("writer_job", "writer_workflow", "expected"),
    [
        pytest.param(
            {},
            {"env": {"RUSTFLAGS": "-D warnings"}},
            "RUSTDOCFLAGS is '-D warnings' in the reader but None",
            id="a-writer-omitting-a-variable",
        ),
        pytest.param(
            {},
            {"env": {"RUSTFLAGS": "-D warnings", "RUSTDOCFLAGS": ""}},
            "RUSTDOCFLAGS is '-D warnings' in the reader but ''",
            id="a-writer-with-a-different-value",
        ),
        pytest.param(
            {"env": {"RUSTFLAGS": "-C prefer-dynamic"}},
            _WORKFLOW,
            "RUSTFLAGS is '-D warnings' in the reader but '-C prefer-dynamic'",
            id="a-job-level-override",
        ),
        pytest.param(
            {},
            _WORKFLOW | {"env": _WORKFLOW["env"] | {"CARGO_INCREMENTAL": 1}},
            "CARGO_INCREMENTAL is None in the reader but '1'",
            id="a-variable-only-the-writer-declares",
        ),
    ],
)
def test_a_writer_compiling_under_another_environment_is_refused(
    writer_job: dict[str, typ.Any], writer_workflow: dict[str, typ.Any], expected: str
) -> None:
    """Each way the writer's environment can drift from the reader's is caught."""
    reader = compile_environment({}, _WORKFLOW)
    violations = environment_violations(
        reader, compile_environment(writer_job, writer_workflow)
    )
    assert any(expected in violation for violation in violations), violations


def test_the_same_environment_declared_at_another_level_is_accepted() -> None:
    """The narrow half: where a value is declared does not matter, only what."""
    reader = compile_environment({}, _WORKFLOW)
    writer = compile_environment({"env": _WORKFLOW["env"]}, {})
    violations = environment_violations(reader, writer)
    assert violations == [], f"a job-level copy was refused: {violations}"
