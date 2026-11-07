"""Execution contract for the `lint-whitaker` Makefile target.

`make lint` is two gates, and the Whitaker half is the one that fails quietly.
`cargo-dylint` warns and *exits 0* when it discovers no libraries, so a gate
that selects `--all` — or that has lost its package list, its `--all-targets`,
or its warning denial — keeps reporting success while analysing less. Reading
the recipe text cannot catch that: a recipe that replaced `$(WHITAKER)` with
`true` would satisfy every substring assertion in `lint_makefile_test.py`.

This module therefore runs the real target. `make` is invoked with the
repository's actual Makefile and a `WHITAKER` that is a stub script recording
its argv and its inherited `RUSTFLAGS`. That is the only way to observe which
arguments the gate passes, in which order, and with what environment, without
running a real Dylint build.

The stub deliberately fails the test rather than passing it when the recipe is
empty, so a target stripped to a no-op is reported as a missing gate.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# The stub writes one line per observation so both argv and environment can be
# asserted. `$0` is included because a recipe that shells out through a wrapper
# would otherwise look like a direct invocation.
_STUB = """\
#!/bin/sh
printf 'argv %s\\n' "$*" >>"$STUB_LOG"
printf 'rustflags %s\\n' "${RUSTFLAGS-}" >>"$STUB_LOG"
"""

# A silent, succeeding stand-in for the tools `lint`'s other prerequisites run.
# It must *drain* standard input: `skill-frontmatter-lint` pipes an extracted
# manifest into its linter, and a child that exits without reading closes the
# pipe, so the feeding `awk` dies of SIGPIPE and the target fails with 141 even
# though every recipe ran.
_QUIET_STUB = """\
#!/bin/sh
cat >/dev/null
exit 0
"""


def _observed_lines(
    target: str, *, variables: dict[str, str], stood_down: tuple[str, ...] = ()
) -> list[str]:
    """Run `target` with a stub `WHITAKER` and return what the stub observed.

    `variables` are passed as `make` command-line assignments, which override
    every `?=` default, so the surrounding environment cannot mask a broken
    recipe. The returned list is the stub's raw log lines: `argv ...` for the
    argument vector and `rustflags ...` for the environment it inherited.

    `stood_down` names make variables that should resolve to the quiet
    stand-in instead of a real tool. A composed target such as `lint` runs
    prerequisites whose own contracts cover them, and which cannot run here:
    this test pins a minimal `PATH` and a scratch `HOME`, so `uv` and `cargo`
    alike would have to resolve from a directory that does not exist.
    """
    make = shutil.which("make")
    assert make is not None, "the workflow contract tests require GNU make"

    with tempfile.TemporaryDirectory() as scratch:
        directory = Path(scratch)
        stub = directory / "whitaker-stub"
        stub.write_text(_STUB, encoding="utf-8")
        stub.chmod(0o755)
        quiet = directory / "quiet-stub"
        quiet.write_text(_QUIET_STUB, encoding="utf-8")
        quiet.chmod(0o755)
        log = directory / "observed.log"
        log.touch()
        assignments = [
            *(f"{name}={value}" for name, value in variables.items()),
            *(f"{name}={quiet}" for name in stood_down),
        ]

        # A minimal environment: the Makefile prepends `$HOME/.cargo/bin` and
        # friends to PATH itself, and the stub must resolve before any real
        # `whitaker` could. `MAKEFLAGS` is cleared so an outer recursive make
        # cannot inject arguments into this run.
        environment = {
            "PATH": "/usr/bin:/bin",
            "HOME": scratch,
            "STUB_LOG": str(log),
        }
        result = subprocess.run(
            [
                make,
                "--no-print-directory",
                "-C",
                str(REPO_ROOT),
                target,
                f"WHITAKER={stub}",
                *assignments,
            ],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        assert result.returncode == 0, (
            f"`make {target}` must succeed with a stub WHITAKER; it exited "
            f"{result.returncode}:\n{result.stdout}\n{result.stderr}"
        )

        return [line for line in log.read_text(encoding="utf-8").splitlines() if line]


def _observed_argv(lines: list[str]) -> list[str]:
    """Return the arguments the stub received."""
    argvs = [line.removeprefix("argv ") for line in lines if line.startswith("argv ")]
    assert len(argvs) == 1, (
        "lint-whitaker must invoke WHITAKER exactly once per run, so a second "
        f"invocation cannot hide a second, differently-configured gate; saw {argvs!r}"
    )
    return argvs[0].split()


def _observed_rustflags(lines: list[str]) -> str:
    """Return the `RUSTFLAGS` the stub inherited."""
    flags = [line for line in lines if line.startswith("rustflags ")]
    assert len(flags) == 1, f"the stub should observe RUSTFLAGS once; saw {flags!r}"
    return flags[0].removeprefix("rustflags ")


def _contains_run(tokens: list[str], run: list[str]) -> bool:
    """Return whether `run` appears in `tokens` as one contiguous, ordered run.

    Checking each token independently is not the same question. `-p alpha
    --features beta` contains `-p`, `alpha`, and `beta`, yet it has lost the
    second package and forwarded `beta` as a feature, so the suite analyses a
    narrower set than `WHITAKER_PACKAGES` names.
    """
    width = len(run)
    return any(
        tokens[index : index + width] == run
        for index in range(len(tokens) - width + 1)
    )


def test_lint_whitaker_passes_the_named_library_and_package_list() -> None:
    """The executed gate must name `whitaker_suite` and the configured packages.

    `--all` is the flag whose empty-discovery case exits 0, and a dropped
    `$(WHITAKER_PACKAGES)` silently narrows what the suite analyses. Neither is
    visible in the recipe text once the target is executed: this asserts the
    argv the stub actually receives.
    """
    argv = _observed_argv(
        _observed_lines(
            "lint-whitaker",
            variables={
                "WHITAKER_PACKAGES": "-p alpha -p beta",
                "RUST_FLAGS": "-D warnings",
            },
        )
    )

    assert "--lib" in argv, f"lint-whitaker must select a library; got {argv!r}"
    library = argv[argv.index("--lib") + 1]
    assert library == "whitaker_suite", (
        "lint-whitaker must name the aggregate library, because an empty disco"
        f"very under --all is a warning and exit 0; got {library!r}"
    )
    assert "--all" not in argv, (
        f"lint-whitaker must not fall back to --all; got {argv!r}"
    )

    # Everything after the separator is the argument list handed to the
    # analysed packages. Splitting there keeps a stray `-p` before it from
    # being mistaken for a package selection.
    assert "--" in argv, f"lint-whitaker must separate the package list; got {argv!r}"
    separator = argv.index("--")
    after = argv[separator + 1 :]

    # The pairs must appear as one contiguous, ordered run. Checking each
    # token on its own would pass for `-p alpha --features beta`, where the
    # second `-p` has been dropped and `beta` is no longer a package, so the
    # suite would analyse less than the variable names.
    expected = ["-p", "alpha", "-p", "beta"]
    assert _contains_run(after, expected), (
        "lint-whitaker must forward WHITAKER_PACKAGES verbatim after `--`, "
        f"so the analysed set stays configurable; missing {expected!r} in "
        f"{after!r}"
    )


def test_lint_whitaker_analyses_every_target_with_every_feature() -> None:
    """The suite must see `--all-targets` and `--all-features` after `--`.

    Dropping either narrows the analysed set to the default lib/bin targets or
    to the default feature set, and the gate still exits 0. The task and
    integration tests that this PR touches are exactly what those flags reach.
    """
    argv = _observed_argv(
        _observed_lines(
            "lint-whitaker",
            variables={"RUST_FLAGS": "-D warnings"},
        )
    )

    after = argv[argv.index("--") + 1 :] if "--" in argv else []
    for required in ("--all-targets", "--all-features"):
        assert required in after, (
            f"lint-whitaker must pass {required!r} to the analysed packages, or "
            f"the gate silently covers less; got {after!r}"
        )


def test_lint_whitaker_denies_warnings_through_the_environment() -> None:
    """`RUSTFLAGS` must carry the caller's warning denial into the suite.

    The denial is what makes a Whitaker diagnostic fail the gate rather than
    print. Setting it only in the Makefile's own variable is not enough if the
    recipe forgets to export it for the child process, so this reads the
    environment the stub actually inherited.
    """
    flags = _observed_rustflags(
        _observed_lines(
            "lint-whitaker",
            variables={"RUST_FLAGS": "-D warnings"},
        )
    )

    assert "-D warnings" in flags, (
        "lint-whitaker must export RUST_FLAGS to the Whitaker process, or "
        f"suite diagnostics only print; the stub inherited {flags!r}"
    )


def test_lint_whitaker_honours_a_reconfigured_package_list() -> None:
    """A changed `WHITAKER_PACKAGES` must reach the suite unchanged.

    The package set is a reviewed list rather than a constant, and a recipe
    that hard-coded yesterday's crates would keep passing every other
    assertion. Passing a different value proves the variable is what is used.
    """
    argv = _observed_argv(
        _observed_lines(
            "lint-whitaker",
            variables={"WHITAKER_PACKAGES": "-p gamma", "RUST_FLAGS": "-D warnings"},
        )
    )

    after = argv[argv.index("--") + 1 :] if "--" in argv else []
    assert _contains_run(after, ["-p", "gamma"]), (
        f"a reconfigured package list must reach the suite; got {after!r}"
    )
    for stale in ("alpha", "beta"):
        assert stale not in after, (
            f"lint-whitaker must not add packages of its own; found {stale!r} "
            f"in {after!r}"
        )


def test_lint_runs_the_whitaker_gate_it_declares() -> None:
    """`make lint` must actually reach the Whitaker half, not merely name it.

    The prerequisite line is checked by the text contract, but only an executed
    run shows that the dependency is real. A `lint` recipe that shadowed its
    prerequisites would leave the suite unrun while the declaration still read
    correctly.

    `CARGO` and `UV` are stood down because their halves of `lint` are
    covered by their own contracts, and neither can run here: this test pins a
    minimal `PATH` and a scratch `HOME`, and `uv` resolves from `~/.local/bin`
    on the host and in CI alike. Leaving them live would make the assertion
    depend on the machine rather than on the recipe, and would fail before
    reaching the gate under test.
    """
    lines = _observed_lines(
        "lint",
        variables={
            "RUSTDOC_FLAGS": "-D warnings",
            "RUST_FLAGS": "-D warnings",
        },
        stood_down=("CARGO", "UV"),
    )

    argv = _observed_argv(lines)
    assert "--lib" in argv and "whitaker_suite" in argv, (
        "`make lint` must reach lint-whitaker, which selects the aggregate "
        f"library; the stub saw {argv!r}"
    )
