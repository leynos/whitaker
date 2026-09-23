"""Where the publisher's CodeScene credential is bound, and where it is not.

The upload step's guard tests `env.CS_ACCESS_TOKEN != ''`. That guard is only
half of the arrangement: GitHub evaluates a missing context property as an
empty string, so if the step's `env` binding were deleted the guard would still
read correctly and the upload would skip silently on every run, with the secret
present. So the binding is asserted positively, as is the input that hands it
to the action.

The negative half keeps the credential from spreading: no workflow- or
job-level `env`, and no step other than the upload, may declare it. A wider
scope would put it in reach of every step in that scope, and a reader that
checked only the upload step would not notice.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

#: The variable the upload step binds and the action reads.
CREDENTIAL_KEY: typ.Final[str] = "CS_ACCESS_TOKEN"

#: The expression the step's `env` binds it to, without its `${{ }}` wrapper.
SECRET_REFERENCE: typ.Final[str] = f"secrets.{CREDENTIAL_KEY}"

#: The action input that receives it, and the expression that fills it.
TOKEN_INPUT: typ.Final[str] = "access-token"
ENVIRONMENT_REFERENCE: typ.Final[str] = f"env.{CREDENTIAL_KEY}"

#: A whole-value expression, with the whitespace GitHub allows inside it.
_EXPRESSION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\$\{\{\s*(?P<body>.*?)\s*\}\}$", re.DOTALL
)


def _expression_body(value: object) -> str | None:
    """Return a whole-value `${{ }}` expression's body, or `None`."""
    if not isinstance(value, str):
        return None
    match = _EXPRESSION.match(value.strip())
    return match.group("body") if match else None


def _mapping(value: object) -> dict[str, typ.Any]:
    """Return a value when it is a mapping, and an empty one otherwise."""
    return value if isinstance(value, dict) else {}


def binds_the_credential(step: dict[str, typ.Any]) -> bool:
    """Return whether an upload step binds the credential and passes it on.

    Parameters
    ----------
    step : dict[str, typ.Any]
        One parsed workflow step.

    Returns
    -------
    bool
        True when the step's `env` binds `CS_ACCESS_TOKEN` to
        `${{ secrets.CS_ACCESS_TOKEN }}` and its `access-token` input is
        `${{ env.CS_ACCESS_TOKEN }}`.

    >>> binds_the_credential({
    ...     "env": {"CS_ACCESS_TOKEN": "${{ secrets.CS_ACCESS_TOKEN }}"},
    ...     "with": {"access-token": "${{env.CS_ACCESS_TOKEN}}"},
    ... })
    True
    >>> binds_the_credential({"with": {"access-token": "${{ env.CS_ACCESS_TOKEN }}"}})
    False
    """
    bound = _expression_body(_mapping(step.get("env")).get(CREDENTIAL_KEY))
    passed = _expression_body(_mapping(step.get("with")).get(TOKEN_INPUT))
    return bound == SECRET_REFERENCE and passed == ENVIRONMENT_REFERENCE


def _declares(scope: dict[str, typ.Any]) -> bool:
    """Return whether a workflow, job or step `env` names the credential."""
    return CREDENTIAL_KEY in _mapping(scope.get("env"))


def _job_scopes(
    name: str,
    job: dict[str, typ.Any],
    is_upload: typ.Callable[[dict[str, typ.Any]], bool],
) -> list[str]:
    """Return the scopes within one job, other than upload steps, naming it."""
    steps = job.get("steps")
    step_scopes = [
        f"job {name} step {index}"
        for index, step in enumerate(steps if isinstance(steps, list) else ())
        if isinstance(step, dict) and not is_upload(step) and _declares(step)
    ]
    job_scope = [f"job {name}"] if _declares(job) else []
    return job_scope + step_scopes


def credential_scopes(
    document: dict[str, typ.Any], is_upload: typ.Callable[[dict[str, typ.Any]], bool]
) -> list[str]:
    """Return every scope other than an upload step that declares the credential.

    Parameters
    ----------
    document : dict[str, typ.Any]
        One parsed workflow document.
    is_upload : Callable[[dict[str, typ.Any]], bool]
        Whether a step is an upload step, the one scope allowed to bind it.

    Returns
    -------
    list[str]
        `workflow`, `job <name>` and `job <name> step <index>` for each other
        scope whose `env` names the credential.

    >>> credential_scopes({"env": {"CS_ACCESS_TOKEN": "x"}, "jobs": {}}, bool)
    ['workflow']
    """
    scopes = ["workflow"] if _declares(document) else []
    for name, job in _mapping(document.get("jobs")).items():
        scopes.extend(_job_scopes(name, _mapping(job), is_upload))
    return scopes
