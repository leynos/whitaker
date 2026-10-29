"""Run the post-release verification scripts against PATH stubs.

`scripts/verify-published-snapshot.sh` and `scripts/verify-installed-bootstrap.sh`
only run after a release publishes, so a broken check would otherwise go
unnoticed until then. This harness executes the real scripts in a scratch
sandbox whose `PATH` starts with stub `cargo`, `curl`, `git`, `sudo`,
`whitaker`, and `whitaker-installer` executables. Real coreutils stay
available, so the scripts' own checks are what the tests exercise.

The stubs never touch the network, `/etc/hosts`, or the caller's home
directory: `sudo tee -a /etc/hosts` is redirected into a scratch hosts file,
and the `git ls-remote` canary succeeds only while that file lacks a
`github.com` entry. Every stub appends its invocation to `calls.log`, and
`FAKE_*` environment toggles make a stub violate one property at a time.
"""

from __future__ import annotations

import dataclasses as dc
import os
import subprocess
import typing as typ
from pathlib import Path

REPOSITORY_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
SCRIPTS: typ.Final[Path] = REPOSITORY_ROOT / "scripts"
COMMIT: typ.Final[str] = "0123456789abcdef0123456789abcdef01234567"
ASSET: typ.Final[str] = f"whitaker-clippy-snapshot-{COMMIT}.bundle"

_PREAMBLE: typ.Final[str] = """#!/usr/bin/env bash
set -euo pipefail
echo "$(basename "$0") $*" >>"${FAKE_CALLS}"
"""

_STUBS: typ.Final[dict[str, str]] = {
    # Redirects the one privileged write the scripts perform.
    "sudo": """
args=()
for arg in "$@"; do
    [[ "${arg}" == /etc/hosts ]] && arg=${FAKE_HOSTS}
    args+=("${arg}")
done
exec "${args[@]}"
""",
    # The canary reaches "GitHub" only while the hosts file lacks a denial.
    "git": """
[[ "$1" == ls-remote ]] || exit 0
[[ "${FAKE_DENIAL_INEFFECTIVE:-}" == 1 ]] && exit 0
grep -qs 'github.com' "${FAKE_HOSTS}" && exit 128
exit 0
""",
    # Serves files from the fake release directory by basename.
    "curl": """
output=''
while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --output) output=$2; shift 2 ;;
        -*) shift ;;
        *) url=$1; shift ;;
    esac
done
source="${FAKE_RELEASE}/$(basename "${url}")"
[[ -f "${source}" ]] || { echo "404: ${url}" >&2; exit 22; }
cp -- "${source}" "${output}"
""",
}

_CARGO: typ.Final[str] = """
commit=$(awk -F '"' '/^commit = / {print $2}' installer/clippy-snapshot.toml)
while [[ "$1" != -- ]]; do shift; done
shift
verb=$1; shift
root=''; bundle=''; repo=''
while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --root) root=$2; shift 2 ;;
        --bundle) bundle=$2; shift 2 ;;
        --repo) repo=$2; shift 2 ;;
        *) shift ;;
    esac
done
asset="whitaker-clippy-snapshot-${commit}.bundle"
case "${verb}" in
    verify) [[ -d "${repo}" ]] || { echo "no generation at ${repo}" >&2; exit 1; } ;;
    install)
        if [[ -z "${bundle}" ]]; then
            bundle="${FAKE_RELEASE}/${asset}"
        fi
        if [[ "${FAKE_ACCEPT_TAMPERED:-}" != 1 ]]; then
            expected=$(cut -d ' ' -f 1 "${bundle}.sha256")
            actual=$(sha256sum "${bundle}" | cut -d ' ' -f 1)
            if [[ "${expected}" != "${actual}" ]]; then
                [[ "${FAKE_LEAK_GENERATION:-}" == 1 ]] && mkdir -p "${root}/${commit}"
                if [[ "${FAKE_WRONG_REJECTION:-}" == 1 ]]; then
                    echo "error: could not read the bundle" >&2
                else
                    echo "error: bundle failed SHA-256 verification" >&2
                fi
                exit 1
            fi
        fi
        mkdir -p "${root}/${commit}"
        echo "${root}/${commit}"
        ;;
esac
"""

_WHITAKER_INSTALLER: typ.Final[str] = """
echo "Clippy snapshot: ${FAKE_DATA_ROOT}/whitaker/snapshot"
echo "Dylint driver directory: ${FAKE_DATA_ROOT}/whitaker/driver"
"""

# Emulates `whitaker --all` against the std::fs fixture: the data root comes
# from XDG_DATA_HOME exactly as the installed tool resolves it.
_WHITAKER: typ.Final[str] = """
if [[ "${FAKE_IGNORE_XDG:-}" == 1 ]]; then
    root=${HOME}/.local/share
else
    root=${XDG_DATA_HOME:-${HOME}/.local/share}
fi
snapshot=${root}/whitaker/snapshot
driver_dir=${root}/whitaker/driver
if [[ ! -d "${snapshot}" ]]; then
    [[ "${FAKE_SUCCEED_WITHOUT_SNAPSHOT:-}" == 1 ]] && exit 0
    if [[ "${FAKE_SILENT_WITHOUT_SNAPSHOT:-}" == 1 ]]; then
        echo "error: snapshot missing" >&2
    else
        echo "error: snapshot missing; Whitaker never falls back to cloning public Clippy" >&2
    fi
    [[ "${FAKE_LINT_WITHOUT_SNAPSHOT:-}" == 1 ]] && echo "warning: no_std_fs_operations"
    exit 1
fi
driver="${driver_dir}/toolchain/dylint-driver"
build_driver() {
    mkdir -p "${driver_dir}/toolchain"
    echo "driver-build" >>"${FAKE_CALLS}"
    sleep 0.5
    if [[ "${FAKE_REBUILD_ALWAYS:-}" == 1 ]]; then date +%s%N >"${driver}"; else echo v1 >"${driver}"; fi
}
# FAKE_CONCURRENT_FAILS breaks the second cold build, which only the
# concurrent phase performs, so the failure does not depend on both runs
# landing inside one build window.
if [[ ! -f "${driver}" && "${FAKE_CONCURRENT_FAILS:-}" == 1 ]] &&
    grep -qx driver-build "${FAKE_CALLS}" 2>/dev/null; then
    echo "error: the driver is being built by another process" >&2
    exit 1
fi
if [[ ! -f "${driver}" || "${FAKE_REBUILD_ALWAYS:-}" == 1 ]]; then
    if mkdir "${driver_dir}.lock" 2>/dev/null; then
        build_driver
        rmdir "${driver_dir}.lock"
    else
        while [[ -d "${driver_dir}.lock" ]]; do sleep 0.1; done
    fi
fi
[[ "${FAKE_SUCCEED:-}" == 1 ]] && exit 0
if [[ "${FAKE_NO_DIAGNOSTIC:-}" == 1 ]]; then
    echo "error: crash" >&2
else
    echo "error: no_std_fs_operations" >&2
fi
exit 1
"""

_BODIES: typ.Final[dict[str, str]] = {
    "cargo": _CARGO,
    "whitaker": _WHITAKER,
    "whitaker-installer": _WHITAKER_INSTALLER,
}


@dc.dataclass(frozen=True)
class Sandbox:
    """A scratch repository, stub `PATH`, and fake home for one script run.

    Attributes
    ----------
    root
        The scratch directory holding every other path.
    repository
        The working directory the scripts run from; it carries the manifest.
    work
        The `WORK_DIR` argument handed to the script.
    release
        The directory `curl` and `cargo` serve release assets from.
    hosts
        The scratch hosts file standing in for `/etc/hosts`.
    calls
        The log every stub appends its invocation to.
    data_root
        The fake `XDG_DATA_HOME` holding the installed snapshot and driver.
    """

    root: Path
    repository: Path
    work: Path
    release: Path
    hosts: Path
    calls: Path
    data_root: Path

    def call_lines(self) -> list[str]:
        """Return the recorded stub invocations in order."""
        if not self.calls.exists():
            return []
        return self.calls.read_text(encoding="utf-8").splitlines()


def make_sandbox(root: Path) -> Sandbox:
    """Create a sandbox with stubs installed and a valid published release.

    Parameters
    ----------
    root
        An empty scratch directory, such as pytest's `tmp_path`.

    Returns
    -------
    Sandbox
        The populated sandbox.
    """
    stubs = root / "stubs"
    stubs.mkdir()
    for name in ("sudo", "git", "curl", "cargo", "whitaker", "whitaker-installer"):
        body = _BODIES.get(name, _STUBS.get(name, ""))
        path = stubs / name
        path.write_text(_PREAMBLE + body, encoding="utf-8")
        path.chmod(0o755)
    repository = root / "repository"
    (repository / "installer").mkdir(parents=True)
    (repository / "installer" / "clippy-snapshot.toml").write_text(
        f'commit = "{COMMIT}"\n', encoding="utf-8"
    )
    release = root / "release"
    release.mkdir()
    bundle = release / ASSET
    bundle.write_bytes(bytes(range(256)) * 32)
    digest = subprocess.run(
        ["sha256sum", str(bundle)], capture_output=True, text=True, check=True
    ).stdout.split()[0]
    (release / f"{ASSET}.sha256").write_text(f"{digest}  {ASSET}\n", encoding="utf-8")
    hosts = root / "hosts"
    hosts.write_text("127.0.0.1 localhost\n", encoding="utf-8")
    data_root = root / "home" / ".local" / "share"
    (data_root / "whitaker" / "snapshot").mkdir(parents=True)
    return Sandbox(
        root=root,
        repository=repository,
        work=root / "work",
        release=release,
        hosts=hosts,
        calls=root / "calls.log",
        data_root=data_root,
    )


def run_script(
    sandbox: Sandbox, script: str, **toggles: str
) -> subprocess.CompletedProcess[str]:
    """Run `scripts/<script>` in the sandbox with the given `FAKE_*` toggles.

    Parameters
    ----------
    sandbox
        The sandbox to run in.
    script
        The script file name under `scripts/`.
    **toggles
        Environment variables such as `FAKE_ACCEPT_TAMPERED="1"`.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished process with captured output.
    """
    env = {
        key: value
        for key, value in os.environ.items()
        if key != "BASH_ENV" and not key.startswith(("FAKE_", "XDG_", "GIT_"))
    }
    env.update(
        PATH=f"{sandbox.root / 'stubs'}{os.pathsep}{env['PATH']}",
        HOME=str(sandbox.root / "home"),
        FAKE_CALLS=str(sandbox.calls),
        FAKE_HOSTS=str(sandbox.hosts),
        FAKE_RELEASE=str(sandbox.release),
        FAKE_DATA_ROOT=str(sandbox.data_root),
        **toggles,
    )
    return subprocess.run(  # noqa: S603 - the script path is repository-owned
        ["bash", str(SCRIPTS / script), str(sandbox.work)],
        cwd=sandbox.repository,
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
