"""Contract tests for the release-pipeline healing changes (issue #288).

These checks lock in the behaviours that restore dependency-binary
publication after the failures documented in issue #288:

1. Every build leg runs on a native runner — no ``cross: true`` legs remain
   in either workflow, so ``openssl-sys`` cross builds and cross-architecture
   ``rustc-dev`` installs cannot recur.
2. Dependency tools install under an explicit stable toolchain
   (``cargo +stable install``), so the repository's pinned nightly can never
   reimpose a rustc floor on host-tool builds.
3. The rolling gate self-heals: when the manifest is unchanged it probes the
   rolling release for every expected dependency archive and rebuilds on any
   absence.
4. The tagged release's ``publish`` job runs after partial build failures
   (``!cancelled()``) and tolerates absent per-leg artefacts, so successful
   legs are never discarded.

Examples
--------
Run these checks:
`python3 -m pytest tests/workflows/test_release_workflow_healing.py`
"""

from __future__ import annotations

from pathlib import Path

from tests.workflows.rolling_release_workflow_test_support import (
    _find_step_by_name,
    _get_job_dict,
    _load_workflow_mapping,
)
from tests.workflows.workflow_test_helpers import REPO_ROOT, WORKFLOW_PATH

RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github/workflows/release.yml"

DEPENDENCY_TARGETS = {
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
}

NATIVE_RUNNERS = {
    "x86_64-unknown-linux-gnu": "ubuntu-22.04",
    "aarch64-unknown-linux-gnu": "ubuntu-24.04-arm",
    "x86_64-apple-darwin": "macos-15-intel",
}


def _job_from(workflow_path, job_name):
    """Return a job mapping from a workflow file."""
    workflow = _load_workflow_mapping(workflow_path.read_text(encoding="utf-8"))
    jobs = _get_job_dict(workflow, "jobs")
    return _get_job_dict(jobs, job_name)


def _matrix_entries(workflow_path, job_name):
    """Return the include-matrix entries for a job."""
    job = _job_from(workflow_path, job_name)
    return job["strategy"]["matrix"]["include"]


def _assert_native_runners(workflow_path, job_name):
    entries = _matrix_entries(workflow_path, job_name)
    targets = {entry["target"] for entry in entries}
    assert targets == DEPENDENCY_TARGETS, (
        f"{workflow_path.name}:{job_name} must cover the published target set"
    )
    for entry in entries:
        assert "cross" not in entry, (
            f"{workflow_path.name}:{job_name} must not cross-compile "
            f"{entry['target']}; cross builds broke openssl-sys and "
            "rustc-dev installs (issue #288)"
        )
        expected = NATIVE_RUNNERS.get(entry["target"])
        if expected is not None:
            assert entry["os"] == expected, (
                f"{workflow_path.name}:{job_name} must build "
                f"{entry['target']} natively on {expected}"
            )


def test_all_build_legs_run_on_native_runners() -> None:
    """No workflow leg cross-compiles; former cross legs use native runners."""
    for job_name in ("build-installer", "build-dependency-binaries"):
        _assert_native_runners(RELEASE_WORKFLOW_PATH, job_name)
    for job_name in ("build-lints", "build-dependency-binaries"):
        _assert_native_runners(WORKFLOW_PATH, job_name)


def _dependency_install_script(workflow_path):
    job = _job_from(workflow_path, "build-dependency-binaries")
    step = _find_step_by_name(job["steps"], "Build and package dependency binaries")
    assert step is not None, (
        f"{workflow_path.name} is missing the dependency install step"
    )
    return step["run"]


def test_dependency_installs_use_stable_toolchain() -> None:
    """Host tools install under stable so the nightly pin imposes no floor."""
    for workflow_path in (RELEASE_WORKFLOW_PATH, WORKFLOW_PATH):
        script = _dependency_install_script(workflow_path)
        assert "cargo +stable install" in script, (
            f"{workflow_path.name} must install dependency tools with "
            "`cargo +stable install`; the pinned nightly can be older than "
            "the tools' locked rustc floor (issue #288)"
        )
        job = _job_from(workflow_path, "build-dependency-binaries")
        toolchain_step = _find_step_by_name(
            job["steps"], "Install stable toolchain for host tools"
        )
        assert toolchain_step is not None, (
            f"{workflow_path.name} must install the stable toolchain before "
            "dependency installs"
        )
        assert "rustup toolchain install stable" in toolchain_step["run"]


def test_rolling_gate_probes_release_assets() -> None:
    """The gate rebuilds when expected rolling assets are missing."""
    job = _job_from(WORKFLOW_PATH, "dependency-manifest-changes")
    step = _find_step_by_name(job["steps"], "Check whether dependency manifest changed")
    assert step is not None
    script = step["run"]
    assert "scripts/check_dependency_binary_assets.py" in script, (
        "the gate must probe the rolling release via the asset-check script "
        "so it can self-heal after a failed publish (issue #288)"
    )
    env = step.get("env", {})
    assert "GH_TOKEN" in env, "the asset probe needs GH_TOKEN to query gh"
    probe_script = REPO_ROOT / "scripts/check_dependency_binary_assets.py"
    assert probe_script.exists(), "the asset-check script must exist"


def test_release_publish_tolerates_partial_build_failures() -> None:
    """publish runs after failed legs and downloads tolerate absences."""
    job = _job_from(RELEASE_WORKFLOW_PATH, "publish")
    condition = str(job.get("if", ""))
    assert "!cancelled()" in condition, (
        "publish must run even when a build leg fails, so successful legs "
        "are not discarded (issue #288)"
    )
    for step_name in ("Download all artefacts", "Download dependency artefacts"):
        step = _find_step_by_name(job["steps"], step_name)
        assert step is not None, f"publish is missing the step '{step_name}'"
        assert step.get("continue-on-error") is True, (
            f"'{step_name}' must tolerate absent artefacts from failed legs"
        )


def _step_names(workflow_path: Path, job_name: str) -> list[str]:
    """Return the ordered names for a workflow job's steps."""
    job = _job_from(workflow_path, job_name)
    return [step["name"] for step in job["steps"]]


def test_linux_release_legs_check_glibc_before_upload() -> None:
    """Each Linux artefact lane checks the conservative glibc baseline first."""
    expected_jobs = {
        RELEASE_WORKFLOW_PATH: (
            "build-installer",
            "build-dependency-binaries",
        ),
        WORKFLOW_PATH: (
            "build-lints",
            "build-dependency-binaries",
        ),
    }
    upload_names = {
        "build-installer": "Upload artefact",
        "build-lints": "Upload artefact",
        "build-dependency-binaries": "Upload dependency artefact",
    }
    for workflow_path, job_names in expected_jobs.items():
        for job_name in job_names:
            names = _step_names(workflow_path, job_name)
            assert names.index("Check glibc baseline") < names.index(
                upload_names[job_name]
            ), f"{workflow_path.name}:{job_name} must check glibc before upload"
            check = _find_step_by_name(
                _job_from(workflow_path, job_name)["steps"],
                "Check glibc baseline",
            )
            assert check is not None
            check_script = check.get("run")
            assert isinstance(check_script, str)
            assert "check_glibc_baseline.py" in check_script
            assert "GLIBC_2.35" in check_script


def test_rolling_dependency_glibc_check_reads_the_manifest() -> None:
    """The rolling check covers every dependency binary selected for packaging."""
    job = _job_from(WORKFLOW_PATH, "build-dependency-binaries")
    check = _find_step_by_name(job["steps"], "Check glibc baseline")
    assert check is not None
    script = check.get("run")
    assert isinstance(script, str)
    assert "dependency_binaries_manifest.py --output /tmp/dependency-binaries.tsv" in script
    assert "binaries=()" in script
    assert "while IFS=$'\\t' read -r _package binary _version; do" in script
    assert 'binaries+=("dependency-root/bin/${binary}")' in script
    assert "done < /tmp/dependency-binaries.tsv" in script
    assert '"${binaries[@]}"' in script


def test_tagged_release_requires_packaged_compatibility_verification() -> None:
    """The tagged publish job waits for successful Ubuntu compatibility checks."""
    compatibility = _job_from(RELEASE_WORKFLOW_PATH, "release-compatibility")
    assert compatibility["runs-on"] == "ubuntu-22.04"
    assert compatibility["needs"] == ["build-installer", "build-dependency-binaries"]
    compatibility_condition = str(compatibility["if"])
    assert "!cancelled()" in compatibility_condition
    assert "needs.build-installer.result" not in compatibility_condition
    assert "needs.build-dependency-binaries.result" not in compatibility_condition
    names = _step_names(RELEASE_WORKFLOW_PATH, "release-compatibility")
    assert names.index("Verify x86 packaged compatibility") < len(names)
    verification = _find_step_by_name(
        compatibility["steps"],
        "Verify x86 packaged compatibility",
    )
    assert verification is not None
    script = verification.get("run")
    assert isinstance(script, str)
    assert "whitaker-installer" in script
    assert "cargo_dylint" in script
    assert "dylint_link" in script
    assert 'cargo_dylint" dylint --version' in script
    assert '"$dylint_link" --version' in script

    publish = _job_from(RELEASE_WORKFLOW_PATH, "publish")
    assert "release-compatibility" in publish["needs"]
    assert "needs.release-compatibility.result == 'success'" in str(publish["if"])
