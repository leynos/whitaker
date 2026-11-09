"""Verify that Makefile Cargo invocations consistently honour CARGO_LOCKED."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.workflows.cargo_locked_harness import (
    _LOCK_RELEVANT_CARGO,
    REPO_ROOT,
    MakeRunEnvironment,
    _assert_make_test_environments,
    _assert_make_test_invocations,
    _cargo_head,
    _cargo_invocation,
    _has_locked_flag,
    _is_lock_relevant_invocation,
    _make_log_name,
    _makefile_recipe_lines,
    _run_make,
)
from tests.workflows.cargo_locked_stubs import (
    _write_cargo_stub,
    _write_publish_check_cargo_stub,
    _write_stub,
    _write_tool_stubs,
)


def test_recipe_lines_are_empty_for_an_absent_target() -> None:
    """An unknown Makefile target yields no recipe lines."""
    assert _makefile_recipe_lines("definitely-not-a-real-target") == [], (
        "an absent Makefile target must yield no recipe lines"
    )


def test_coverage_preserves_data_without_conflicting_llvm_cov_options() -> None:
    """Both coverage passes reuse data without incompatible cleanup flags."""
    coverage_recipe = "\n".join(_makefile_recipe_lines("coverage"))

    assert coverage_recipe.count("llvm-cov nextest --no-report") == 2, (
        "both coverage passes must defer reporting until their data is merged"
    )
    assert 'CORE_TEST_RUNNER="llvm-cov nextest --no-report --no-clean"' not in (
        coverage_recipe
    ), "cargo-llvm-cov rejects --no-clean together with --no-report"


def test_coverage_recipe_exports_one_explicit_nested_cargo_target() -> None:
    """Require outer coverage and nested Cargo builds to share one target."""
    makefile = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    recipe = "\n".join(_makefile_recipe_lines("coverage"))

    assert "COVERAGE_TARGET_DIR ?= $(CURDIR)/target/llvm-cov-target" in makefile, (
        "coverage must provide the default shared Cargo target directory"
    )
    assert 'CARGO_LLVM_COV_TARGET_DIR="$(COVERAGE_TARGET_DIR)"' in recipe, (
        "coverage must direct cargo-llvm-cov to the shared Cargo target directory"
    )
    assert 'CARGO_TARGET_DIR="$(COVERAGE_TARGET_DIR)"' in recipe, (
        "coverage must expose the shared target directory to nested Cargo commands"
    )
    assert "$(MAKE) test" in recipe and (
        'TEST_RUNNER="$(CARGO_LLVM_COV) llvm-cov nextest' in recipe
    ), (
        "coverage must invoke the nested test runner through cargo-llvm-cov"
    )


@pytest.mark.parametrize("target", ["fetch", "test", "publish-check"])
def test_recipe_cargo_calls_thread_cargo_locked(target: str) -> None:
    """Lock-relevant Cargo calls in the recipe forward `$(CARGO_LOCKED)`.

    Complements the stub-executed `make test`/`publish-check` tests with a
    recipe-text check: it isolates each Cargo invocation and asserts the lock
    flag sits *inside* that call, not merely somewhere on the line.
    """
    lock_relevant = [
        line for line in _makefile_recipe_lines(target) if _LOCK_RELEVANT_CARGO.search(line)
    ]
    assert lock_relevant, f"the {target} recipe should invoke Cargo build/test/package"
    for line in lock_relevant:
        invocation = _cargo_invocation(line)
        assert "$(CARGO_LOCKED)" in invocation, (
            f"{target} must thread $(CARGO_LOCKED) inside the Cargo call: {invocation.strip()!r}"
        )


def test_installer_msrv_recipe_installs_the_packaged_crate() -> None:
    """Require the MSRV target to call the checked staged-package gate."""
    recipe = "\n".join(_makefile_recipe_lines("installer-msrv-check"))

    assert "python3 scripts/check_installer_msrv.py" in recipe
    assert 'CARGO="$(CARGO)"' in recipe
    assert "INSTALLER_MSRV_SCRATCH" in recipe














@pytest.mark.parametrize(
    ("locked", "should_use_locked"),
    [(None, True), ("", False), ("--locked", True)],
)
def test_make_test_forwards_cargo_locked(
    cargo_stub: tuple[Path, Path],
    locked: str | None,
    should_use_locked: bool,
) -> None:
    """Prepare dependencies before tests and honour the requested lock mode."""
    stub_dir, cargo = cargo_stub
    _write_tool_stubs(stub_dir)

    # Point the whitaker-script backup safeguard at an absent temp path so the
    # recipe never touches the real `$(HOME)/.local/bin/whitaker`.
    whitaker_script = stub_dir.parent / "whitaker-script"
    environment = MakeRunEnvironment(cargo=cargo, locked=locked, stub_dir=stub_dir)
    recorded = _run_make(
        "test",
        environment,
        extra_make_args=[f"WHITAKER_SCRIPT={whitaker_script}"],
    )

    log_name = _make_log_name(environment)
    cargo_calls = [line for line in recorded if _is_lock_relevant_invocation(line)]
    _assert_make_test_invocations(
        cargo_calls,
        recorded,
        should_use_locked=should_use_locked,
        log_name=log_name,
    )
    expected_driver_path = str(
        stub_dir.parent / "workspace" / "dylint-drivers" / "test-driver"
    )
    _assert_make_test_environments(
        stub_dir / f"test-{log_name}.env",
        cargo=cargo,
        expected_driver_path=expected_driver_path,
    )


def test_fetch_clears_an_inherited_offline_mode(
    cargo_stub: tuple[Path, Path],
) -> None:
    """Preparation runs online even when the caller exported offline mode.

    `fetch` is the step that populates an empty registry, so inheriting
    `CARGO_NET_OFFLINE=true` would make preparation fail before it did any
    work. The snapshot tool is the first Cargo call of the prerequisite, so it
    is the one that proves the policy starts before the `fetch` recipe body.
    """
    stub_dir, cargo = cargo_stub
    environment = MakeRunEnvironment(cargo=cargo, locked=None, stub_dir=stub_dir)

    recorded = _run_make(
        "fetch",
        environment,
        extra_env={"CARGO_NET_OFFLINE": "true"},
    )

    heads = [_cargo_head(line) for line in recorded]
    assert heads == ["run", "fetch", "run", "metadata", "metadata", "build"], (
        f"fetch must install the snapshot, fetch, warm the driver, then write "
        f"metadata; recorded: {recorded!r}"
    )
    offline_modes = [
        line.split("\t", maxsplit=1)[0]
        for line in (stub_dir / "fetch-default.env").read_text(encoding="utf-8").splitlines()
    ]
    assert offline_modes == ["<unset>"] * len(heads), (
        "every preparation call, including the snapshot tool build, must clear "
        f"an inherited CARGO_NET_OFFLINE; recorded: {offline_modes!r}"
    )


def test_fetch_writes_each_metadata_query_to_its_own_file(
    cargo_stub: tuple[Path, Path],
) -> None:
    """Preparation keeps the full and `--no-deps` metadata outputs apart.

    The two files travel different routes: Nextest reads the full document
    through `--cargo-metadata`, while the coverage driver reads the
    `--no-deps` document through `CARGO_METADATA_CACHE`. Writing the wrong
    query to either path would hand a consumer a silently incomplete or
    over-complete document, so each destination is checked for its own query.
    """
    stub_dir, cargo = cargo_stub
    environment = MakeRunEnvironment(cargo=cargo, locked=None, stub_dir=stub_dir)

    _run_make("fetch", environment, extra_env={"CARGO_NET_OFFLINE": "true"})

    warmup = stub_dir.parent / "workspace" / "target" / "dylint-warmup"
    nextest_metadata = (warmup / "nextest-cargo-metadata.json").read_text(
        encoding="utf-8"
    )
    coverage_metadata = (warmup / "llvm-cov-cargo-metadata.json").read_text(
        encoding="utf-8"
    )

    assert '"resolve"' in nextest_metadata, (
        "the Nextest file must hold the full metadata query"
    )
    assert '"resolve"' not in coverage_metadata, (
        "the coverage file must hold the --no-deps query"
    )
    assert nextest_metadata != coverage_metadata, (
        "the two preparation queries must not be interchangeable"
    )


def test_dylint_smoke_compares_the_driver_digest_after_each_offline_run() -> None:
    """Both offline smoke runs must prove the driver was reused, not rebuilt.

    The cold run builds the driver into an emptied slot; each later offline run
    must be followed by a digest comparison. Dropping either comparison would
    let a rebuild pass as a reuse, which is the property the target exists to
    establish.
    """
    recipe = "\n".join(_makefile_recipe_lines("dylint-smoke"))

    assert recipe.count('[ "$$(digest)" = "$$cold" ]') == 2, (
        "each offline smoke run must compare the driver digest with the cold build"
    )
    assert recipe.count("smoke;") == 2, (
        "the smoke target must run the offline suite twice"
    )


@pytest.mark.parametrize(
    ("locked", "should_use_locked"),
    [(None, True), ("", False), ("--locked", True)],
)
def test_publish_check_forwards_cargo_locked_to_every_invocation(
    tmp_path: Path, locked: str | None, should_use_locked: bool
) -> None:
    """Every lock-relevant Cargo call in `publish-check` honours the lock mode."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    cargo = _write_publish_check_cargo_stub(stub_dir)
    _write_tool_stubs(stub_dir)

    recorded = _run_make(
        "publish-check",
        MakeRunEnvironment(cargo=cargo, locked=locked, stub_dir=stub_dir),
        extra_make_args=[
            "LINT_CRATES=bumpy_road_function",
            "PUBLISH_PACKAGES=whitaker-common",
        ],
    )

    lock_relevant = [line for line in recorded if _is_lock_relevant_invocation(line)]
    heads = [_cargo_head(line) for line in lock_relevant]
    # publish-check must exercise the workspace build, a per-lint build, and
    # packaging — not just a single invocation. It deliberately runs no tests:
    # the coverage lane is the single execution of the suite per pull request.
    assert heads.count("build") >= 2, (
        f"publish-check should run the workspace and per-lint builds; recorded: {recorded!r}"
    )
    assert "nextest run" not in heads, (
        f"publish-check must not re-run the suite the coverage lane executed; recorded: {recorded!r}"
    )
    assert "package" in heads, f"publish-check should package crates; recorded: {recorded!r}"
    for line in lock_relevant:
        assert _has_locked_flag(line) is should_use_locked, (
            f"publish-check must use {_make_log_name(MakeRunEnvironment(cargo, locked, stub_dir))} "
            f"mode; invocation: {line!r}"
        )


def test_publish_check_uses_staged_installer_gate(tmp_path: Path) -> None:
    """Route installer packaging through the same checked MSRV boundary."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    cargo = _write_publish_check_cargo_stub(stub_dir)
    _write_tool_stubs(stub_dir)
    _write_stub(
        stub_dir,
        "python3",
        'printf "python3:%s:CARGO=%s\\n" "$*" "$CARGO" >> "$CARGO_LOCKED_LOG"',
    )

    recorded = _run_make(
        "publish-check",
        MakeRunEnvironment(cargo=cargo, locked="", stub_dir=stub_dir),
        extra_make_args=[
            "LINT_CRATES=bumpy_road_function",
            "PUBLISH_PACKAGES=whitaker-installer",
        ],
    )

    script_call = (
        "python3:scripts/check_installer_msrv.py:"
        f"CARGO={cargo}"
    )
    assert script_call in recorded, recorded
    assert not any(
        "package" in line and "-p whitaker-installer" in line
        for line in recorded
    ), recorded


@pytest.fixture
def cargo_stub(tmp_path: Path) -> tuple[Path, Path]:
    """Create the stub ``bin`` directory with a Cargo stand-in.

    Returns the ``(stub_dir, cargo)`` pair shared by the Makefile tests.
    """
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    cargo = _write_cargo_stub(stub_dir)
    return stub_dir, cargo


@pytest.mark.parametrize(
    ("locked", "should_use_locked"),
    [(None, True), ("", False), ("--locked", True)],
)
def test_representative_targets_forward_cargo_locked(
    cargo_stub: tuple[Path, Path], locked: str | None, should_use_locked: bool
) -> None:
    """Ordinary Cargo targets include the requested lock mode and no other one."""
    stub_dir, cargo = cargo_stub
    environment = MakeRunEnvironment(cargo=cargo, locked=locked, stub_dir=stub_dir)

    recorded = [
        invocation
        for target in ("typecheck", "lint")
        for invocation in _run_make(target, environment)
    ]

    assert recorded, (
        f"typecheck/lint should invoke Cargo in {'locked' if locked else 'unlocked'} mode; "
        f"recorded invocations: {recorded!r}"
    )
    assert all(_has_locked_flag(invocation) is should_use_locked for invocation in recorded), (
        f"typecheck/lint should use {locked or 'unlocked'} mode; "
        f"recorded invocations: {recorded!r}"
    )


@pytest.mark.parametrize("locked", ["", "--locked"])
def test_release_dry_run_forwards_cargo_locked_to_metadata_and_builds(
    cargo_stub: tuple[Path, Path], locked: str
) -> None:
    """Installer metadata and both builds share the caller's lock mode."""
    stub_dir, cargo = cargo_stub
    _write_stub(stub_dir, "rustc", 'echo "host: fake-host"')
    _write_stub(stub_dir, "jq", "echo 0.2.5")

    recorded = _run_make(
        "release-installer-dry-run",
        MakeRunEnvironment(cargo=cargo, locked=locked, stub_dir=stub_dir),
    )

    assert len(recorded) == 3, (
        "release-installer-dry-run should record metadata plus two builds in "
        f"{'locked' if locked else 'unlocked'} mode; recorded invocations: {recorded!r}"
    )
    assert recorded[0].startswith("metadata "), (
        "release-installer-dry-run should record metadata first in "
        f"{'locked' if locked else 'unlocked'} mode; recorded invocations: {recorded!r}"
    )
    assert all(_has_locked_flag(invocation) == bool(locked) for invocation in recorded), (
        f"release-installer-dry-run should use {locked or 'unlocked'} mode; "
        f"recorded invocations: {recorded!r}"
    )
