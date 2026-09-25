"""Properties of the publisher credential readers over generated jobs.

The contract tests hold the repository's own publisher and a handful of
documents written one rule at a time. These properties generate the job
instead: unrelated steps, the availability check and the upload in any order,
and the credential bound, under any letter case and any variable name, in at
most one `env` scope.

Invariants covered:

- `credential_scopes` finds an `env` naming the credential at every scope,
  the upload's own included, and accuses no document without one;
- `credential_sites` names exactly the check's command and the upload's input
  when no `env` holds the credential, and more than that when one does;
- `checks_availability` recognizes the check wherever it sits, and never
  once it carries an `env`.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

import typing as typ

from hypothesis import given
from hypothesis import strategies as st
from publisher_credential import (
    CHECK_COMMAND,
    checks_availability,
    credential_scopes,
    credential_sites,
)

#: The upload step's action and its input, as the publisher carries them.
UPLOAD_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage@abc"
)
PASSED: typ.Final[str] = "${{ secrets.CS_ACCESS_TOKEN }}"

#: The scopes a generated case may bind the credential in.
SCOPES: typ.Final[tuple[str, ...]] = (
    "nowhere",
    "workflow",
    "job",
    "upload",
    "check",
    "other",
)

#: The credential's name in any letter case GitHub resolves alike.
_SPELLINGS: typ.Final[st.SearchStrategy[str]] = st.sampled_from(
    ["CS_ACCESS_TOKEN", "cs_access_token", "Cs_Access_Token"]
)


class Case(typ.NamedTuple):
    """One generated publisher and where it binds the credential."""

    document: dict[str, typ.Any]
    scope: str
    check_at: int
    upload_at: int


@st.composite
def _cases(draw: st.DrawFn) -> Case:
    """Build one publisher job with the credential in at most one scope."""
    scope = draw(st.sampled_from(SCOPES))
    others = draw(st.integers(min_value=1, max_value=3))
    binding = {draw(st.sampled_from(["T", "TOKEN"])): (
        "${{ secrets." + draw(_SPELLINGS) + " }}"
    )}
    steps: list[dict[str, typ.Any]] = [
        {"run": f"make step-{index}"} for index in range(others)
    ]
    if scope == "other":
        steps[0]["env"] = binding
    upload: dict[str, typ.Any] = {"uses": UPLOAD_ACTION, "with": {"access-token": PASSED}}
    check: dict[str, typ.Any] = {"id": "codescene_token", "run": CHECK_COMMAND}
    if scope == "upload":
        upload["env"] = binding
    if scope == "check":
        check["env"] = binding
    upload_at = draw(st.integers(min_value=0, max_value=len(steps)))
    steps.insert(upload_at, upload)
    check_at = draw(st.integers(min_value=0, max_value=len(steps)))
    steps.insert(check_at, check)
    job: dict[str, typ.Any] = {"steps": steps}
    if scope == "job":
        job["env"] = binding
    document: dict[str, typ.Any] = {"jobs": {"up": job}}
    if scope == "workflow":
        document["env"] = binding
    upload_index = upload_at + (1 if check_at <= upload_at else 0)
    return Case(document, scope, check_at, upload_index)


@given(_cases())
def test_an_env_holding_the_credential_is_always_found(case: Case) -> None:
    """Any `env` naming the credential is reported, and no other is."""
    found = credential_scopes(case.document)
    assert bool(found) == (case.scope != "nowhere"), (case.scope, found)


@given(_cases())
def test_the_sites_are_exactly_the_check_and_the_input_when_unbound(case: Case) -> None:
    """With no `env` binding, the credential sits at the two permitted paths."""
    permitted = sorted([
        f"jobs.up.steps[{case.check_at}].run",
        f"jobs.up.steps[{case.upload_at}].with.access-token",
    ])
    sites = credential_sites(case.document)
    assert (sites == permitted) == (case.scope == "nowhere"), (case.scope, sites)


@given(_cases())
def test_the_check_is_recognized_until_it_carries_an_env(case: Case) -> None:
    """The check is the check wherever it sits, and not once it holds an `env`."""
    check = case.document["jobs"]["up"]["steps"][case.check_at]
    assert checks_availability(check) == (case.scope != "check"), check
