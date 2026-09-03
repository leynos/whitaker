"""Validate Namespace cache ownership, tool provenance, and observability."""

import shlex
from pathlib import Path
from typing import Any

import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github/workflows/ci.yml"
COVERAGE_WORKFLOW_PATH = REPO_ROOT / ".github/workflows/coverage-main.yml"
DYLINT_TOOLS_SCRIPT = REPO_ROOT / "scripts/install-dylint-tools.sh"
CACHE_STATE_SCRIPT = "scripts/record-namespace-cache-state.sh"
CLIPPY_MIRROR_SCRIPT = "scripts/provision-clippy-mirror.sh"
SCCACHE_STATS_SCRIPT = "scripts/record-sccache-effectiveness.sh"
CLIPPY_MIRROR_CACHE_PATH = "~/.cache/whitaker-mirrors"
CACHE_ACTION = (
    "namespacelabs/nscloud-cache-action@c5f8dab7560444c4bf8dbc64f1b203431873c547"
)
SETUP_RUST_ACTION = (
    "leynos/shared-actions/.github/actions/setup-rust@"
    "5daae0a332441d170d88ca648c9e71f0bbe96cb3"
)
INSTALL_NIXIE_ACTION = (
    "leynos/shared-actions/.github/actions/install-nixie@"
    "bffacaf91d3f3515110679a30fbf6dc781ddc549"
)
NAMESPACE_JOBS = {
    "coverage-check": 2,
    "linux-full": 4,
}


def _load_jobs() -> dict[str, dict[str, Any]]:
    """Load the CI jobs as workflow mappings."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), "CI workflow must parse to a mapping"
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "CI workflow must declare a jobs mapping"
    return jobs


def _steps_by_name(job: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Index the named workflow steps for one job."""
    steps = job.get("steps")
    assert isinstance(steps, list), "Namespace job must declare a step list"
    return {step["name"]: step for step in steps if isinstance(step.get("name"), str)}


def _step_names(job: dict[str, Any]) -> list[str]:
    """Return the ordered names from one workflow job."""
    return list(_steps_by_name(job))


def _recorded_cache_paths(run_script: str) -> set[str]:
    """Return the paths the cache-state recorder is asked to measure.

    The first argument is the cache tag; the rest are cached paths.
    """
    words = shlex.split(run_script.replace("\\\n", " "))
    start = words.index(CACHE_STATE_SCRIPT)
    return set(words[start + 2 :])


def _mounted_cache_paths(cache_step: dict[str, Any]) -> set[str]:
    """Return the paths the Namespace cache action mounts."""
    return {line.strip() for line in cache_step["with"]["path"].splitlines() if line.strip()}


def test_namespace_jobs_have_one_external_cache_owner() -> None:
    """Require Namespace caching before Rust setup and every build consumer."""
    jobs = _load_jobs()
    for job_name in NAMESPACE_JOBS:
        job = jobs[job_name]
        steps = _steps_by_name(job)
        names = _step_names(job)
        cache_step = steps["Set up Namespace cache"]
        setup_step = steps["Setup Rust"]
        cache_actions = [
            step for step in job["steps"] if step.get("uses") == CACHE_ACTION
        ]

        assert len(cache_actions) == 1, (
            f"{job_name} must declare exactly one Namespace cache action"
        )
        assert cache_step["uses"] == CACHE_ACTION, (
            f"{job_name} must use the pinned Namespace cache action"
        )
        assert "cache" not in cache_step["with"], (
            f"{job_name} must configure explicit cache paths"
        )
        cached_paths = cache_step["with"]["path"]
        assert "~/.cargo/registry" in cached_paths, (
            f"{job_name} must cache Cargo registry downloads"
        )
        assert "~/.cache/uv" in cached_paths, (
            f"{job_name} must cache uv downloads"
        )
        assert "~/.local/share/uv" in cached_paths, (
            f"{job_name} must cache installed uv tool environments"
        )
        assert "~/.local/bin" in cached_paths, (
            f"{job_name} must cache uv tool executable shims"
        )
        assert names.index("Set up Namespace cache") < names.index("Setup Rust"), (
            f"{job_name} must mount its cache before Rust setup"
        )
        assert setup_step["uses"] == SETUP_RUST_ACTION, (
            f"{job_name} must use the pinned shared Rust setup action"
        )
        assert setup_step["with"] == {
            "cache-provider": "external",
            "use-sccache": False,
        }, f"{job_name} must delegate cache ownership to Namespace"
        assert not any(
            str(step.get("uses", "")).startswith("actions/cache@")
            for step in steps.values()
        ), f"{job_name} must not mix GitHub and Namespace caches"


def test_namespace_jobs_report_volume_and_compiler_cache_results() -> None:
    """Require cache-hit, sccache statistics, and retained JSON evidence."""
    jobs = _load_jobs()
    for job_name in NAMESPACE_JOBS:
        steps = _steps_by_name(jobs[job_name])
        names = _step_names(jobs[job_name])

        assert steps["Set up Namespace cache"]["id"] == "namespace-cache"
        cache_summary = steps["Record Namespace cache state"]
        assert cache_summary["env"]["NAMESPACE_CACHE_HIT"] == (
            "${{ steps.namespace-cache.outputs.cache-hit }}"
        )
        assert CACHE_STATE_SCRIPT in cache_summary["run"], (
            f"{job_name} must record cache state through the shared recorder"
        )
        recorded = _recorded_cache_paths(cache_summary["run"])
        mounted = _mounted_cache_paths(steps["Set up Namespace cache"])
        assert recorded == mounted, (
            f"{job_name} must report the restored size of every mounted path; "
            f"mounted {sorted(mounted)}, recorded {sorted(recorded)}"
        )
        assert names.index("Reset sccache statistics") < names.index(
            "Record sccache effectiveness"
        )
        assert SCCACHE_STATS_SCRIPT in steps["Record sccache effectiveness"]["run"]
        upload_step = steps["Upload sccache statistics"]
        assert upload_step["with"]["path"] == "sccache-stats.json"
        assert upload_step["with"]["retention-days"] == 14


def test_namespace_jobs_bound_nextest_to_their_profile_size() -> None:
    """Keep test concurrency at or below each allocated vCPU count."""
    jobs = _load_jobs()
    for job_name, expected_threads in NAMESPACE_JOBS.items():
        assert jobs[job_name]["env"]["NEXTEST_TEST_THREADS"] == expected_threads


def test_namespace_tool_installers_cannot_compile_fallbacks() -> None:
    """Reject source-building tool installers in paid Namespace jobs."""
    jobs = _load_jobs()
    for job_name in NAMESPACE_JOBS:
        steps = _steps_by_name(jobs[job_name])
        run_scripts = "\n".join(
            str(step.get("run", "")) for step in steps.values() if "run" in step
        )
        assert "cargo install" not in run_scripts
        for step in steps.values():
            if str(step.get("uses", "")).startswith("taiki-e/install-action@"):
                assert step.get("with", {}).get("fallback") == "none"
        sccache_step = steps["Install sccache"]
        assert sccache_step["uses"] == (
            "taiki-e/install-action@18b1216eba7f8039b0f8d131d5473787f0edce68"
        )
        assert sccache_step["with"]["tool"] == "sccache@0.16.0"

    mdtablefix_step = _steps_by_name(jobs["linux-full"])["Install mdtablefix"]
    mdtablefix_script = mdtablefix_step["run"]
    assert "releases/download/v${MDTABLEFIX_VERSION}/mdtablefix-linux-x86_64" in (
        mdtablefix_script
    )
    assert "sha256sum --check --status" in mdtablefix_script
    assert "cargo install" not in mdtablefix_script
    cache_paths = _steps_by_name(jobs["linux-full"])["Set up Namespace cache"][
        "with"
    ]["path"]
    assert "~/.cache/mdtablefix-build" not in cache_paths
    assert "~/.cache/merman" in cache_paths

    linux_full = jobs["linux-full"]
    linux_steps = _steps_by_name(linux_full)
    linux_step_names = _step_names(linux_full)
    nixie_step = linux_steps["Install Nixie"]
    assert "Install Merman CLI" not in linux_steps
    assert nixie_step == {
        "name": "Install Nixie",
        "uses": INSTALL_NIXIE_ACTION,
        "with": {
            "nixie-version": "1.1.0",
            "merman-version": "0.7.0",
            "python-version": "3.14",
        },
    }
    assert linux_step_names.index("Set up Namespace cache") < linux_step_names.index(
        "Install Nixie"
    )
    assert linux_step_names.index("Setup uv") < linux_step_names.index("Install Nixie")
    assert linux_step_names.index("Install Nixie") < linux_step_names.index("Nixie")


def test_namespace_jobs_share_one_repository_cache_tag() -> None:
    """Avoid duplicate volumes while retaining different measured shapes."""
    jobs = _load_jobs()
    runners = {jobs[job_name]["runs-on"] for job_name in NAMESPACE_JOBS}

    assert runners == {
        "namespace-profile-rust-linux-light;"
        "overrides.cache-tag=whitaker-linux-amd64-v1",
        "namespace-profile-rust-linux-ci;overrides.cache-tag=whitaker-linux-amd64-v1",
    }


def _load_coverage_steps() -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    """Load the main-branch coverage job's steps in order and by name."""
    workflow = yaml.safe_load(COVERAGE_WORKFLOW_PATH.read_text(encoding="utf-8"))
    job = workflow["jobs"]["coverage-upload"]
    return job["steps"], _steps_by_name(job)


def test_clippy_source_fetch_has_one_cache_owner() -> None:
    """Require a cached Clippy mirror before anything builds `dylint_driver`.

    `dylint_driver`'s build script clones rust-lang/rust-clippy to read
    `clippy_utils/src/sym.rs`. Left unowned, that clone repeats on every cold
    build and fails when GitHub rejects the unauthenticated request.
    """
    jobs = _load_jobs()
    for job_name in NAMESPACE_JOBS:
        job = jobs[job_name]
        steps = _steps_by_name(job)
        names = _step_names(job)
        cached_paths = _mounted_cache_paths(steps["Set up Namespace cache"])
        assert CLIPPY_MIRROR_CACHE_PATH in cached_paths, (
            f"{job_name} must cache the Clippy mirror's parent directory"
        )
        mirror_step = steps["Provision the Clippy source mirror"]
        assert CLIPPY_MIRROR_SCRIPT in mirror_step["run"]
        assert mirror_step["id"] == "clippy-mirror"
        assert names.index("Set up Namespace cache") < names.index(
            "Provision the Clippy source mirror"
        ), f"{job_name} must mount the cache before populating the mirror"
        assert names.index("Provision the Clippy source mirror") < names.index(
            "Setup Rust"
        ), f"{job_name} must provision the mirror before any Cargo build"


def test_clippy_mirror_is_not_the_cache_mount_point() -> None:
    """Keep the mirror below the mounted directory, never at it.

    A cold volume materializes the mount point as a directory, so a mirror
    placed at the mount point cannot be replaced when a stale generation is
    discarded.
    """
    jobs = _load_jobs()
    for job_name in NAMESPACE_JOBS:
        run_script = _steps_by_name(jobs[job_name])[
            "Provision the Clippy source mirror"
        ]["run"]
        assert "whitaker-mirrors/rust-clippy.git" in run_script, (
            f"{job_name} must place the mirror below the cached directory"
        )


def test_only_one_namespace_job_can_publish_a_cache_generation() -> None:
    """Designate a single writer for the shared repository cache tag.

    Both Namespace jobs attach `whitaker-linux-amd64-v1`. The deployed
    profiles only publish generations from `main`, and `coverage-check` is
    restricted to pull requests, so `linux-full` is the sole writer and no
    two lanes can populate the same cold key concurrently.
    """
    jobs = _load_jobs()
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    triggers = set(workflow[True] if True in workflow else workflow["on"])
    assert triggers == {"pull_request", "workflow_dispatch"}
    assert jobs["coverage-check"]["if"] == "github.event_name == 'pull_request'", (
        "coverage-check must stay pull-request only so it never writes a "
        "cache generation"
    )
    assert "if" not in jobs["linux-full"], (
        "linux-full is the designated cache writer and must run on dispatch"
    )


def test_coverage_upload_owns_its_clippy_mirror_and_measures_sccache() -> None:
    """Hold the GitHub-hosted coverage job to the same cache contract."""
    workflow = yaml.safe_load(COVERAGE_WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert workflow["env"]["RUSTC_WRAPPER"] == "sccache", (
        "coverage must route nested Cargo builds through the compiler cache"
    )
    ordered, steps = _load_coverage_steps()
    names = [step["name"] for step in ordered if isinstance(step.get("name"), str)]

    cache_step = steps["Cache the Clippy source mirror"]
    assert cache_step["uses"].startswith("actions/cache@")
    assert cache_step["with"]["path"] == CLIPPY_MIRROR_CACHE_PATH
    assert CLIPPY_MIRROR_SCRIPT in steps["Provision the Clippy source mirror"]["run"]
    assert names.index("Cache the Clippy source mirror") < names.index(
        "Provision the Clippy source mirror"
    )
    assert names.index("Provision the Clippy source mirror") < names.index(
        "Generate coverage"
    )
    assert names.index("Reset sccache statistics") < names.index("Generate coverage")
    assert SCCACHE_STATS_SCRIPT in steps["Record sccache effectiveness"]["run"]

    checkout = steps["Checkout"]
    assert "@v" not in checkout["uses"], "pin the checkout action by commit SHA"
    for step in ordered:
        if str(step.get("uses", "")).startswith("taiki-e/install-action@"):
            assert step["with"]["fallback"] == "none"
            assert "@" in step["with"]["tool"], "pin every installed tool version"


def test_dylint_host_tools_are_not_built_from_source() -> None:
    """Reject the Cargo source build for the pinned Dylint host tools."""
    script = DYLINT_TOOLS_SCRIPT.read_text(encoding="utf-8")
    assert "cargo install" not in script, (
        "install-dylint-tools.sh must use the checksum-verified prebuilt "
        "release archives"
    )
    assert "sha256" in script.lower(), (
        "install-dylint-tools.sh must verify a pinned digest"
    )
