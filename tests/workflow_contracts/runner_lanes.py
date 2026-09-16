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

#: What a cache key must vary by inside a matrix job. Both Linux legs of the
#: rolling release run one restore step and one save step, so only the
#: rendered key keeps their archives apart.
LEG_DISCRIMINATOR: Final[str] = "${{ runner.arch }}"


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


def runs_even_when_the_job_fails(condition: object) -> bool:
    """Report whether a step's condition still runs it after a failure.

    ``always()`` is the whole condition on a job with one placement. A matrix
    job's step says ``always() && <leg guard>``, which is the same promise
    narrowed to the legs the rule is about; a rule that demanded the bare form
    would refuse the narrower one and push the guard somewhere weaker.

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
    >>> runs_even_when_the_job_fails(None)
    False
    """
    text = str(condition).strip()
    return text == "always()" or text.startswith("always() && ")


def linux_arm(value: object) -> str:
    """Return what an input resolves to on a Linux leg.

    A literal resolves to itself, which is every non-matrix job here. The only
    expression understood is the leg-conditional one a matrix job needs in
    order to pass one input to its Linux legs and another to the rest; any
    other expression is refused, because reporting a guessed value would be
    worse than reporting that it cannot be read.

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
    """
    text = str(value)
    if "${{" not in text:
        return text
    prefix = "${{ " + LINUX_LEG_GUARD + " && '"
    assert text.startswith(prefix) and text.endswith("' }}"), (
        f"only the leg-conditional form is readable here; got {text!r}"
    )
    return text[len(prefix) :].split("'", 1)[0]
