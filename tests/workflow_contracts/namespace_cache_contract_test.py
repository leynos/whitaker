"""Validate Namespace cache ownership, tool provenance, and observability."""

from pathlib import Path
from typing import Any

import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/ci.yml"
CACHE_ACTION = (
    "namespacelabs/nscloud-cache-action@c5f8dab7560444c4bf8dbc64f1b203431873c547"
)
SETUP_RUST_ACTION = (
    "leynos/shared-actions/.github/actions/setup-rust@"
    "5daae0a332441d170d88ca648c9e71f0bbe96cb3"
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
        assert "NAMESPACE_CACHE_HIT" in cache_summary["run"]
        assert names.index("Reset sccache statistics") < names.index(
            "Record sccache effectiveness"
        )
        stats_script = steps["Record sccache effectiveness"]["run"]
        assert "sccache --show-stats" in stats_script
        assert "--stats-format json" in stats_script
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

    merman_script = _steps_by_name(jobs["linux-full"])["Install Merman CLI"]["run"]
    assert "sha256sum --check" in merman_script
    assert "merman-cli 0.7.0" in merman_script

    nixie_script = _steps_by_name(jobs["linux-full"])["Install Nixie"]["run"]
    assert "if command -v nixie" in nixie_script
    assert 'uv tool install --force --python 3.14 "nixie-cli==1.1.0"' in (
        nixie_script
    )
    assert nixie_script.count("command -v nixie") == 2


def test_namespace_jobs_share_one_repository_cache_tag() -> None:
    """Avoid duplicate volumes while retaining different measured shapes."""
    jobs = _load_jobs()
    runners = {jobs[job_name]["runs-on"] for job_name in NAMESPACE_JOBS}

    assert runners == {
        "namespace-profile-rust-linux-light;"
        "overrides.cache-tag=whitaker-linux-amd64-v1",
        "namespace-profile-rust-linux-ci;overrides.cache-tag=whitaker-linux-amd64-v1",
    }
