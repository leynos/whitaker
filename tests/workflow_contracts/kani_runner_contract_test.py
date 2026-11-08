"""Test the Kani resource wrapper and harness-selection boundary."""

from __future__ import annotations

import os
import subprocess
import typing as typ
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
RUN_KANI = REPOSITORY_ROOT / "scripts" / "run-kani.sh"
RUN_KANI_LIMITED = REPOSITORY_ROOT / "scripts" / "run-kani-limited.sh"


class _PreflightCase(typ.NamedTuple):
    """Describe one limited-runner preflight rejection scenario."""

    runner_arguments: tuple[str, ...]
    environment: dict[str, str]
    missing_tools: frozenset[str]
    platform: str
    grep_status: int | None
    error_fragment: str


def _write_executable(path: Path, contents: str) -> None:
    """Write an executable shell stub."""
    path.write_text(contents, encoding="utf-8")
    path.chmod(0o755)


def _write_limited_stubs(
    directory: Path,
    *,
    missing_tools: frozenset[str] = frozenset(),
    platform: str = "Linux",
    grep_status: int | None = None,
) -> None:
    """Create controlled command stubs for the limited runner."""
    directory.mkdir(parents=True, exist_ok=True)
    _write_executable(directory / "uname", f"#!/bin/sh\nprintf '%s\\n' '{platform}'\n")
    if "systemd-run" not in missing_tools:
        _write_executable(
            directory / "systemd-run",
            """#!/bin/bash
if [[ ${1:-} == --version ]]; then
    printf '%s\\n' "${FAKE_SYSTEMD_VERSION:-systemd 257}"
    exit 0
fi
printf '%s\\n' "$*" >> "$SYSTEMD_RUN_LOG"
while [[ $# -gt 0 && $1 != -- ]]; do shift; done
[[ $# -gt 0 ]] || exit 2
shift
exec "$@"
""",
        )
    if "systemctl" not in missing_tools:
        _write_executable(
            directory / "systemctl",
            """#!/bin/bash
printf 'Result=%s\\n' "${FAKE_SCOPE_RESULT:-success}"
printf 'MemoryPeak=%s\\n' "${FAKE_SCOPE_PEAK:-}"
""",
        )
    if "timeout" not in missing_tools:
        _write_executable(directory / "timeout", "#!/bin/sh\nexec /usr/bin/timeout \"$@\"\n")
    _write_executable(
        directory / "cat",
        """#!/bin/bash
case ${1:-} in
    */memory.peak)
        if [[ ${FAKE_PEAK_MODE:-value} == unavailable ]]; then exit 1; fi
        printf '%s\\n' "${FAKE_PEAK_BYTES:-1048576}"
        ;;
    *) exec /usr/bin/cat "$@" ;;
esac
""",
    )
    if grep_status is not None:
        _write_executable(directory / "grep", f"#!/bin/sh\nexit {grep_status}\n")
        _write_executable(directory / "sed", "#!/bin/sh\nprintf '257\\n'\n")


def _limited_environment(
    tmp_path: Path,
    bin_dir: Path,
    overrides: dict[str, str] | None = None,
    *,
    include_host_path: bool = True,
) -> dict[str, str]:
    """Build an isolated environment for the limited runner."""
    environment = os.environ.copy()
    for name in (
        "WHITAKER_KANI_MEMORY_MAX_BYTES",
        "WHITAKER_KANI_TIMEOUT_SECONDS",
        "FAKE_SCOPE_RESULT",
        "FAKE_SCOPE_PEAK",
        "FAKE_PEAK_MODE",
        "FAKE_SYSTEMD_VERSION",
    ):
        environment.pop(name, None)
    system_path = environment.get("PATH", "/usr/bin:/bin")
    environment["PATH"] = (
        f"{bin_dir}{os.pathsep}{system_path}" if include_host_path else str(bin_dir)
    )
    environment["TMPDIR"] = str(tmp_path)
    environment["SYSTEMD_RUN_LOG"] = str(tmp_path / "systemd-run.log")
    if overrides:
        environment.update(overrides)
    return environment


def _run_limited(
    tmp_path: Path,
    command: tuple[str, ...] = ("/bin/bash", "-c", "true"),
    overrides: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the actual limited-runner script with controlled systemd commands."""
    bin_dir = tmp_path / "bin"
    _write_limited_stubs(bin_dir)
    environment = _limited_environment(tmp_path, bin_dir, overrides)
    return subprocess.run(
        ["/bin/bash", str(RUN_KANI_LIMITED), "test_harness", *command],
        cwd=REPOSITORY_ROOT,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
        timeout=20,
    )


def _has_readable_process_cgroup_peak() -> bool:
    """Report whether the current Linux cgroup exposes its peak file."""
    cgroup_file = Path("/proc/self/cgroup")
    controller_file = Path("/sys/fs/cgroup/cgroup.controllers")
    if not cgroup_file.is_file() or not controller_file.is_file():
        return False
    cgroup_path = next(
        (
            entry.split("::", maxsplit=1)[1]
            for entry in cgroup_file.read_text(encoding="utf-8").splitlines()
            if entry.startswith("0::")
        ),
        None,
    )
    if cgroup_path is None:
        return False
    peak_file = Path("/sys/fs/cgroup", cgroup_path.lstrip("/"), "memory.peak")
    return peak_file.is_file() and os.access(peak_file, os.R_OK)


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {"WHITAKER_KANI_MEMORY_MAX_BYTES": "0"},
                frozenset(),
                "Linux",
                None,
                "WHITAKER_KANI_MEMORY_MAX_BYTES",
            ),
            id="memory-limit-zero",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {"WHITAKER_KANI_MEMORY_MAX_BYTES": "8589934593"},
                frozenset(),
                "Linux",
                None,
                "WHITAKER_KANI_MEMORY_MAX_BYTES",
            ),
            id="memory-limit-above-cap",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {"WHITAKER_KANI_TIMEOUT_SECONDS": "not-a-number"},
                frozenset(),
                "Linux",
                None,
                "WHITAKER_KANI_TIMEOUT_SECONDS",
            ),
            id="timeout-not-numeric",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {"WHITAKER_KANI_TIMEOUT_SECONDS": "601"},
                frozenset(),
                "Linux",
                None,
                "WHITAKER_KANI_TIMEOUT_SECONDS",
            ),
            id="timeout-above-cap",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {},
                frozenset(),
                "Darwin",
                None,
                "require Linux",
            ),
            id="unsupported-platform",
        ),
        pytest.param(
            _PreflightCase(
                (),
                {},
                frozenset(),
                "Linux",
                None,
                "usage: scripts/run-kani-limited.sh",
            ),
            id="missing-harness",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness",),
                {},
                frozenset(),
                "Linux",
                None,
                "usage: scripts/run-kani-limited.sh",
            ),
            id="missing-command",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {},
                frozenset({"systemd-run"}),
                "Linux",
                None,
                "requires systemd-run",
            ),
            id="missing-systemd-run",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {},
                frozenset({"systemctl"}),
                "Linux",
                None,
                "requires systemctl",
            ),
            id="missing-systemctl",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {},
                frozenset({"timeout"}),
                "Linux",
                None,
                "requires timeout",
            ),
            id="missing-timeout",
        ),
        pytest.param(
            _PreflightCase(
                ("test_harness", "/bin/true"),
                {},
                frozenset(),
                "Linux",
                1,
                "cgroup v2 memory controller",
            ),
            id="missing-cgroup-memory-controller",
        ),
    ],
)
def test_limited_runner_rejects_preflight_failures(
    tmp_path: Path, case: _PreflightCase
) -> None:
    """Reject invalid wrapper settings and unavailable host prerequisites."""
    bin_dir = tmp_path / "bin"
    _write_limited_stubs(
        bin_dir,
        missing_tools=case.missing_tools,
        platform=case.platform,
        grep_status=case.grep_status,
    )
    result = subprocess.run(
        ["/bin/bash", str(RUN_KANI_LIMITED), *case.runner_arguments],
        cwd=REPOSITORY_ROOT,
        env=_limited_environment(
            tmp_path,
            bin_dir,
            case.environment,
            include_host_path=False,
        ),
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 2, result.stderr
    assert case.error_fragment in result.stderr, result.stderr


@pytest.mark.parametrize(("version", "option_expected"), [("systemd 257", False), ("systemd 258", True)])
def test_limited_runner_controls_systemd_environment_expansion(
    tmp_path: Path, version: str, option_expected: bool
) -> None:
    """Disable command expansion only on systemd versions that support it."""
    if not _has_readable_process_cgroup_peak():
        pytest.skip("the test host does not expose readable cgroup v2 peak memory")
    result = _run_limited(tmp_path, overrides={"FAKE_SYSTEMD_VERSION": version})
    invocation = (tmp_path / "systemd-run.log").read_text(encoding="utf-8")

    assert result.returncode == 0, result.stderr
    assert ("--expand-environment=no" in invocation) is option_expected, (
        f"systemd version {version!r}, invocation: {invocation!r}"
    )


def test_limited_runner_propagates_timeout_status(tmp_path: Path) -> None:
    """Return GNU timeout's status when a proof exceeds its time budget."""
    if not _has_readable_process_cgroup_peak():
        pytest.skip("the test host does not expose readable cgroup v2 peak memory")
    result = _run_limited(
        tmp_path,
        command=("/bin/bash", "-c", "sleep 3"),
        overrides={"WHITAKER_KANI_TIMEOUT_SECONDS": "1"},
    )

    assert result.returncode == 124, result.stderr
    assert "Kani timed out for test_harness" in result.stderr, result.stderr


def test_limited_runner_propagates_command_failure(tmp_path: Path) -> None:
    """Return a proof command's non-zero status without recording a pass."""
    if not _has_readable_process_cgroup_peak():
        pytest.skip("the test host does not expose readable cgroup v2 peak memory")
    result = _run_limited(tmp_path, command=("/bin/bash", "-c", "exit 23"))

    assert result.returncode == 23, result.stderr
    assert "exit_status=23" in result.stdout, (
        f"stdout: {result.stdout}\nstderr: {result.stderr}"
    )


def test_limited_runner_fails_closed_after_an_oom_kill(tmp_path: Path) -> None:
    """Report an OOM-killed scope as an inconclusive proof."""
    if not _has_readable_process_cgroup_peak():
        pytest.skip("the test host does not expose readable cgroup v2 peak memory")
    result = _run_limited(tmp_path, overrides={"FAKE_SCOPE_RESULT": "oom-kill"})

    assert result.returncode == 1, result.stderr
    assert "process-tree memory cap" in result.stderr, result.stderr


def test_limited_runner_fails_closed_without_peak_memory(tmp_path: Path) -> None:
    """Report missing peak-memory data as an inconclusive proof."""
    if not _has_readable_process_cgroup_peak():
        pytest.skip("the test host does not expose readable cgroup v2 peak memory")
    result = _run_limited(tmp_path, overrides={"FAKE_PEAK_MODE": "unavailable"})

    assert result.returncode == 1, result.stderr
    assert "could not report peak memory" in result.stderr, result.stderr
