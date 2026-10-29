"""Both release workflows publish, require, and verify the Clippy snapshot.

Every installer embeds `installer/clippy-snapshot.toml` and downloads the
bundle it pins from the rolling release, so a release that omits the bundle
installs nothing (#526). These tests hold the publishing jobs to building the
bundle at most once, refusing to publish without it, and proving the published
assets work on a fresh runner with github.com denied.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from ubicloud_workflow_support import load_workflow

WORKFLOWS: typ.Final[tuple[str, ...]] = ("release.yml", "rolling-release.yml")


def _jobs(workflow: str) -> dict[str, typ.Any]:
    """Return the jobs mapping of one workflow."""
    jobs = load_workflow(workflow).get("jobs")
    assert isinstance(jobs, dict), f"{workflow} must declare jobs"
    return jobs


def _steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return a job's steps."""
    return job.get("steps") or []


def _step_names(job: dict[str, typ.Any]) -> list[str]:
    """Return a job's step names in order."""
    return [str(step.get("name", "")) for step in _steps(job)]


def _step(job: dict[str, typ.Any], name: str) -> dict[str, typ.Any]:
    """Return the step called `name`, failing when absent."""
    for step in _steps(job):
        if step.get("name") == name:
            return step
    pytest.fail(f"no step named {name!r}")


@pytest.mark.parametrize("workflow", WORKFLOWS)
def test_the_snapshot_pair_is_staged_by_the_pair_aware_script(workflow: str) -> None:
    """One script decides build, repair, or refusal for the bundle and sidecar."""
    job = _jobs(workflow)["clippy-snapshot"]
    prepare = _step(job, "Prepare the Clippy snapshot assets")
    assert prepare["id"] == "prepare"
    assert "scripts/prepare-clippy-snapshot-assets.sh" in str(prepare["run"])
    assert "${{" not in str(prepare["run"]), "expressions must reach the shell via env"
    upload = _step(job, "Upload the Clippy snapshot assets")
    assert upload["if"] == "steps.prepare.outputs.state != 'published'"
    assert upload["with"]["path"] == "dist/whitaker-clippy-snapshot-*"


@pytest.mark.parametrize("workflow", WORKFLOWS)
def test_publication_requires_the_pinned_snapshot(workflow: str) -> None:
    """Publishing waits for the snapshot job and refuses to go out without it."""
    publish = _jobs(workflow)["publish"]
    assert "clippy-snapshot" in publish["needs"]
    names = _step_names(publish)
    require = "Require the pinned Clippy snapshot"
    assert names.index("Download the Clippy snapshot assets") < names.index(require)
    run = str(_step(publish, require)["run"])
    assert "sha256sum --check --strict" in run
    assert "refusing to publish" in run
    assert "must not replace it" in run, "a rebuilt bundle never joins a published pair"


def test_the_rolling_release_lists_the_bundle_and_keeps_its_sidecar() -> None:
    """The asset list feeding the publish script includes bundles."""
    publish = _jobs("rolling-release.yml")["publish"]
    run = str(_step(publish, "Check whether release assets are available")["run"])
    assert "dist/*.bundle" in run
    assert "dist/*.sha256" in run


def test_the_versioned_release_uploads_the_bundle() -> None:
    """The versioned release attaches the bundle beside the archives."""
    publish = _jobs("release.yml")["publish"]
    assert "dist/*.bundle" in str(_step(publish, "Create GitHub Release")["run"])


@pytest.mark.parametrize("workflow", WORKFLOWS)
def test_published_assets_are_verified_on_a_fresh_runner(workflow: str) -> None:
    """After publication the published suite and snapshot lint offline."""
    job = _jobs(workflow)["verify-published-bootstrap"]
    assert job["needs"] == "publish"
    assert job["if"] == "${{ !cancelled() && needs.publish.result == 'success' }}"
    names = _step_names(job)
    order = [
        "Install the published suite and bootstrap",
        "Verify the published Clippy snapshot",
        "Verify the installed bootstrap without public Clippy",
    ]
    assert [names.index(name) for name in order] == sorted(names.index(name) for name in order)
    assert "--no-source-fallback" in str(_step(job, order[0])["run"])
    assert "scripts/verify-published-snapshot.sh" in str(_step(job, order[1])["run"])
    assert "scripts/verify-installed-bootstrap.sh" in str(_step(job, order[2])["run"])


def test_the_released_installer_lints_offline_on_every_platform() -> None:
    """`verify-binstall` runs the released installer on Linux, macOS, and Windows."""
    job = _jobs("release.yml")["verify-binstall"]
    assert job["strategy"]["matrix"]["os"] == ["ubuntu-latest", "macos-latest", "windows-latest"]
    names = _step_names(job)
    install = "Install the suite with the released installer"
    assert names.index("Run the installed installer") < names.index(install)
    assert "--no-source-fallback" in str(_step(job, install)["run"])
    unix = _step(job, "Verify the installed bootstrap without public Clippy")
    assert unix["if"] == "runner.os != 'Windows'"
    windows = _step(job, "Verify the installed bootstrap through the PowerShell wrapper")
    assert windows["if"] == "runner.os == 'Windows'"
    assert "verify-installed-bootstrap.ps1" in str(windows["run"])


def test_the_versioned_snapshot_job_uses_the_tag_and_no_template_shell() -> None:
    """The tag's manifest decides the snapshot, and its name never becomes shell."""
    job = _jobs("release.yml")["clippy-snapshot"]
    assert _step(job, "Checkout")["with"]["ref"] == "${{ env.RELEASE_TAG }}"
    prepare = _step(job, "Prepare the Clippy snapshot assets")
    assert prepare["env"]["SNAPSHOT_RELEASE"] == "${{ env.RELEASE_TAG }}"


def test_a_versioned_release_requires_the_snapshot_on_rolling() -> None:
    """Installers download from `rolling`, so a tag cannot publish without it."""
    publish = _jobs("release.yml")["publish"]
    names = _step_names(publish)
    gate = "Require the pinned Clippy snapshot on the rolling release"
    assert names.index(gate) < names.index("Create GitHub Release")
    assert "gh release view rolling" in str(_step(publish, gate)["run"])


@pytest.mark.parametrize(
    ("workflow", "job"),
    [
        ("release.yml", "verify-binstall"),
        ("release.yml", "verify-published-bootstrap"),
        ("rolling-release.yml", "verify-published-bootstrap"),
    ],
)
def test_the_wrapper_directory_reaches_path_natively(workflow: str, job: str) -> None:
    """Windows needs a native path in GITHUB_PATH for PowerShell to resolve it."""
    run = str(_step(_jobs(workflow)[job], "Put the wrapper directory on PATH")["run"])
    assert 'cygpath -w "$HOME/.local/bin"' in run
