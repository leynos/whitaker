"""Contract tests for the `lint` gate and the CI job that installs its tools.

`make lint` is two gates, not one: rustdoc plus Clippy, and the Whitaker
Dylint suite. Each half has a failure mode that a naive check misses.

- rustdoc and Clippy deny warnings through variables that are easy to drop
  while editing the recipe, and a dropped flag turns the gate green rather
  than red.
- `cargo-dylint` warns and *exits 0* when it discovers no libraries. A gate
  that selects `--all` therefore passes silently when the suite is missing or
  only partly installed. Naming the aggregate library turns that case into an
  error, and the packages under `--` must stay inside the support crates that
  can build without `rustc_private` plumbing.

The lint gate also needs a prerequisite the Makefile cannot install: the
`whitaker` binary. It is provided by `whitaker-installer`, so the CI workflow
must install it as its own step, before `make lint`, at the version the
workflow pins.

Most of these tests read the Makefile recipe text and parse the workflow YAML
without running either. The exception is the install step: its branch structure
is only observable by execution, so it is run against stubs that record every
command and touch neither the network nor the host. Run via
``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import subprocess
import tempfile
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPO_ROOT / "Makefile"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"


def _recipe_lines(target: str) -> list[str]:
    """Return the tab-indented command lines of a Makefile target's recipe."""
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    start = next(
        (
            index + 1
            for index, line in enumerate(lines)
            if re.match(rf"{re.escape(target)}:", line)
        ),
        len(lines),
    )
    recipe: list[str] = []
    for line in lines[start:]:
        if not line.startswith("\t"):
            break
        recipe.append(line)
    return recipe


def _target_line(target: str) -> str:
    """Return the declaration line of a Makefile target."""
    pattern = re.compile(rf"^{re.escape(target)}:(.*)$")
    for line in MAKEFILE_PATH.read_text(encoding="utf-8").splitlines():
        match = pattern.match(line)
        if match:
            return match.group(1)
    raise AssertionError(f"the Makefile must declare a {target} target")


def _variable(name: str) -> str:
    """Return the value of a simple `NAME ?= value` Makefile assignment."""
    pattern = re.compile(rf"^{re.escape(name)}\s*\?=\s*(.+)$")
    for line in MAKEFILE_PATH.read_text(encoding="utf-8").splitlines():
        match = pattern.match(line)
        if match:
            return match.group(1).strip()
    raise AssertionError(f"the Makefile must assign {name} with ?=")


def test_lint_runs_both_the_clippy_and_whitaker_gates() -> None:
    """`lint` must stay a composition of both halves, in either order.

    The two halves are prerequisites of `lint`, so a dropped one removes the
    gate without leaving a trace in any recipe that a reader would notice.
    """
    prerequisites = _target_line("lint")
    assert prerequisites.strip(), "the lint target must declare prerequisites"

    for dependency in ("lint-clippy", "lint-whitaker"):
        assert dependency in prerequisites, (
            f"the lint target must depend on {dependency}; a gate that drops "
            f"one half silently stops enforcing it. Found: {prerequisites!r}"
        )


def test_lint_clippy_denies_warnings_in_both_tools() -> None:
    """Both halves of the Clippy gate must deny warnings.

    `RUSTDOCFLAGS` and `RUST_FLAGS` are the only reason a warning fails this
    gate; without them rustdoc and Clippy report warnings and exit 0.
    """
    recipe = " ".join(_recipe_lines("lint-clippy"))
    assert recipe, "the lint-clippy target must have a recipe"

    assert "$(RUSTDOC_FLAGS)" in recipe, (
        "the rustdoc invocation must pass $(RUSTDOC_FLAGS), which carries the "
        "-D warnings denial"
    )
    assert "$(RUST_FLAGS)" in recipe, (
        "the Clippy invocation must pass $(RUST_FLAGS), which carries the "
        "-D warnings denial"
    )

    rustdoc_flags = _variable("RUSTDOC_FLAGS")
    rust_flags = _variable("RUST_FLAGS")
    for name, value in (("RUSTDOC_FLAGS", rustdoc_flags), ("RUST_FLAGS", rust_flags)):
        assert "-D warnings" in value, (
            f"{name} must deny warnings so the gate can fail; found {value!r}"
        )


def test_lint_whitaker_names_the_aggregate_library_and_its_packages() -> None:
    """The suite gate must name `whitaker_suite`, not rely on `--all`.

    `cargo-dylint --all` warns and exits 0 when it discovers no libraries, so a
    missing or partially installed suite would pass the gate. Naming the
    aggregate library makes that an error.
    """
    recipe = " ".join(_recipe_lines("lint-whitaker"))
    assert recipe, "the lint-whitaker target must have a recipe"

    assert "--lib whitaker_suite" in recipe, (
        "lint-whitaker must select --lib whitaker_suite; without a named "
        "library, an empty discovery is a warning and exit 0"
    )
    assert "--all " not in recipe, (
        "lint-whitaker must not fall back to --all; that is the flag whose "
        "empty-discovery case exits 0"
    )
    assert "$(RUST_FLAGS)" in recipe, (
        "lint-whitaker must pass $(RUST_FLAGS) so suite diagnostics fail the gate"
    )
    assert "$(WHITAKER_PACKAGES)" in recipe, (
        "lint-whitaker must pass $(WHITAKER_PACKAGES) so the analysed crate set "
        "stays configurable and reviewable"
    )
    # The flags after `--` decide *what* gets analysed. Dropping them narrows
    # the gate to the default lib/bin targets of the support crates, which
    # still exits 0, so a silently shrunken lint would read as a passing gate.
    for required in ("--all-targets", "--all-features"):
        assert required in recipe, (
            f"lint-whitaker must pass {required!r}; without it the suite "
            f"analyses fewer targets and still exits 0. Found {recipe!r}"
        )


def test_whitaker_packages_stay_inside_the_support_crates() -> None:
    """Every analysed package must be one that builds without `rustc_private`.

    The lint crates, the aggregate suite, the rustc proxy shims and the
    `whitaker` root crate all need the `dylint-driver` feature and
    `prefer-dynamic` RUSTFLAGS, which `cargo dylint`'s plain check build does
    not provide. Adding one of them here would make the gate fail for a reason
    unrelated to the code under review.
    """
    packages = _variable("WHITAKER_PACKAGES").split()
    assert packages, "WHITAKER_PACKAGES must name at least one package"

    analysed = [token.removeprefix("-p") for token in packages if token != "-p"]
    assert len(analysed) == packages.count("-p"), (
        f"WHITAKER_PACKAGES must be a flat sequence of -p NAME pairs; found "
        f"{packages!r}"
    )

    forbidden = {
        "whitaker",
        "whitaker_suite",
        "clippy_utils",
        "func_type_and_gen",
        "rustc_ast",
        "rustc_attr_data_structures",
        "rustc_hir",
        "rustc_lint",
        "rustc_middle",
        "rustc_session",
        "rustc_span",
    }
    assert forbidden.isdisjoint(analysed), (
        "WHITAKER_PACKAGES must not name rustc_private or driver crates: "
        f"{sorted(forbidden & set(analysed))}"
    )

    # A ban list alone lets the set shrink to something that still passes:
    # dropping a support crate narrows what the suite analyses without any
    # error, so the gate would cover less while continuing to report success.
    # Naming the intended set makes an accidental deletion a failure, and the
    # reason to add one is then a deliberate edit to this list as well.
    intended = {
        "whitaker-common",
        "whitaker-installer",
        "whitaker_clones_core",
        "whitaker_sarif",
    }
    assert intended <= set(analysed), (
        "WHITAKER_PACKAGES has lost support crates, so the suite now analyses "
        f"less than the gate claims. Missing: {sorted(intended - set(analysed))}"
    )


def test_the_root_library_tests_are_reachable_from_the_test_gate() -> None:
    """The root package's library tests must run under `make test`.

    `TEST_EXCLUDES` may name the `whitaker` root package, because its test
    *binaries* cannot build under `--all-features`: the `dylint-driver` feature
    pulls `rustc_private` and rustc rejects the duplicate prelude crates. The
    *library* does build. What must not happen is excluding the package and
    then never running its library tests, because
    `src/testing/ui/tests.rs` holds the Dylint copy-library regression tests
    and their loss would only surface later as a UI harness failing in CI.

    The check is conditional so that fixing the underlying link problem and
    dropping the exclusion wholesale stays allowed; what it forbids is the
    silent gap between the two.
    """
    if "--exclude whitaker" not in _variable("TEST_EXCLUDES"):
        return

    recipe = " ".join(_recipe_lines("test-root-lib"))
    assert recipe, (
        "TEST_EXCLUDES skips the whitaker root package, so a test-root-lib "
        "target must run the library tests it hides"
    )
    for required in ("-p whitaker", "--lib", "--all-features"):
        assert required in recipe, (
            f"test-root-lib must select {required!r} so it runs the package the "
            f"exclusion skips, in the only shape that links; found {recipe!r}"
        )
    assert "--tests" not in recipe, (
        "test-root-lib must not build the root test binaries; under "
        "--all-features they fail with 'cannot satisfy dependencies so `std` "
        "only shows up once'"
    )

    test_recipe = " ".join(_recipe_lines("test"))
    assert "test-root-lib" in test_recipe, (
        "`make test` must invoke test-root-lib, or the root library tests are "
        "unreachable from the repository's gate"
    )


def test_the_nextest_filter_regression_guard_is_reachable_from_the_test_gate() -> None:
    """The nextest override guard must run under `make test`.

    `tests/nextest_ui_filter.rs` is an *integration* binary of the `whitaker`
    root package, so it is covered by `TEST_EXCLUDES` exactly as the root
    library is, and `test-root-lib`'s `--lib` does not reach it. Without a
    target that names this test, nothing in the repository runs it.

    That gap is worth its own check because the defect it guards is silent: a
    shadowed or non-matching override does not fail the build, it just stops
    serializing and sizing the dylint UI harnesses, and the symptom appears
    later as flaky Windows CI far from the configuration that caused it.
    """
    if "--exclude whitaker" not in _variable("TEST_EXCLUDES"):
        return

    recipe = " ".join(_recipe_lines("test-nextest-filter"))
    assert recipe, (
        "TEST_EXCLUDES skips the whitaker root package, so a test-nextest-filter "
        "target must run the nextest override guard it hides"
    )
    for required in ("-p whitaker", "--test nextest_ui_filter", "--all-features"):
        assert required in recipe, (
            f"test-nextest-filter must select {required!r} so it runs the "
            f"excluded integration binary; found {recipe!r}"
        )
    assert "--lib" not in recipe, (
        "test-nextest-filter must not select --lib; that shape runs the library "
        "and would leave the integration binary unexercised"
    )

    test_recipe = " ".join(_recipe_lines("test"))
    assert "test-nextest-filter" in test_recipe, (
        "`make test` must invoke test-nextest-filter, or the nextest override "
        "guard is unreachable from the repository's gate"
    )


def _load_ci() -> dict[str, object]:
    """Parse the CI workflow."""
    return yaml.safe_load(CI_WORKFLOW_PATH.read_text(encoding="utf-8"))


def _linux_steps() -> list[dict[str, object]]:
    """Return the ordered step mappings of the `linux-full` job."""
    workflow = _load_ci()
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the CI workflow must declare jobs"
    linux_job = jobs.get("linux-full")
    assert isinstance(linux_job, dict), "the CI workflow must declare linux-full"
    steps = linux_job.get("steps")
    assert isinstance(steps, list), "linux-full must declare steps"
    return steps


def _step_named(name: str) -> tuple[int, dict[str, object]]:
    """Return the index and mapping of a named `linux-full` step."""
    for index, step in enumerate(_linux_steps()):
        if isinstance(step, dict) and step.get("name") == name:
            return index, step
    raise AssertionError(f"linux-full must include a {name!r} step")


# Every command the install script can issue, plus `command` itself, is
# replaced by a stub that records its arguments and reports a fixed success or
# failure. Anything else the script reaches for is a hard error rather than a
# real execution, so the harness cannot run an unrelated program on the host.
_INSTALL_STUB = """\
#!/bin/sh
printf '%s\\n' "$0 $*" >>"$STUB_LOG"
# A successful install must leave the binary runnable, so the script's own
# final invocation resolves. Only a download creates it.
case "$0" in
  */cargo|cargo)
    printf '#!/bin/sh\\nprintf "%%s\\\\n" "$0 $*" >>"$STUB_LOG"\\n' \\
      >"$STUB_BIN/whitaker-installer"
    chmod 0755 "$STUB_BIN/whitaker-installer" ;;
esac
exit 0
"""


def _run_install_script(script: str, *, cached: bool, version: str) -> list[str]:
    """Execute the CI install step against stubs and return what it ran.

    `script` is the workflow's `run:` body, so this exercises the branch
    structure the workflow actually ships rather than a copy of it. The
    returned list is each invocation in order, as `program args`. Stub state is
    per-call: a fresh directory holds the stubs and the log, so cases cannot
    leak into one another.
    """
    with tempfile.TemporaryDirectory() as scratch:
        directory = Path(scratch)
        bin_dir = directory / "bin"
        bin_dir.mkdir()
        log = directory / "calls.log"
        log.touch()

        stubs = ["cargo"]
        # `command -v` is a shell builtin, so the cache probe cannot be stubbed;
        # whether the binary is "already installed" is expressed by whether it
        # exists on PATH at all.
        if cached:
            stubs.append("whitaker-installer")
        for name in stubs:
            stub = bin_dir / name
            stub.write_text(_INSTALL_STUB, encoding="utf-8")
            stub.chmod(0o755)

        environment = {
            # Keep the script's own semantics (`#!/bin/sh`, `set -e`) but make
            # every command it calls a stub, and let it see the pinned version.
            "PATH": f"{bin_dir}:/usr/bin:/bin",
            "HOME": scratch,
            "STUB_LOG": str(log),
            "STUB_BIN": str(bin_dir),
            "WHITAKER_INSTALLER_VERSION": version,
        }
        result = subprocess.run(
            ["/bin/sh", "-e", "-c", script],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        assert result.returncode == 0, (
            "the install step must succeed under the stubs; it failed with "
            f"{result.returncode}:\n{result.stderr}"
        )

        return [
            line.replace(str(bin_dir) + "/", "", 1).strip()
            for line in log.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]


def test_ci_installs_whitaker_before_the_lint_gate() -> None:
    """`make lint` needs the `whitaker` binary, so CI must install it first.

    Installing inside the gate would turn a broken or rate-limited download
    into a lint failure with no installer diagnostics, and would hide the fact
    that the prerequisite exists at all. A separate named step keeps the two
    failures distinguishable and the ordering explicit.
    """
    install_index, install = _step_named("Install Whitaker")
    lint_index, lint = _step_named("Lint")

    assert install_index < lint_index, (
        "Install Whitaker must precede the Lint step: the Whitaker suite gate "
        "invokes the binary that step provides"
    )
    assert lint.get("run") == "make lint", (
        "the Lint step must run `make lint`, which covers both the Clippy and "
        f"Whitaker halves; found {lint.get('run')!r}"
    )

    script = install.get("run")
    assert isinstance(script, str), "Install Whitaker must run a shell script"

    # Reading the script is not enough. Text assertions cannot tell which
    # branch runs, and a deleted standalone execution still satisfies a
    # "mentions whitaker-installer" check. Running the script against stubs
    # pins the behaviour instead: the cached path must skip the download, the
    # download path must run at the pinned version, and the freshly installed
    # binary must then actually be executed in both cases.
    #
    # There is deliberately no `cargo install` case. `linux-full` runs on paid
    # Ubicloud runners, where `ubicloud_tool_provenance_test` forbids building
    # a published binary from source; the binstall provisioning step is what
    # makes that unconditional, so a fallback here would be unreachable code
    # whose only effect would be to reintroduce the prohibited path.
    cached = _run_install_script(script, cached=True, version="9.9.9")
    assert not any("binstall" in call for call in cached), (
        "the cached path must not download anything; found "
        f"{[call for call in cached if 'binstall' in call]!r}"
    )
    assert cached[-1] == "whitaker-installer", (
        "the cached path must still execute the installed binary; found "
        f"{cached!r}"
    )

    installed = _run_install_script(script, cached=False, version="9.9.9")
    assert installed[0].startswith("cargo binstall"), (
        f"the download must use binstall; found {installed!r}"
    )
    assert "9.9.9" in installed[0], (
        f"the binstall command must use the pinned version; found {installed[0]!r}"
    )
    assert installed[-1] == "whitaker-installer", (
        "the download path must execute the installed binary after install; "
        f"found {installed!r}"
    )


def test_ci_pins_the_installer_version_and_keys_its_cache_on_it() -> None:
    """The installer pin must live at workflow level and key the cache."""
    workflow = _load_ci()
    env = workflow.get("env")
    assert isinstance(env, dict), "the CI workflow must declare an env mapping"

    pinned = env.get("WHITAKER_INSTALLER_VERSION")
    assert isinstance(pinned, str) and pinned, (
        "the workflow must pin WHITAKER_INSTALLER_VERSION at workflow level"
    )

    cache_index, cache = _step_named("Cache Whitaker installer")
    install_index, _ = _step_named("Install Whitaker")
    assert cache_index < install_index, (
        "the installer cache must be restored before the install step uses it"
    )

    cache_with = cache.get("with")
    assert isinstance(cache_with, dict), "the installer cache step must configure `with`"
    key = cache_with.get("key")
    assert isinstance(key, str), "the installer cache step must set a key"

    for marker in ("${{ runner.os }}", "${{ runner.arch }}", "WHITAKER_INSTALLER_VERSION"):
        assert marker in key, (
            f"the installer cache key must include {marker!r} so a new pin is "
            f"not served from the previous version's entry; found {key!r}"
        )

    path = cache_with.get("path")
    assert path is not None, "the installer cache step must set a path"
    assert "whitaker-installer" in "\n".join(
        path if isinstance(path, list) else [str(path)]
    ), "the installer cache must cover the whitaker-installer binary itself"
