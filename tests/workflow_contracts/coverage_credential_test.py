"""CV-005: how the CodeScene credential and host are found in a pull-request lane.

The boundary scan reads the credential from two sources, the raw text and the
parsed values, and each has to be proved on its own: a case feeding both at
once passes while either reader is deleted. It also refuses `secrets: inherit`,
which hands a called workflow the credential while naming nothing, and any
mention of CodeScene's host, which a plain `curl` needs and no action or
command name reveals.

Run via ``make test-workflow-contracts``.
"""

import pytest
from coverage_boundary import (
    CODESCENE_HOST,
    CREDENTIAL_ENVIRONMENT_KEY,
    coverage_surface_offenders,
)
from ubicloud_workflow_support import parse_workflow


def _document(job_body: str) -> dict[str, object]:
    """Return a pull-request workflow with one job carrying the given body."""
    parsed = parse_workflow(f"on: pull_request\njobs:\n  a:\n{job_body}")
    assert isinstance(parsed, dict), "the synthetic workflow must parse to a mapping"
    return parsed


@pytest.mark.parametrize(
    "job_body",
    [
        pytest.param(
            "    steps:\n      - env:\n"
            f"          {CREDENTIAL_ENVIRONMENT_KEY}: ${{{{ secrets.X }}}}\n"
            "        run: make test\n",
            id="a-step-env-key",
        ),
        pytest.param(
            "    steps:\n      - env:\n"
            f"          TOKEN: ${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}\n"
            "        run: make test\n",
            id="a-step-env-value",
        ),
        pytest.param(
            "    steps:\n      - uses: some/action@abc\n        with:\n"
            f"          token: ${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}\n",
            id="an-action-input",
        ),
        pytest.param(
            "    steps:\n"
            f'      - run: echo "${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"\n',
            id="a-run-body",
        ),
        pytest.param(
            "    uses: ./.github/workflows/probe.yml\n    secrets:\n"
            f"      {CREDENTIAL_ENVIRONMENT_KEY}: ${{{{ secrets.X }}}}\n",
            id="named-forwarding",
        ),
    ],
)
def test_a_parsed_credential_is_found_without_the_raw_text(job_body: str) -> None:
    """The parsed reader, on its own.

    The raw text is passed empty, so only the parsed scan can report these.
    Every earlier fixture either named the credential in raw text alone or in
    both sources, so deleting the parsed reader failed nothing.
    """
    offenders = coverage_surface_offenders("scratch.yml", _document(job_body), "")
    assert any("parsed value" in offence for offence in offenders), (
        f"the parsed reader must find the credential here; it gave {offenders}"
    )


def test_inherited_secrets_are_refused() -> None:
    """`secrets: inherit` names nothing and forwards everything.

    A scan for the credential's name finds no mention of it, while the called
    workflow receives it with every other secret the caller holds.
    """
    document = _document(
        "    uses: ./.github/workflows/probe.yml\n    secrets: inherit\n"
    )
    offenders = coverage_surface_offenders("scratch.yml", document, "")
    assert offenders == [
        "scratch.yml:a forwards every secret with `secrets: inherit`"
    ], f"inherited secrets must be the one offence here; it gave {offenders}"


def test_forwarding_another_secret_by_name_is_not_accused() -> None:
    """The narrow half: forwarding is refused for this credential, not at all."""
    document = _document(
        "    uses: ./.github/workflows/probe.yml\n    secrets:\n"
        "      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}\n"
    )
    offenders = coverage_surface_offenders("scratch.yml", document, "")
    assert not offenders, f"another secret may be forwarded; it gave {offenders}"


@pytest.mark.parametrize(
    "raw",
    [
        pytest.param(
            "curl https://api.codescene.io/v2/projects/1\n", id="a-curl-to-the-api"
        ),
        pytest.param("# upload to CodeScene.IO later\n", id="another-letter-case"),
    ],
)
def test_the_host_is_refused_in_any_letter_case(raw: str) -> None:
    """A plain `curl` names neither the action nor the command.

    It needs the host, so the host is refused in the raw text, and in any
    letter case because DNS ignores it.
    """
    document = _document("    steps:\n      - run: make test\n")
    offenders = coverage_surface_offenders("scratch.yml", document, raw)
    assert offenders == [f"scratch.yml: raw text names {CODESCENE_HOST}"], (
        f"the host must be the one offence here; it gave {offenders}"
    )
