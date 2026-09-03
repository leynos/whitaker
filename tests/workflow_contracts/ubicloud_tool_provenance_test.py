"""Validate tool provenance, concurrency bounds, and compiler-cache wiring."""

from __future__ import annotations

from ubicloud_workflow_support import (
    INSTALL_ACTION,
    REPOSITORY_ROOT,
    SETUP_RUST_ACTION,
    UBICLOUD_JOBS,
    job_steps,
    load_job,
    load_workflow,
    restore_steps,
    run_scripts,
    step_names,
    steps_by_name,
)

VCPU_CONSTANT = "LINUX_RUNNER_VCPUS"
EXPECTED_VCPUS = "2"
DERIVED_CONCURRENCY_VARIABLES = ("CARGO_BUILD_JOBS", "NEXTEST_TEST_THREADS")

#: Tool pins that must appear verbatim in the lane's tools cache key, so a
#: version bump cannot silently reuse an archive built from the old pins.
TOOL_PINS_IN_KEY: dict[str, tuple[str, ...]] = {
    "coverage-check": ("sccache0.16.0", "nextest0.9.114", "llvmcov0.6.24"),
    "coverage-upload": ("sccache0.16.0", "nextest0.9.114", "llvmcov0.6.24"),
    "linux-full": (
        "sccache0.16.0",
        "nextest0.9.114",
        "msrv1.85.0",
        "bun1.2.21",
        "mmdc11.9.0",
        "uv0.11.19",
        "nixie1.1.0",
        "merman0.7.0",
    ),
}


def _tools_restore_key(job_name: str) -> str:
    """Return the rendered key template of a job's tools cache restore."""
    step = steps_by_name(load_job(job_name))["Restore the Rust toolchain and installed tools"]
    return str(step["with"]["key"])


def test_no_ubicloud_job_compiles_a_tool_from_source() -> None:
    """Paid runner minutes must never be spent rebuilding a published binary."""
    for job_name in UBICLOUD_JOBS:
        job = load_job(job_name)
        assert "cargo install" not in run_scripts(job), (
            f"{job_name} must not build a tool from source"
        )
        for step in job_steps(job):
            if str(step.get("uses", "")).startswith("taiki-e/install-action@"):
                assert step["uses"] == INSTALL_ACTION, (
                    f"{job_name} must use the reviewed installer action pin"
                )
                assert step.get("with", {}).get("fallback") == "none", (
                    f"{job_name}: {step['name']!r} must fail rather than compile"
                )


def test_mdtablefix_is_installed_from_a_checksum_verified_release() -> None:
    """The Markdown formatter defines canonical output, so its build must match."""
    script = str(steps_by_name(load_job("linux-full"))["Install mdtablefix"]["run"])
    assert "releases/download/v${MDTABLEFIX_VERSION}/mdtablefix-linux-x86_64" in script
    assert "sha256sum --check --status" in script
    assert "cargo install" not in script


def test_setup_rust_delegates_cache_and_compiler_cache_ownership() -> None:
    """The shared action must not become a second owner of either cache."""
    for job_name in UBICLOUD_JOBS:
        setup = steps_by_name(load_job(job_name))["Setup Rust"]
        assert setup["uses"] == SETUP_RUST_ACTION, (
            f"{job_name} must use the reviewed shared Rust setup pin"
        )
        assert setup["with"] == {"cache-provider": "external", "use-sccache": False}, (
            f"{job_name} must own its Cargo cache and sccache configuration"
        )


def test_one_named_constant_bounds_build_and_test_concurrency() -> None:
    """Cargo and nextest must never oversubscribe the label's two vCPUs."""
    for workflow_name in set(UBICLOUD_JOBS.values()):
        workflow = load_workflow(workflow_name)
        assert str(workflow["env"][VCPU_CONSTANT]) == EXPECTED_VCPUS, (
            f"{workflow_name} must declare the Ubicloud shape once"
        )

    for job_name in UBICLOUD_JOBS:
        job = load_job(job_name)
        script = str(steps_by_name(job)["Bound concurrency to the runner shape"]["run"])
        for variable in DERIVED_CONCURRENCY_VARIABLES:
            assert f"{variable}=%s" in script, (
                f"{job_name} must derive {variable} from {VCPU_CONSTANT}"
            )
        assert script.count(f"${{{VCPU_CONSTANT}}}") == len(
            DERIVED_CONCURRENCY_VARIABLES
        ), f"{job_name} must derive both bounds from the one named constant"
        assert job.get("env", {}).get("NEXTEST_TEST_THREADS") is None, (
            f"{job_name} must not pin a second, undeclared thread count"
        )


def test_no_workflow_requests_unbounded_pytest_parallelism() -> None:
    """`-n auto` would spawn one worker per host CPU on a shared hypervisor."""
    for workflow_name in ("ci.yml", "coverage-main.yml"):
        workflow = load_workflow(workflow_name)
        for job in workflow["jobs"].values():
            assert "-n auto" not in run_scripts(job), (
                f"{workflow_name} must bound pytest workers explicitly"
            )


def test_compiler_cache_uses_exactly_one_selected_backend() -> None:
    """Two configured backends make the reported hit rate unattributable."""
    for workflow_name in set(UBICLOUD_JOBS.values()):
        workflow = load_workflow(workflow_name)
        assert workflow["env"]["SCCACHE_BACKEND"] == "gha", (
            f"{workflow_name} must declare the backend once, at workflow level"
        )
        assert workflow["env"]["RUSTC_WRAPPER"] == "sccache", (
            f"{workflow_name} must route every rustc invocation through sccache"
        )
        assert "SCCACHE_GHA_ENABLED" not in workflow["env"], (
            f"{workflow_name} must let the selector export the backend variables"
        )

    for job_name in UBICLOUD_JOBS:
        job = load_job(job_name)
        names = step_names(job)
        selector = steps_by_name(job)["Select the compiler cache backend"]
        assert "scripts/select-sccache-backend.sh" in str(selector["run"])
        credentials = steps_by_name(job)["Expose the Actions cache credentials to sccache"]
        assert credentials["if"] == "env.SCCACHE_BACKEND == 'gha'", (
            f"{job_name} must export the Actions cache credentials only for GHA"
        )
        assert names.index("Select the compiler cache backend") < names.index(
            "Setup Rust"
        ), f"{job_name} must choose a backend before any Cargo invocation"


def test_compiler_cache_effectiveness_is_always_recorded() -> None:
    """Zero compile requests is a broken integration, not a clean run."""
    for job_name in UBICLOUD_JOBS:
        job = load_job(job_name)
        names = step_names(job)
        assert names.index("Reset sccache statistics") < names.index(
            "Record sccache effectiveness"
        ), f"{job_name} must zero the counters before the build"
        record = steps_by_name(job)["Record sccache effectiveness"]
        assert record["if"] == "always()", (
            f"{job_name} must publish sccache statistics even when the build fails"
        )
        assert "scripts/record-sccache-effectiveness.sh" in str(record["run"])


def test_tools_cache_keys_name_every_pin_they_depend_on() -> None:
    """An explainable key lets a reviewer justify a miss without reading steps."""
    for job_name, pins in TOOL_PINS_IN_KEY.items():
        key = _tools_restore_key(job_name)
        for pin in pins:
            assert pin in key, f"{job_name} tools cache key must record {pin}"
        assert "hashFiles('rust-toolchain.toml')" in key, (
            f"{job_name} tools cache key must track the pinned toolchain"
        )


def test_dylint_host_tool_key_matches_the_makefile_pins() -> None:
    """The cached archive must be invalidated by the versions the build installs."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    assert "CARGO_DYLINT_VERSION ?= 6.0.1" in makefile
    assert "DYLINT_LINK_VERSION ?= 6.0.1" in makefile
    step = steps_by_name(load_job("linux-full"))["Restore the Dylint host tools"]
    key = str(step["with"]["key"])
    assert "cargo-dylint6.0.1" in key
    assert "dylint-link6.0.1" in key
    assert step["with"]["path"].strip() == "~/.cache/whitaker-dylint-tools", (
        "the Dylint host tools cache must own the durable Makefile directory"
    )


def test_clippy_mirror_is_owned_and_provisioned_after_its_restore() -> None:
    """The mirror removes an unowned multi-hundred-megabyte upstream clone."""
    for job_name in UBICLOUD_JOBS:
        job = load_job(job_name)
        names = step_names(job)
        mirror = steps_by_name(job)["Restore the Clippy source mirror"]
        assert "clippy-mirror-v1-dylint6.0.1" in str(mirror["with"]["key"])
        assert names.index("Restore the Clippy source mirror") < names.index(
            "Provision the Clippy source mirror"
        ), f"{job_name} must restore the mirror before provisioning it"


def test_nested_cargo_builds_share_one_absolute_target_directory() -> None:
    """Dylint's nested build must not fall back to an uninstrumented target."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    assert 'CARGO_LLVM_COV_TARGET_DIR="$(COVERAGE_TARGET_DIR)"' in makefile
    assert 'CARGO_TARGET_DIR="$(COVERAGE_TARGET_DIR)"' in makefile
    assert "COVERAGE_TARGET_DIR ?= $(CURDIR)/target/llvm-cov-target" in makefile


def test_no_restore_step_is_left_without_an_identifier() -> None:
    """An unnamed restore cannot be reported, saved, or explained."""
    for job_name in UBICLOUD_JOBS:
        for step in restore_steps(load_job(job_name)):
            assert step.get("id"), f"{job_name}: {step['name']!r} must declare an id"
