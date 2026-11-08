"""Keep the helper binaries behind a feature that only the helpers' callers pass.

``cargo install`` and ``cargo binstall`` build or expect every binary of a
package. The four release and maintenance helpers are not part of what users
install, so they declare ``required-features`` (issue 509), and every command
that builds or runs one must therefore enable that feature. A command that
forgets fails at the point of use with "target requires the features".

These tests parse the manifest and read the commands that name a helper; they
do not restate a list of files.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import tomllib
import typing as typ

from ubicloud_workflow_support import REPOSITORY_ROOT

FEATURE: typ.Final[str] = "release-tools"
MANIFEST = REPOSITORY_ROOT / "installer/Cargo.toml"
PRIMARY_BIN: typ.Final[str] = "whitaker-installer"
SEARCHED_SUFFIXES: typ.Final[tuple[str, ...]] = (".yml", ".yaml", ".sh")


def _bins() -> list[dict[str, typ.Any]]:
    """Return the manifest's ``[[bin]]`` tables."""
    return tomllib.loads(MANIFEST.read_text(encoding="utf-8"))["bin"]


def _helper_names() -> list[str]:
    """Return the binaries other than the primary installer."""
    return [b["name"] for b in _bins() if b["name"] != PRIMARY_BIN]


def test_only_the_installer_is_built_without_a_feature() -> None:
    """Every helper requires the feature; the installer requires nothing."""
    for entry in _bins():
        required = entry.get("required-features", [])
        if entry["name"] == PRIMARY_BIN:
            assert not required, "whitaker-installer must build without features"
        else:
            assert required == [FEATURE], f"{entry['name']} must require only {FEATURE!r}"


def test_the_feature_is_declared() -> None:
    """The feature exists, so ``required-features`` can be satisfied."""
    features = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))["features"]
    assert FEATURE in features


def _command_files() -> list[typ.Any]:
    """Return the files whose commands may build or run a helper."""
    roots = [REPOSITORY_ROOT / ".github", REPOSITORY_ROOT / "scripts", REPOSITORY_ROOT / "Makefile"]
    files = []
    for root in roots:
        if root.is_file():
            files.append(root)
        else:
            files.extend(p for p in root.rglob("*") if p.suffix in SEARCHED_SUFFIXES)
    return files


def test_every_command_naming_a_helper_enables_the_feature() -> None:
    """A ``--bin <helper>`` command passes ``--features`` with the feature."""
    pattern = re.compile(r"--bin\s+(" + "|".join(map(re.escape, _helper_names())) + r")\b")
    offenders = []
    for path in _command_files():
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if pattern.search(line) and FEATURE not in line:
                offenders.append(f"{path.relative_to(REPOSITORY_ROOT)}:{number}: {line.strip()}")
    assert not offenders, "commands build a helper without the feature:\n" + "\n".join(offenders)
