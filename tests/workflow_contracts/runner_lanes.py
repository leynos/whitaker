"""Names one runner placement, whether or not a matrix produced it.

The cache and provenance contracts were written when every placement this
repository cared about was a whole job, so their vocabulary is a job name.
A matrix job has no single placement: ``build-lints`` declares one step list
and GitHub runs it five times, on five runner images, and only two of those
runs are Linux. A contract that can only say "the ``build-lints`` job" cannot
say "the x86_64 Linux leg must stay on 22.04", which is the thing whitaker's
binaries depend on.

This module supplies the missing half of the vocabulary: a lane is a job
together with the matrix leg that selected its runner, and ``None`` for the
leg means the job is not a matrix at all. Resolving a lane to its runner
label is reading, not evaluating: the only expression understood is a
``runs-on`` that defers to the matrix, and anything else is reported rather
than guessed at.
"""

from __future__ import annotations

import re
import typing as typ
from typing import Any, Final

from ubicloud_workflow_support import load_workflow

#: The one ``runs-on`` expression this module resolves. A matrix job that
#: computed its label any other way would need its own reading, so it is
#: refused rather than approximated.
MATRIX_RUNNER_EXPRESSION: Final[str] = "${{ matrix.os }}"

#: The matrix key that selects a leg. Both matrix jobs in the rolling release
#: enumerate their legs by build target, which is also what the glibc baseline
#: step keys its own condition on, so the two agree by construction.
LEG_KEY: Final[str] = "target"


class RunnerLane(typ.NamedTuple):
    """One placement: a job, or one leg of a matrix job.

    Attributes
    ----------
    workflow : str
        The workflow file's name, as it appears in ``.github/workflows``.
    job : str
        The job's key in that workflow's ``jobs`` mapping.
    leg : str or None
        The matrix ``target`` that selects this leg, or ``None`` when the
        job declares no matrix and so has exactly one placement.

    Examples
    --------
    >>> RunnerLane("ci.yml", "linux-full").name
    'linux-full'
    >>> RunnerLane("rolling-release.yml", "build-lints", "x86_64-unknown-linux-gnu").name
    'build-lints[x86_64-unknown-linux-gnu]'
    """

    workflow: str
    job: str
    leg: str | None = None

    @property
    def name(self) -> str:
        """Return the lane's name, qualified by its leg when it has one.

        Returns
        -------
        str
            ``job`` for a plain job and ``job[leg]`` for a matrix leg, so a
            failure message says which of five runs it is talking about.
        """
        return self.job if self.leg is None else f"{self.job}[{self.leg}]"

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:name`` for this lane.

        Examples
        --------
        >>> str(RunnerLane("ci.yml", "linux-full"))
        'ci.yml:linux-full'
        """
        return f"{self.workflow}:{self.name}"


#: Every placement this repository puts on an Ubicloud runner, matrix legs
#: included. The three whole jobs restate `UBICLOUD_JOBS`, and a contract
#: holds the two lists to each other so neither can be extended alone.
UBICLOUD_LANES: Final[tuple[RunnerLane, ...]] = (
    RunnerLane("ci.yml", "coverage-check"),
    RunnerLane("ci.yml", "linux-full"),
    RunnerLane("coverage-main.yml", "coverage-upload"),
    RunnerLane("rolling-release.yml", "build-lints", "x86_64-unknown-linux-gnu"),
    RunnerLane("rolling-release.yml", "build-lints", "aarch64-unknown-linux-gnu"),
    RunnerLane(
        "rolling-release.yml", "build-dependency-binaries", "x86_64-unknown-linux-gnu"
    ),
    RunnerLane(
        "rolling-release.yml", "build-dependency-binaries", "aarch64-unknown-linux-gnu"
    ),
)

#: The prefix every Ubicloud runner label carries.
UBICLOUD_LABEL_PREFIX: Final[str] = "ubicloud-"

#: Labels served by GitHub's own hosted runner pool.
#:
#: actionlint knows these natively, so they are the labels a registry may
#: not contain. Keyed on what GitHub hosts rather than on how one vendor
#: spells its labels, so a second paid provider needs a registration here
#: and not a second prefix to match against. A label in neither this set
#: nor the registry fails the registry contract, which is what a typo
#: should do.
GITHUB_HOSTED_LABELS: Final[frozenset[str]] = frozenset(
    {
        "ubuntu-latest",
        "ubuntu-24.04",
        "ubuntu-24.04-arm",
        "ubuntu-22.04",
        "ubuntu-22.04-arm",
        "windows-latest",
        "macos-latest",
        "macos-15",
        "macos-15-intel",
    }
)

#: The whole of a conditional `runs-on`: one condition, then the label
#: taken when it holds and the label taken when it does not. A lane on
#: the fork fallback can bill for either arm depending on the head that
#: triggered it, so a contract asking what a job can bill for has to read
#: both. Whitaker has no such lane today; the reader is here so that the
#: registry contract is already right on the pull request that adds one.
CONDITIONAL_RUNS_ON: Final[typ.Pattern[str]] = re.compile(
    r"^\$\{\{\s*(?P<condition>.+?)\s*"
    r"&&\s*'(?P<when_true>[^']+)'\s*"
    r"\|\|\s*'(?P<when_false>[^']+)'\s*\}\}$"
)


def leg_guard_selects(condition: object, leg: str) -> bool:
    """Report whether a step's condition runs it on exactly one named leg.

    A containment test is not enough and this is the third guard on this
    branch to learn it. ``false && matrix.target == 'x86_64-unknown-linux-gnu'``
    contains the leg's name and never runs, so a substring check accepts a
    condition that switches the step off, which for the glibc baseline means
    publishing binaries nothing checked.

    The optional ``${{ }}`` wrapper and surrounding whitespace are normalized,
    because both spellings are the same condition. Anything else, another
    operator or a different target, is refused.

    Parameters
    ----------
    condition : object
        A step's ``if`` value, parsed.
    leg : str
        The matrix ``target`` the step must be guarded to.

    Returns
    -------
    bool
        True when the condition is exactly that guard.

    Examples
    --------
    >>> leg_guard_selects("matrix.target == 'x86_64-unknown-linux-gnu'",
    ...                   "x86_64-unknown-linux-gnu")
    True
    >>> leg_guard_selects("${{ matrix.target == 'x86_64-unknown-linux-gnu' }}",
    ...                   "x86_64-unknown-linux-gnu")
    True
    >>> leg_guard_selects("false && matrix.target == 'x86_64-unknown-linux-gnu'",
    ...                   "x86_64-unknown-linux-gnu")
    False
    >>> leg_guard_selects("matrix.target == 'aarch64-unknown-linux-gnu'",
    ...                   "x86_64-unknown-linux-gnu")
    False
    """
    text = str(condition).strip()
    if text.startswith("${{") and text.endswith("}}"):
        text = text[3:-2].strip()
    pattern = r"matrix\.target\s*==\s*(['\"])" + re.escape(leg) + r"\1"
    return re.fullmatch(pattern, text) is not None


#: What a cache key must vary by inside a matrix job. Both Linux legs of the
#: rolling release run one restore step and one save step, so only the
#: rendered key keeps their archives apart.
LEG_DISCRIMINATOR: Final[str] = "${{ runner.arch }}"


#: The longest `timeout-minutes` any lane here may declare. Not a limit on
#: what a job needs, but a bound on what a hang can cost: the slowest observed
#: leg in this repository is under ten minutes, so anything approaching two
#: hours is a typo or a hang nobody has looked at.
MAXIMUM_LANE_TIMEOUT_MINUTES: Final[int] = 120


#: Every lane whose runner image sets the glibc floor for a consumer of
#: whitaker's x86_64 Linux binaries. Both rolling-release matrix jobs publish
#: artefacts that land side by side in an installation, so the floor is the
#: higher of the two and neither may be raised alone.
GLIBC_BASELINE_LANES: Final[tuple[RunnerLane, ...]] = (
    RunnerLane("rolling-release.yml", "build-lints", "x86_64-unknown-linux-gnu"),
    RunnerLane(
        "rolling-release.yml", "build-dependency-binaries", "x86_64-unknown-linux-gnu"
    ),
)

#: The Ubicloud image family that supplies that floor. 22.04 ships glibc 2.35.
GLIBC_BASELINE_IMAGE: Final[str] = "ubuntu-2204"

#: The ceiling the baseline step enforces, which is 22.04's glibc.
GLIBC_BASELINE_MAXIMUM: Final[str] = "GLIBC_2.35"

#: The step that enforces it, and the script it must run.
GLIBC_BASELINE_STEP: Final[str] = "Check glibc baseline"
GLIBC_BASELINE_SCRIPT: Final[str] = "scripts/check_glibc_baseline.py"


def load_lane(lane: RunnerLane) -> dict[str, Any]:
    """Return the job mapping a lane belongs to.

    Every leg of a matrix job shares one step list, so this returns the same
    mapping for each leg. What differs between legs is the runner label and
    the step conditions, which callers read separately.

    Parameters
    ----------
    lane : RunnerLane
        The lane to resolve.

    Returns
    -------
    dict
        The job mapping.
    """
    jobs = load_workflow(lane.workflow).get("jobs")
    assert isinstance(jobs, dict), f"{lane.workflow} must declare a jobs mapping"
    job = jobs.get(lane.job)
    assert isinstance(job, dict), f"{lane} must name a declared job"
    return job


def matrix_legs(job: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Return a matrix job's legs, indexed by the key that selects them.

    Only the ``include`` form is read, because that is the form both matrix
    jobs here use and the only one that pairs a target with an image in a
    single entry. A job with no matrix returns an empty mapping, which is how
    a caller distinguishes "no legs" from "a leg I could not find".

    Parameters
    ----------
    job : dict
        A job mapping.

    Returns
    -------
    dict
        Each leg's entry, keyed by its ``target``.
    """
    include = job.get("strategy", {}).get("matrix", {}).get("include")
    if not isinstance(include, list):
        return {}
    legs: dict[str, dict[str, Any]] = {}
    for entry in include:
        assert isinstance(entry, dict), f"matrix include entry must be a mapping: {entry!r}"
        selector = entry.get(LEG_KEY)
        assert isinstance(selector, str), f"matrix include entry must declare {LEG_KEY}"
        legs[selector] = entry
    return legs


def lane_label(lane: RunnerLane) -> str:
    """Return the runner label a lane actually runs on.

    A plain job's ``runs-on`` is its label. A matrix job defers to the matrix,
    and the label is the leg entry's ``os``. Any other expression is refused:
    a contract that guessed at one would report a label the workflow does not
    use, which is worse than reporting that it cannot read it.

    Parameters
    ----------
    lane : RunnerLane
        The lane to resolve.

    Returns
    -------
    str
        The runner label, with no expression left in it.
    """
    job = load_lane(lane)
    runs_on = job.get("runs-on")
    assert isinstance(runs_on, str), f"{lane} must declare a string runs-on"
    assert "\n" not in runs_on, (
        f"{lane} declares a runs-on containing a line break, which YAML keeps "
        f"inside the value and GitHub then evaluates as written: {runs_on!r}"
    )
    if lane.leg is None:
        assert "${{" not in runs_on, f"{lane} declares no leg, so its runs-on must be a label"
        return runs_on
    assert runs_on == MATRIX_RUNNER_EXPRESSION, (
        f"{lane} names a leg, so its job must take its label from the matrix, "
        f"not from {runs_on!r}"
    )
    legs = matrix_legs(job)
    assert lane.leg in legs, f"{lane} must name a declared matrix leg; got {sorted(legs)}"
    label = legs[lane.leg].get("os")
    assert isinstance(label, str), f"{lane} must declare an os for its leg"
    return label


#: The guard a step carries when it belongs to the Linux legs of a matrix job
#: and not to the macOS and Windows legs beside them.
LINUX_LEG_GUARD: Final[str] = "runner.os == 'Linux'"

#: The one conditional value form this module resolves, as a template. A
#: matrix job cannot pass a different input per leg any other way.
LINUX_ARM_TEMPLATE: Final[str] = "${{ " + LINUX_LEG_GUARD + " && '{linux}' || '{other}' }}"

#: The same form as a pattern, with both arms captured. A matrix job's input
#: has to name what the other legs receive as well as what the Linux legs do,
#: or a contract reading only the Linux arm leaves three legs unasserted.
LEG_CONDITIONAL_INPUT: Final[typ.Pattern[str]] = re.compile(
    r"\$\{\{\s*" + re.escape(LINUX_LEG_GUARD) + r"\s*"
    r"&&\s*'(?P<linux>[^']*)'\s*"
    r"\|\|\s*'(?P<other>[^']*)'\s*\}\}"
)


def runs_even_when_the_job_fails(condition: object) -> bool:
    """Report whether a step's condition still runs it after a failure.

    ``always()`` is the whole condition on a job with one placement. A matrix
    job's step says ``always() && <leg guard>``, which is the same promise
    narrowed to the legs the rule is about; a rule that demanded the bare form
    would refuse the narrower one and push the guard somewhere weaker.

    Only those two are accepted. Anything else beginning ``always() &&`` is
    refused, because the conjunction can just as easily switch the step off:
    ``always() && false`` starts with the same six characters and never runs,
    which would let the headroom, cache-observation and compiler-cache
    contracts pass with the telemetry they exist to require turned off.

    Parameters
    ----------
    condition : object
        A step's ``if`` value, parsed.

    Returns
    -------
    bool
        True when the step runs on failure.

    Examples
    --------
    >>> runs_even_when_the_job_fails("always()")
    True
    >>> runs_even_when_the_job_fails("always() && runner.os == 'Linux'")
    True
    >>> runs_even_when_the_job_fails("runner.os == 'Linux'")
    False
    >>> runs_even_when_the_job_fails("always() && false")
    False
    >>> runs_even_when_the_job_fails(None)
    False
    """
    text = str(condition).strip()
    return text in {"always()", f"always() && {LINUX_LEG_GUARD}"}


def linux_arm(value: object) -> str:
    """Return what an input resolves to on a Linux leg.

    A literal resolves to itself, which is every non-matrix job here. The only
    expression understood is the leg-conditional one a matrix job needs in
    order to pass one input to its Linux legs and another to the rest; any
    other expression is refused, because reporting a guessed value would be
    worse than reporting that it cannot be read.

    Both arms must be present. An expression with only the Linux arm parses
    and yields the same answer for this function, while leaving whatever the
    other three legs receive unasserted, so it is refused here rather than
    silently half-read.

    Parameters
    ----------
    value : object
        A step input's declared value.

    Returns
    -------
    str
        The value a Linux leg receives.

    Examples
    --------
    >>> linux_arm("external")
    'external'
    >>> linux_arm("${{ runner.os == 'Linux' && 'external' || 'github' }}")
    'external'
    >>> linux_arm("${{ runner.os == 'Linux' && 'external' }}")
    Traceback (most recent call last):
    ...
    AssertionError: only the leg-conditional form with both arms is readable...
    """
    text = str(value)
    if "${{" not in text:
        return text
    match = LEG_CONDITIONAL_INPUT.fullmatch(text)
    assert match is not None, (
        "only the leg-conditional form with both arms is readable here, "
        f"{LINUX_ARM_TEMPLATE!r}; got {text!r}"
    )
    return match["linux"]


def _runner_declarations(job: dict[str, Any]) -> list[object]:
    """Return the runner declarations a job resolves through.

    A matrix job defers to its legs, so each leg's image is a declaration and
    the job's own ``runs-on`` is only the deferral. A reusable-workflow caller
    declares nothing: the called workflow places its own jobs.

    Parameters
    ----------
    job : dict
        A job mapping, as parsed from a workflow.

    Returns
    -------
    list
        One declaration per leg, or the job's own ``runs-on``, or nothing.

    Examples
    --------
    >>> _runner_declarations({"runs-on": "ubuntu-latest"})
    ['ubuntu-latest']
    >>> _runner_declarations({"uses": "org/repo/.github/workflows/w.yml@sha"})
    []
    """
    legs = matrix_legs(job)
    if legs:
        return [entry.get("os") for entry in legs.values()]
    if "runs-on" not in job and isinstance(job.get("uses"), str):
        return []
    return [job.get("runs-on")]


def _labels_from_declaration(value: object) -> set[str]:
    """Return the labels one runner declaration can resolve to.

    The matrix deferral names no label of its own. A conditional contributes
    both arms, because which one a run bills for depends on the head that
    triggered it. Anything else is a literal.

    Parameters
    ----------
    value : object
        One ``runs-on`` declaration, as parsed.

    Returns
    -------
    set of str
        The labels that declaration can resolve to.

    Raises
    ------
    AssertionError
        If the declaration is a shape this reader cannot inventory.

    Examples
    --------
    >>> _labels_from_declaration("ubicloud-standard-2")
    {'ubicloud-standard-2'}
    >>> _labels_from_declaration(MATRIX_RUNNER_EXPRESSION)
    set()
    """
    assert isinstance(value, str), (
        "runs-on here is a label or the matrix deferral. The list form and "
        "the group/labels mapping are valid GitHub and this reader cannot "
        "inventory them, so a job using one is refused rather than "
        f"contributing nothing to the registry question; got {value!r}"
    )
    if value == MATRIX_RUNNER_EXPRESSION:
        return set()
    conditional = CONDITIONAL_RUNS_ON.match(value)
    if conditional:
        return {conditional["when_true"], conditional["when_false"]}
    return {value}


def declared_labels(job: dict[str, Any]) -> set[str]:
    """Return every runner label a job could resolve to.

    A matrix job resolves through its legs, so the legs' images are the
    labels and the `${{ matrix.os }}` deferral itself is not one. A
    conditional contributes both arms, because which one a run bills for
    depends on the head that triggered it. Anything else is a literal.

    A job that calls a reusable workflow declares no ``runs-on`` at all: the
    called workflow places its own jobs and this one bills for nothing, so it
    contributes no labels. Every other shape the reader cannot inventory is
    refused rather than skipped, because skipping answers "no labels" for a
    job that may bill for several and a registry question must never be
    answered quietly.

    Parameters
    ----------
    job : dict
        A job mapping, as parsed from a workflow.

    Returns
    -------
    set of str
        Every label the job could resolve to, with no expression left in it.

    Raises
    ------
    AssertionError
        If ``runs-on`` is a shape this reader cannot inventory, such as the
        list form or the ``group``/``labels`` mapping. A reusable-workflow
        caller is exempt: it declares no ``runs-on`` and bills for nothing.

    Examples
    --------
    >>> declared_labels({"runs-on": "ubuntu-latest"})
    {'ubuntu-latest'}
    >>> sorted(
    ...     declared_labels(
    ...         {
    ...             "runs-on": "${{ github.event.pull_request.head.repo.fork"
    ...             " && 'ubuntu-latest' || 'ubicloud-standard-2' }}"
    ...         }
    ...     )
    ... )
    ['ubicloud-standard-2', 'ubuntu-latest']
    """
    declared = _runner_declarations(job)
    labels: set[str] = set()
    for value in declared:
        labels.update(_labels_from_declaration(value))
    return labels


def billable_labels(job: dict[str, Any]) -> set[str]:
    """Return the labels a job can bill for.

    Everything it could resolve to, less what GitHub hosts itself.

    Parameters
    ----------
    job : dict
        A job mapping, as parsed from a workflow.

    Returns
    -------
    set of str
        The labels that cost money, which are the ones a registry must
        account for.

    Examples
    --------
    >>> billable_labels({"runs-on": "ubuntu-latest"})
    set()
    >>> billable_labels({"runs-on": "ubicloud-standard-2-ubuntu-2404"})
    {'ubicloud-standard-2-ubuntu-2404'}
    """
    return declared_labels(job) - GITHUB_HOSTED_LABELS
