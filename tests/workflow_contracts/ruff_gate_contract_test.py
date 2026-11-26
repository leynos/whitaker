"""Hold the Python lint and format gates to their whole-tree coverage.

The gate this file guards exists because the previous invocation linted two
files out of forty-eight with Ruff's default rule set. A reviewer saw a green
gate that certified almost none of the tree. These contracts make the silent
regressions fail instead:

* `lint` must compose `lint-python`, so the gate cannot be dropped from the
  composed target while each half still passes;
* `lint-python` must check the repository root and `check-fmt` must verify the
  formatter over the same set, so the source set cannot shrink back to a
  hand-written list;
* `--isolated` must not return: it would discard `ruff.toml` and restore the
  default rule set without changing the file list;
* `ruff.toml` must select the documented rule families and confine its
  test-only ignores to test paths, so the policy cannot narrow unreviewed.

The discovery check runs the pinned Ruff from the Makefile over the working
tree and compares what it finds with `git ls-files`. That is the evidence the
issue asked for: every Python file the repository ships, not a list that
looks right.

Run this contract with ``make test-workflow-contracts``.
"""

from __future__ import annotations

import os
import re
import shlex
import subprocess
import tomllib
from pathlib import Path

import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPOSITORY_ROOT / "Makefile"
RUFF_CONFIG_PATH = REPOSITORY_ROOT / "ruff.toml"
CI_WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"

#: Rule families the configuration must select. Dropping one narrows the gate
#: without changing any command line, so the loss would otherwise be invisible.
REQUIRED_RULES = {"D", "ANN", "E501", "S101", "PLR0913", "PLR0917"}

#: Rules that are legitimate in tests only. They must not be ignored for a
#: production path, or the gate would stop enforcing them where they matter.
TEST_ONLY_RULES = {"S101", "PLR0913", "PLR0917"}

#: Per-file-ignore patterns that identify test code.
TEST_PATTERNS = ("**/tests/**", "**/test_*.py", "**/*_test.py")


def _makefile_lines() -> list[str]:
    """Return the Makefile split into lines."""
    return MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()


def _declaration(target: str) -> str:
    """Return the text after ``target:`` on its declaration line."""
    pattern = re.compile(rf"^{re.escape(target)}:(.*)$")
    for line in _makefile_lines():
        match = pattern.match(line)
        if match:
            return match.group(1)
    raise AssertionError(f"the Makefile must declare a {target} target")


def _recipe_lines(target: str) -> list[str]:
    """Return the tab-indented command lines of a Makefile target's recipe."""
    lines = _makefile_lines()
    start = next(
        (
            index + 1
            for index, line in enumerate(lines)
            if line.startswith(f"{target}:")
        ),
        None,
    )
    if start is None:
        raise AssertionError(f"the Makefile must declare a {target} target")
    recipe: list[str] = []
    for line in lines[start:]:
        if not line.startswith("\t"):
            break
        recipe.append(line)
    return recipe


def _ruff_arguments(target: str) -> list[str]:
    """Return the argument vector of a target's Ruff invocation.

    The Ruff command is the recipe's final command, so the tokens after
    ``$(RUFF)`` are exactly its subcommand and operands. Comparing the whole
    vector rejects a narrowed operand such as ``./scripts`` that a substring
    check would accept.
    """
    recipe = " ".join(_recipe_lines(target)).replace("\\", " ")
    tokens = shlex.split(recipe)
    try:
        return tokens[tokens.index("$(RUFF)") + 1 :]
    except ValueError as error:
        raise AssertionError(f"{target} must invoke $(RUFF)") from error


def _assignment(name: str) -> str:
    """Return the value of a simple ``NAME ?= value`` Makefile assignment."""
    pattern = re.compile(rf"^{re.escape(name)}\s*\?=\s*(.+)$")
    for line in _makefile_lines():
        match = pattern.match(line)
        if match:
            return match.group(1).strip()
    raise AssertionError(f"the Makefile must assign {name} with ?=")


def _ruff_config() -> dict[str, object]:
    """Return the decoded checked-in Ruff configuration."""
    with RUFF_CONFIG_PATH.open("rb") as handle:
        return tomllib.load(handle)


def _discovered_python_files() -> set[Path]:
    """Return the files the checked-in Ruff configuration discovers."""
    version = _assignment("RUFF_VERSION")
    environment = {
        **os.environ,
        # Match the Makefile's repository-local caches so this check reuses the
        # tool the lint gate already installed instead of fetching a second copy.
        "UV_CACHE_DIR": str(REPOSITORY_ROOT / ".uv-cache"),
        "UV_TOOL_DIR": str(REPOSITORY_ROOT / ".uv-tools"),
    }
    result = subprocess.run(
        ["uv", "tool", "run", f"ruff@{version}", "check", "--show-files", "."],
        cwd=REPOSITORY_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, (
        "Ruff must report the files it discovers; "
        f"exit {result.returncode}: {result.stderr}"
    )
    return {Path(line).resolve() for line in result.stdout.splitlines() if line}


def test_lint_composes_the_python_gate() -> None:
    """`lint` must stay a composition that includes the Python lane.

    The lane is a prerequisite of `lint`, so dropping it removes the gate
    without leaving a trace in any recipe a reader would notice.
    """
    declaration = _declaration("lint")
    assert "lint-python" in declaration, (
        "the lint target must depend on lint-python; dropping it silently "
        f"stops enforcing the Python policy. Found: {declaration!r}"
    )


def test_lint_python_checks_the_repository_root() -> None:
    """The Python lane must check the whole tree under the checked-in config.

    Passing a directory list is how the old gate came to certify two files.
    `--isolated` is equally destructive in the other direction: it discards
    `ruff.toml` and restores Ruff's default rule set.
    """
    arguments = _ruff_arguments("lint-python")
    assert arguments == ["check", "."], (
        "lint-python must run exactly `$(RUFF) check .` so Ruff discovers "
        f"every Python file from the repository root. Found: {arguments!r}"
    )


def test_check_fmt_verifies_python_formatting() -> None:
    """`check-fmt` must verify the formatter over the same source set."""
    arguments = _ruff_arguments("check-fmt")
    assert arguments == ["format", "--check", "."], (
        "check-fmt must run exactly `$(RUFF) format --check .` so the "
        f"formatting half covers the linted tree. Found: {arguments!r}"
    )


def test_fmt_applies_the_same_python_formatting() -> None:
    """`fmt` must apply what `check-fmt` verifies, or the gate is one-way."""
    arguments = _ruff_arguments("fmt")
    assert arguments == ["format", "."], (
        "fmt must run exactly `$(RUFF) format .`; otherwise `make fmt` cannot "
        f"produce the state check-fmt accepts. Found: {arguments!r}"
    )


def test_ruff_version_is_a_pinned_release() -> None:
    """A floating version would change the rule set under a green gate."""
    version = _assignment("RUFF_VERSION")
    assert re.fullmatch(r"\d+\.\d+\.\d+", version), (
        f"RUFF_VERSION must pin an exact release, found {version!r}"
    )


def test_config_selects_the_documented_rule_families() -> None:
    """The configuration must state the policy instead of inheriting one."""
    config = _ruff_config()
    assert config.get("include") == ["**/*.py"], (
        "include must restrict discovery to Python files; "
        f"found {config.get('include')!r}"
    )
    assert config.get("target-version") == "py314", (
        f"target-version must name the gate's interpreter, found "
        f"{config.get('target-version')!r}"
    )
    assert config.get("line-length") == 88, (
        f"line-length must stay at the documented 88, found "
        f"{config.get('line-length')!r}"
    )

    lint = config.get("lint")
    assert isinstance(lint, dict), "ruff.toml must declare a [lint] table"
    selected = set(lint.get("select", ()))
    missing = sorted(REQUIRED_RULES - selected)
    assert not missing, (
        f"ruff.toml must select the documented rule families; missing: {missing}"
    )

    pydocstyle = lint.get("pydocstyle")
    assert isinstance(pydocstyle, dict), "ruff.toml must declare [lint.pydocstyle]"
    assert pydocstyle.get("convention") == "numpy", (
        "docstring rules must use the NumPy convention these modules follow; "
        f"found {pydocstyle.get('convention')!r}"
    )


def test_test_only_rules_are_ignored_only_in_tests() -> None:
    """Test-only ignores must not leak onto production paths."""
    config = _ruff_config()
    lint = config.get("lint")
    assert isinstance(lint, dict), "ruff.toml must declare a [lint] table"
    ignores = lint.get("per-file-ignores")
    assert isinstance(ignores, dict), "ruff.toml must declare per-file ignores"

    for pattern in TEST_PATTERNS:
        ignored = set(ignores.get(pattern, ()))
        missing = sorted(TEST_ONLY_RULES - ignored)
        assert not missing, (
            f"the test pattern {pattern!r} must ignore {missing}; tests assert "
            "and take long parameter lists by design"
        )

    production_ignores = {
        code
        for pattern, codes in ignores.items()
        if pattern not in TEST_PATTERNS
        for code in codes
    }
    leaked = sorted(TEST_ONLY_RULES & production_ignores)
    assert not leaked, (
        f"test-only rules must not be ignored outside test paths; leaked: {leaked}"
    )


def test_ruff_discovers_every_tracked_python_file() -> None:
    """Every Python file Git tracks must be in the gate's source set."""
    tracked = subprocess.run(
        ["git", "ls-files", "*.py"],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    shipped = {
        (REPOSITORY_ROOT / line).resolve() for line in tracked.stdout.splitlines()
    }
    discovered = _discovered_python_files()
    missing = sorted(shipped - discovered)
    assert not missing, (
        "the Ruff gate does not discover tracked Python files: "
        f"{[str(path.relative_to(REPOSITORY_ROOT)) for path in missing]}"
    )


def _check_fmt_index(steps: list[object]) -> int | None:
    """Return the index of a step that runs ``make check-fmt``, or ``None``.

    The step is identified by its command rather than its display name, so
    renaming it cannot remove it from inspection, and a step merely named for
    formatting but running something else cannot satisfy the contract.
    """
    for index, step in enumerate(steps):
        if isinstance(step, dict) and "make check-fmt" in str(step.get("run", "")):
            return index
    return None


def test_ci_sets_up_uv_before_the_formatting_gate() -> None:
    """CI must install uv before `make check-fmt` runs the Python formatter."""
    workflow = yaml.safe_load(CI_WORKFLOW_PATH.read_text(encoding="utf-8"))
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "ci.yml must declare jobs"

    checked = 0
    for job_name, job in jobs.items():
        steps = job.get("steps") if isinstance(job, dict) else None
        if not isinstance(steps, list):
            continue
        check = _check_fmt_index(steps)
        if check is None:
            continue
        checked += 1
        setup = next(
            (
                index
                for index, step in enumerate(steps)
                if isinstance(step, dict)
                and str(step.get("uses", "")).startswith("astral-sh/setup-uv")
            ),
            None,
        )
        assert setup is not None, (
            f"{job_name} runs `make check-fmt`, which now invokes Ruff, but "
            "never sets up uv"
        )
        assert setup < check, (
            f"{job_name} must set up uv before `make check-fmt`; found uv at "
            f"step {setup} and formatting at step {check}"
        )

    assert checked, "ci.yml must run `make check-fmt` in at least one job"
