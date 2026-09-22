"""Shared loaders for the Ubicloud workflow contract tests.

The contract tests read the checked-in workflows as data rather than running
them, so they need a small vocabulary for "the jobs that run on Ubicloud",
"the cache steps in a job", and "the paths a cache step owns". Keeping that
vocabulary here lets each contract module stay focused on one policy.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path
from typing import Any, Final

import yaml

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Iterable

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS_DIRECTORY = REPOSITORY_ROOT / ".github/workflows"

#: `actions/cache` v6.1.0. Ubicloud's transparent cache intercepts this
#: version, so Linux archives land in Ubicloud's store and Windows archives
#: land in GitHub's, from one action and one pin. The deprecated
#: `ubicloud/cache` fork is deliberately not used.
CACHE_ACTION_SHA = "55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
RESTORE_ACTION = f"actions/cache/restore@{CACHE_ACTION_SHA}"
SAVE_ACTION = f"actions/cache/save@{CACHE_ACTION_SHA}"
INSTALL_ACTION = "taiki-e/install-action@18b1216eba7f8039b0f8d131d5473787f0edce68"
#: shared-actions `main` at the merge of #458. It keeps the rule that its
#: built-in `github` cache provider does not archive `target/<profile>`, so no
#: shared action can become the second owner of compiler output that `sccache`
#: already owns, and it adds two things the previous pin lacked: it restores
#: the caller's Actions cache-service selection, and it starts the `sccache`
#: server from a `run:` step after those exports, which is what makes the
#: shared action's own compiler-cache arm work on Ubicloud.
#:
#: Every caller in this repository pins this one revision again. The lanes were
#: split while the release boundaries waited for evidence, and the evidence
#: arrived from the rolling release rather than from a tag: whitaker publishes
#: on every merge to `main`, so those lanes run on every merge and had already
#: been exercising the newer wiring by the time the split was written.
#:
#: Asserted by value rather than by shape, so a bump has to update this
#: constant and someone has to confirm the new revision still leaves this
#: repository the sole owner of its caches.
SHARED_ACTIONS_REF: Final[str] = "7cb894fe62c40951cccf33819548095e64a1291e"

SETUP_RUST_ACTION = f"leynos/shared-actions/.github/actions/setup-rust@{SHARED_ACTIONS_REF}"

#: Every Ubicloud job, mapped to the workflow that declares it.
UBICLOUD_JOBS: dict[str, str] = {
    "coverage-check": "ci.yml",
    "linux-full": "ci.yml",
    "coverage-upload": "coverage-main.yml",
    # A matrix job, and only two of its five legs are on Ubicloud. The rules
    # keyed on this mapping are about the step list, which every leg shares,
    # so the job belongs here whole; which legs run where is `RunnerLane`'s
    # to say, and `runner_placement_contract_test` holds the two together.
    "build-lints": "rolling-release.yml",
    "build-dependency-binaries": "rolling-release.yml",
}

#: Every job that runs this repository's own gates: the suite, the lints, the
#: coverage run. They share a shape that a build job does not have, so the
#: rules about that shape are keyed here rather than on every Ubicloud job.
#: Nextest concurrency, the Clippy source mirror and the tools cache are all
#: rules about running the gates, and `build-lints` runs none of them; it
#: cross-compiles the lint crates for five targets and packages them.
SUITE_JOBS: dict[str, str] = {
    "coverage-check": "ci.yml",
    "linux-full": "ci.yml",
    "coverage-upload": "coverage-main.yml",
}

#: Every job that owns cache archives, including the GitHub-hosted Windows
#: lane. Cache ownership rules apply to all of them; Ubicloud-specific rules
#: such as the backend selector apply only to `UBICLOUD_JOBS`.
CACHING_JOBS: dict[str, str] = UBICLOUD_JOBS | {"windows-compat": "ci.yml"}

#: The single job permitted to save each cache key family.
CACHE_KEY_WRITERS: dict[str, str] = {
    "cargo-registry-coverage-v1-": "coverage-upload",
    "cargo-registry-lint-v1-": "linux-full",
    "tools-coverage-v1-": "coverage-upload",
    "tools-lint-v1-": "linux-full",
    "dylint-tools-v1-": "linux-full",
    "clippy-mirror-v1-": "coverage-upload",
    # The `sccache-coverage-v1-` and `sccache-lint-v1-` families are gone, not
    # merely unlisted. Both Linux workflows run sccache on the `gha` backend
    # against Ubicloud's cache proxy, so no lane archives `~/.cache/sccache`
    # and there is no directory for a family to name. The rolling-release
    # lanes below stay on the local-directory backend and keep theirs.
    "cargo-registry-windows-v1-": "windows-compat",
    "cargo-registry-rolling-v1-": "build-lints",
    "sccache-rolling-v1-": "build-lints",
    "cargo-registry-depbin-v1-": "build-dependency-binaries",
    "sccache-depbin-v1-": "build-dependency-binaries",
}


class DuplicateKeyError(yaml.constructor.ConstructorError):
    """A workflow declares one mapping key twice."""


class StrictSafeLoader(yaml.SafeLoader):
    """A `SafeLoader` that refuses a mapping declaring one key twice.

    PyYAML keeps the last value of a repeated key and says nothing, so a job
    declaring `runs-on` twice parses into a document that has discarded the
    first label. A contract reading that document judges a lane GitHub may not
    run the way it reads, and passes on a file it never saw whole.
    """

    def construct_mapping(
        self, node: yaml.MappingNode, deep: bool = False
    ) -> dict[typ.Hashable, Any]:
        """Build one mapping, refusing a key declared twice in it.

        Parameters
        ----------
        node : yaml.MappingNode
            The mapping node being constructed.
        deep : bool, optional
            Whether nested values are constructed now rather than lazily, as
            PyYAML's own `construct_mapping` takes it.

        Returns
        -------
        dict[typ.Hashable, Any]
            The constructed mapping, built by PyYAML once no key repeats.

        Raises
        ------
        DuplicateKeyError
            When two keys of the mapping construct to the same value.
        """
        seen: set[typ.Hashable] = set()
        for key_node, _ in node.value:
            key = self.construct_object(key_node, deep=deep)
            if key in seen:
                raise DuplicateKeyError(
                    "while constructing a mapping",
                    node.start_mark,
                    f"found duplicate key {key!r}",
                    key_node.start_mark,
                )
            seen.add(key)
        return super().construct_mapping(node, deep=deep)


def parse_workflow(text: str) -> object:
    """Parse workflow text strictly, refusing duplicate mapping keys.

    Every contract that reads a workflow goes through this rather than
    `yaml.safe_load`, so a repeated key fails the suite instead of silently
    losing one of its values.

    Parameters
    ----------
    text : str
        A workflow file's text.

    Returns
    -------
    object
        The parsed document. A workflow parses to a mapping; the caller checks
        that, because a malformed file may parse to anything.

    Raises
    ------
    DuplicateKeyError
        When any mapping in the document declares one key twice.

    >>> parse_workflow("on: push\\njobs: {}\\n")
    {True: 'push', 'jobs': {}}
    """
    return yaml.load(text, Loader=StrictSafeLoader)  # noqa: S506 - a SafeLoader subclass.


def load_workflow(workflow_name: str) -> dict[str, Any]:
    """Return one checked-in workflow parsed as a mapping.

    For example, ``load_workflow("ci.yml")["jobs"]`` yields the CI job set.
    """
    workflow_path = WORKFLOWS_DIRECTORY / workflow_name
    workflow = parse_workflow(workflow_path.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), f"{workflow_name} must parse to a mapping"
    return workflow


def load_job(job_name: str) -> dict[str, Any]:
    """Return one cache-owning job mapping by name.

    For example, ``load_job("linux-full")["steps"]`` yields its step list.
    """
    workflow = load_workflow(CACHING_JOBS[job_name])
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "workflow must declare a jobs mapping"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"workflow must declare the {job_name} job"
    return job


def all_jobs() -> list[tuple[str, str, dict[str, Any]]]:
    """Return every job in every checked-in workflow as a name triple.

    For example, a caller can scan the result for jobs whose inline scripts
    execute the test suite, without listing the workflow files itself.
    """
    discovered: list[tuple[str, str, dict[str, Any]]] = []
    for pattern in ("*.yml", "*.yaml"):
        for workflow_path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            jobs = load_workflow(workflow_path.name).get("jobs")
            if not isinstance(jobs, dict):
                continue
            discovered.extend(
                (workflow_path.name, job_name, job)
                for job_name, job in jobs.items()
                if isinstance(job, dict)
            )
    return discovered


def job_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the ordered step list for one job."""
    steps = job.get("steps")
    assert isinstance(steps, list), "job must declare a step list"
    return steps


def step_names(job: dict[str, Any]) -> list[str]:
    """Return the ordered step names for one job.

    Every step must be named. Silently dropping an unnamed step would let it
    sit anywhere in the order, including ahead of the compiler-cache
    credential export whose position the ordering contracts police.
    """
    names: list[str] = []
    for index, step in enumerate(job_steps(job)):
        name = step.get("name")
        assert isinstance(name, str), f"step {index} must declare a name: {step!r}"
        names.append(name)
    return names


def steps_by_name(job: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Index a job's named steps.

    For example, ``steps_by_name(job)["Setup Rust"]`` returns that step.
    """
    return {step["name"]: step for step in job_steps(job) if "name" in step}


def cache_paths(step: dict[str, Any]) -> list[str]:
    """Return the paths one cache step owns, in declaration order.

    For example, a step whose ``path`` is a two-line block returns both
    entries so a caller can detect two steps claiming the same directory.
    """
    raw = step.get("with", {}).get("path", "")
    return [line.strip() for line in str(raw).splitlines() if line.strip()]


def duplicate_path_owners(steps: list[dict[str, Any]]) -> dict[str, list[str]]:
    """Return each cached path claimed by more than one of ``steps``.

    Ownership is the invariant the whole cache design rests on, so it is
    computed here as a pure function over a step list rather than asserted
    inline: the concrete workflows and the generated cases in the property
    suite then exercise one implementation.

    For example, two steps that both list ``~/.cargo/bin`` yield
    ``{"~/.cargo/bin": ["Restore Cargo registry", "Restore the tools"]}``.
    """
    owners: dict[str, list[str]] = {}
    for step in steps:
        name = str(step.get("name", ""))
        for path in cache_paths(step):
            owners.setdefault(path, []).append(name)
    return {path: names for path, names in owners.items() if len(names) > 1}


def key_family(key: str, families: Iterable[str]) -> str | None:
    """Return the reviewed key family a rendered cache key belongs to.

    The longest matching prefix wins, so a family that is itself a prefix of
    another never captures the more specific one. Returns ``None`` when no
    family claims the key, which the contract treats as an unreviewed key.

    For example, ``key_family("sccache-lint-v1-Linux", CACHE_KEY_WRITERS)``
    returns ``"sccache-lint-v1-"``.
    """
    matches = [family for family in families if key.startswith(family)]
    return max(matches, key=len) if matches else None


def restore_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the job's cache restore steps in declaration order."""
    return [step for step in job_steps(job) if step.get("uses") == RESTORE_ACTION]


def save_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the job's cache save steps in declaration order."""
    return [step for step in job_steps(job) if step.get("uses") == SAVE_ACTION]


def run_scripts(job: dict[str, Any]) -> str:
    """Return every inline shell script in a job, joined for substring checks."""
    return "\n".join(str(step.get("run", "")) for step in job_steps(job))


#: The directory the local-disk sccache backend writes to, as every cache step
#: that has ever owned it spells it.
SCCACHE_DIRECTORY: Final[str] = "~/.cache/sccache"

#: The step that republishes Ubicloud's cache-proxy credentials through
#: `GITHUB_ENV`. Asserted by name because the ordering rules are about where it
#: sits in the step list, and by action because a step that merely carries the
#: name would satisfy the ordering while exporting nothing.
CREDENTIALS_STEP: Final[str] = "Export the Ubicloud cache credentials"
CREDENTIALS_ACTION_PATH: Final[str] = (
    "leynos/shared-actions/.github/actions/export-ubicloud-cache-credentials"
)

#: What `scripts/select-sccache-backend.sh` assumes when `SCCACHE_BACKEND` is
#: unset or empty, kept here so the contract reads the same default the lanes
#: would actually run under rather than treating an omission as a failure.
DEFAULT_BACKEND: Final[str] = "gha"


def backend_for(workflow_name: str) -> str:
    """Return the compiler-cache backend one workflow selects.

    The workflows declare `SCCACHE_BACKEND` once, at workflow level, and
    `scripts/select-sccache-backend.sh` translates it into exactly one set of
    sccache variables. For example, ``backend_for("ci.yml")`` returns
    ``"gha"``.
    """
    env = load_workflow(workflow_name).get("env", {})
    declared = str(env.get("SCCACHE_BACKEND", "")).strip()
    return declared or DEFAULT_BACKEND


def sccache_directory_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the job's cache steps that own the local sccache directory.

    Restores and saves together, because the rule they answer to is about the
    directory existing in the job at all, not about which direction it moves.
    """
    return [
        step
        for step in restore_steps(job) + save_steps(job)
        if SCCACHE_DIRECTORY in cache_paths(step)
    ]
