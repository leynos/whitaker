"""Contract tests for the shipped Agent Skills manifests.

Every directory under `skills/` carries a `SKILL.md` whose YAML frontmatter is
an Agent Skills manifest. `make lint` depends on `skill-manifest-check`, so a
malformed manifest fails the standard gate rather than shipping to a consumer
that installs the tree.

The manifest `name` is the discovery name a strict loader uses, so a manifest
that omits it — or sets it to the empty string — is not discoverable.
`skills-ref` coerces every `metadata` value with `str(v)` rather than rejecting
other shapes, so a YAML sequence survives validation but reaches consumers as a
Python repr; that trap is pinned here because the schema check cannot see it.

The gate itself is pinned too, because both of its silent failure modes have
happened upstream: a shell `for` loop exits with the status of its final
iteration, so a conformant trailing skill masks a malformed earlier one, and a
manifest `awk` cannot read fails only when the pipeline reports it. A dropped
`lint` prerequisite would disable the whole contract without failing any check
of the manifests themselves.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
SHIPPED_MANIFESTS = sorted((REPO_ROOT / "skills").glob("*/SKILL.md"))


def _run_make(*arguments: str) -> subprocess.CompletedProcess[str]:
    """Run one Makefile target in the repository root."""
    return subprocess.run(
        ["make", *arguments],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def _skill_dirs_argument(*skill_dirs: Path) -> list[str]:
    """Render the `SKILL_DIRS` override for a set of skill directories."""
    if not skill_dirs:
        return []
    directories = " ".join(f"{directory}/" for directory in skill_dirs)
    return [f"SKILL_DIRS={directories}"]


def _run_manifest_check(*skill_dirs: Path) -> subprocess.CompletedProcess[str]:
    """Run the manifest contract over the shipped skills or given fixtures."""
    return _run_make("skill-manifest-check", *_skill_dirs_argument(*skill_dirs))


def _frontmatter(manifest: Path) -> dict[str, object]:
    """Parse the YAML frontmatter block of a skill manifest."""
    lines = manifest.read_text(encoding="utf-8").splitlines()
    assert lines and lines[0] == "---", (
        f"{manifest} does not open with a frontmatter fence"
    )
    closing = lines.index("---", 1)
    return yaml.safe_load("\n".join(lines[1:closing])) or {}


def _write_manifest(skill_dir: Path, body: str) -> Path:
    """Create a skill directory containing the given manifest text."""
    skill_dir.mkdir()
    (skill_dir / "SKILL.md").write_text(body, encoding="utf-8")
    return skill_dir


def test_default_skill_dirs_discover_every_shipped_skill() -> None:
    """The default `SKILL_DIRS` glob finds every shipped skill, and is not empty.

    `SKILL_DIRS` defaults to a `skills/*/SKILL.md` glob, so a glob that matched
    nothing would let every manifest target pass without checking anything.
    """
    assert SHIPPED_MANIFESTS, "the repository must ship at least one skill manifest"

    result = _run_make("--dry-run", "skill-manifest-validate")

    assert result.returncode == 0, result.stdout + result.stderr
    for manifest in SHIPPED_MANIFESTS:
        directory = manifest.parent.relative_to(REPO_ROOT)
        assert str(directory) in result.stdout, (
            f"the default SKILL_DIRS must include {directory}; recorded: {result.stdout!r}"
        )


def test_shipped_skill_manifests_satisfy_the_contract() -> None:
    """Every shipped skill passes YAML and Agent Skills schema validation."""
    result = _run_manifest_check()

    assert result.returncode == 0, result.stdout + result.stderr


@pytest.mark.parametrize(
    ("case", "frontmatter"),
    [
        ("missing", "description: A fixture that lacks the required discovery name.\n"),
        ("empty", 'name: ""\ndescription: A fixture whose discovery name is empty.\n'),
    ],
)
def test_manifest_check_rejects_an_unusable_name(
    tmp_path: Path, case: str, frontmatter: str
) -> None:
    """An absent and an empty discovery name both fail discovery.

    A strict loader cannot discover a skill without a usable discovery name, so
    the contract must reject both rather than only the absent case.
    """
    skill_dir = _write_manifest(
        tmp_path / f"{case}-name",
        f"---\n{frontmatter}---\n\n# Fixture\n",
    )

    result = _run_manifest_check(skill_dir)

    assert result.returncode != 0, result.stdout + result.stderr


@pytest.mark.parametrize(
    "manifest", SHIPPED_MANIFESTS, ids=lambda path: path.parent.name
)
def test_shipped_metadata_values_are_strings(manifest: Path) -> None:
    """Metadata carries only string values, which `skills-ref` silently coerces.

    `skills_ref.parser` rewrites every metadata value with `str(v)`, so a YAML
    sequence survives validation but reaches consumers as a Python repr. The
    specification permits string keys and string values only, so reject the
    non-conformant shapes here rather than shipping a silently mangled value.
    """
    metadata = _frontmatter(manifest).get("metadata", {})

    assert isinstance(metadata, dict), (
        f"metadata must be a mapping, got {type(metadata).__name__}"
    )
    non_strings = {
        key: value for key, value in metadata.items() if not isinstance(value, str)
    }
    assert not non_strings, f"metadata values must be strings: {non_strings}"


def test_frontmatter_lint_reports_an_early_failure(tmp_path: Path) -> None:
    """A failure in any skill fails the target, not just one in the final skill.

    The shell `for` loop otherwise exits with the status of its last iteration,
    letting a conformant trailing skill mask a malformed earlier one.
    """
    broken = _write_manifest(
        tmp_path / "a-broken", "---\nname: [unclosed\n---\n\n# Broken\n"
    )
    valid = _write_manifest(
        tmp_path / "z-valid",
        "---\nname: z-valid\ndescription: A conformant trailing fixture.\n---\n\n# Valid\n",
    )

    result = _run_make("skill-frontmatter-lint", *_skill_dirs_argument(broken, valid))

    assert result.returncode != 0, result.stdout + result.stderr


def test_frontmatter_lint_reports_an_unreadable_manifest(tmp_path: Path) -> None:
    """A manifest that cannot be read fails the target rather than being skipped.

    `awk` fails to read a missing `SKILL.md`, a distinct failure path from
    `yamllint` rejecting parsed content — `yamllint` accepts the empty input the
    failed `awk` leaves behind — so the check only holds while the pipeline
    reports the `awk` status.
    """
    absent = tmp_path / "absent"
    absent.mkdir()
    valid = _write_manifest(
        tmp_path / "z-valid",
        "---\nname: z-valid\ndescription: A conformant trailing fixture.\n---\n\n# Valid\n",
    )

    result = _run_make("skill-frontmatter-lint", *_skill_dirs_argument(absent, valid))

    assert result.returncode != 0, result.stdout + result.stderr


def test_lint_depends_on_the_manifest_check() -> None:
    """`make lint` runs the manifest contract, so dropping the prerequisite fails.

    The contract is only enforced because `lint` depends on
    `skill-manifest-check`; without this test, removing that prerequisite would
    silently disable manifest validation while every other test still passed.
    """
    makefile = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    lint_line = next(
        (line for line in makefile.splitlines() if re.match(r"lint:", line)),
        None,
    )

    assert lint_line is not None, "the Makefile must define the lint target"
    prerequisites = lint_line.split(":", 1)[1].split("##", 1)[0].split()
    assert "skill-manifest-check" in prerequisites, (
        f"lint must depend on skill-manifest-check; recorded: {prerequisites!r}"
    )


def test_make_recipes_run_under_bash() -> None:
    """The Makefile selects Bash, so a recipe can rely on Bash-only options.

    The workflows run `make` under Bash, but `defaults.run.shell` governs the
    `run` steps written in a workflow; Make selects the shell for a recipe
    itself. It otherwise executes recipes with `/bin/sh`, which is `dash` on the
    CI runner and rejects the `set -o pipefail` that `skill-frontmatter-lint`
    relies on. The probe is deliberately brittle about which shell answers:
    `$BASH_VERSION` is set only by Bash, so the target fails wherever Make falls
    back to `sh`, however `sh` is provided on that host.
    """
    probe = (
        "recipe-shell-probe:\n"
        '\t@test -n "$$BASH_VERSION" ||'
        ' { echo "recipes do not run under Bash"; exit 1; }\n'
    )

    result = _run_make("--eval", probe, "recipe-shell-probe")

    assert result.returncode == 0, result.stdout + result.stderr
