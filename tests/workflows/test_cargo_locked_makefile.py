"""Verify that Makefile Cargo invocations consistently honour CARGO_LOCKED."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]


def _write_stub(directory: Path, name: str, body: str) -> Path:
    """Write an executable shell stub and return its path."""
    path = directory / name
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(0o755)
    return path


def _write_cargo_stub(directory: Path) -> Path:
    """Write a Cargo stand-in that records metadata and build invocations."""
    return _write_stub(
        directory,
        "cargo",
        '''echo "$@" >> "$CARGO_LOCKED_LOG"
case "$1" in
metadata) echo '{{"packages":[{{"name":"whitaker-installer","version":"0.2.5"}}]}}' ;;
build)
    target=""
    for argument in "$@"; do
        if [ "$previous" = "--target" ]; then target="$argument"; fi
        previous="$argument"
    done
    mkdir -p "target/$target/release"
    : > "target/$target/release/whitaker-installer"
    cat > "target/$target/release/whitaker-package-installer" <<'EOF'
#!/bin/sh
for argument in "$@"; do
    if [ "$previous" = "--output-dir" ]; then output_dir="$argument"; fi
    previous="$argument"
done
touch "$output_dir/whitaker-installer.tgz"
EOF
    chmod 755 "target/$target/release/whitaker-package-installer"
    ;;
esac''',
    )


def _run_make(target: str, cargo: Path, locked: str, stub_dir: Path) -> list[str]:
    """Run one target with stubbed tools and return recorded Cargo arguments."""
    log = stub_dir / f"{target}-{locked or 'unlocked'}.log"
    environment = os.environ | {
        "CARGO_LOCKED_LOG": str(log),
        "PATH": f"{stub_dir}:/usr/bin:/bin",
    }
    result = subprocess.run(
        ["make", target, f"CARGO={cargo}", f"CARGO_LOCKED={locked}"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    assert result.returncode == 0, result.stderr
    return log.read_text(encoding="utf-8").splitlines()


@pytest.mark.parametrize("locked", ["", "--locked"])
def test_representative_targets_forward_cargo_locked(tmp_path: Path, locked: str) -> None:
    """Ordinary Cargo targets include the requested lock mode and no other one."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    cargo = _write_cargo_stub(stub_dir)

    recorded = [
        invocation
        for target in ("typecheck", "lint")
        for invocation in _run_make(target, cargo, locked, stub_dir)
    ]

    assert recorded
    assert all(("--locked" in invocation) == bool(locked) for invocation in recorded)


@pytest.mark.parametrize("locked", ["", "--locked"])
def test_release_dry_run_forwards_cargo_locked_to_metadata_and_builds(
    tmp_path: Path, locked: str
) -> None:
    """Installer metadata and both builds share the caller's lock mode."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    _write_stub(stub_dir, "rustc", 'echo "host: fake-host"')
    _write_stub(stub_dir, "jq", "echo 0.2.5")
    cargo = _write_cargo_stub(stub_dir)

    recorded = _run_make("release-installer-dry-run", cargo, locked, stub_dir)

    assert len(recorded) == 3
    assert recorded[0].startswith("metadata ")
    assert all(("--locked" in invocation) == bool(locked) for invocation in recorded)
