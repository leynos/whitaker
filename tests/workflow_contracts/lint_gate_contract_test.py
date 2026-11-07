"""Hold the workspace-wide lint policy together as one contract.

The hardening the repository relies on is declared in three places, and
none of them is checked by the compiler:

* the root manifest sets the lint levels under `[workspace.lints.*]`;
* every member manifest opts into them with `[lints] workspace = true`;
* the `lint` recipe runs the documentation and Clippy lanes that apply
  them across `CARGO_FLAGS`.

A member that drops the opt-in keeps compiling under Cargo's default lint
set without any diagnostic, and a narrowed recipe stops checking the
targets it dropped while `make lint` still exits zero. These tests fail
when any of the three drifts, so the gate cannot lose coverage silently.

The one recorded exception is `vendor/dylint_testing`: `cargo vendor`
writes a Cargo-normalized manifest carrying its own inline policy, so it
cannot inherit the workspace table. It is still built as a member and is
therefore still linted, which these tests also pin.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import subprocess  # noqa: S404  # Cargo is the authority on workspace membership.
import tomllib
import typing as typ

from ubicloud_workflow_support import REPOSITORY_ROOT

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from pathlib import Path

CARGO_MANIFEST = REPOSITORY_ROOT / "Cargo.toml"
MAKEFILE = REPOSITORY_ROOT / "Makefile"

#: The vendored crate Cargo patches in, whose manifest cannot inherit.
VENDORED_MEMBER = "vendor/dylint_testing"

#: Rust lints whose loss would restore a behaviour the policy prohibits.
REQUIRED_RUST_LINTS = {
    "unsafe_code": "forbid",
    "missing_docs": "deny",
}

#: Clippy lints that guard the hygiene, panic, and error-handling rules the
#: hardening introduced; dropping any one reopens the pattern it denies.
REQUIRED_CLIPPY_LINTS = {
    "allow_attributes": "deny",
    "allow_attributes_without_reason": "deny",
    "expect_used": "deny",
    "missing_assert_message": "deny",
    "panic_in_result_fn": "deny",
    "unwrap_used": "deny",
}

#: Rustdoc lints that keep the generated documentation buildable.
REQUIRED_RUSTDOC_LINTS = {
    "broken_intra_doc_links": "deny",
    "missing_crate_level_docs": "deny",
}

#: Clippy groups the vendored manifest must keep enabled. `cargo vendor`
#: cannot carry the workspace's individual denials, so these groups are what
#: makes `-D warnings` still fail the crate.
VENDORED_CLIPPY_GROUPS = ("nursery", "pedantic")


def _manifest(path: Path) -> dict[str, typ.Any]:
    """Return one decoded Cargo manifest."""
    with path.open("rb") as handle:
        return tomllib.load(handle)


def _level(entry: object) -> str | None:
    """Return a lint level whether the table spells it out or not."""
    if isinstance(entry, str):
        return entry
    if isinstance(entry, dict):
        level = entry.get("level")
        return level if isinstance(level, str) else None
    return None


def _workspace_lints(group: str) -> dict[str, typ.Any]:
    """Return one `[workspace.lints.<group>]` table from the root manifest."""
    table = _manifest(CARGO_MANIFEST)["workspace"]["lints"].get(group)
    assert isinstance(table, dict), f"[workspace.lints.{group}] must exist"
    return table


def _resolved_member_manifests() -> list[Path]:
    """Return the manifest of every member Cargo resolves, not just the globs.

    Cargo also makes an in-workspace path dependency a member when no
    `members` pattern names it, so expanding the patterns alone would miss a
    crate the workspace still builds and lints.
    """
    cargo = os.environ.get("CARGO", "cargo")
    result = subprocess.run(  # noqa: S603  # Fixed argument list, no shell.
        [cargo, "metadata", "--no-deps", "--format-version", "1"],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        check=False,
        text=True,
    )
    assert result.returncode == 0, (
        f"cargo metadata must describe the workspace; got {result.stderr!r}"
    )
    metadata = json.loads(result.stdout)
    return [
        pathlib.Path(package["manifest_path"]) for package in metadata["packages"]
    ]


def _member_manifests() -> dict[Path, dict[str, typ.Any]]:
    """Return every resolved workspace member's decoded manifest, keyed by path."""
    manifests = {CARGO_MANIFEST: _manifest(CARGO_MANIFEST)}
    for path in _resolved_member_manifests():
        if path != CARGO_MANIFEST:
            manifests[path] = _manifest(path)
    return manifests


def _relative(path: Path) -> str:
    """Return a path relative to the repository root, POSIX-separated."""
    return path.relative_to(REPOSITORY_ROOT).as_posix()


def _makefile_variables() -> dict[str, str]:
    """Return every `?=` variable default in the Makefile, keyed by name."""
    text = MAKEFILE.read_text(encoding="utf-8")
    return dict(re.findall(r"^([A-Z_][A-Z0-9_]*) \?= (.*)$", text, re.MULTILINE))


def _recipe_commands(target: str) -> list[str]:
    """Return the shell commands in one Make target's recipe.

    Make-level comments and blank lines are skipped, and recipe comments
    are dropped, so a comment naming a flag cannot satisfy a check meant
    to catch the command that carries it.
    """
    lines = MAKEFILE.read_text(encoding="utf-8").splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert start is not None, f"{target} must be defined in the Makefile"
    commands: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("\t"):
            command = line.strip().lstrip("@-+").strip()
            if command and not command.startswith("#"):
                commands.append(command)
            continue
        if line.strip() and not line.startswith("#"):
            break
    return commands


def _only_command(commands: list[str], marker: str) -> str:
    """Return the one recipe command naming `marker`."""
    matching = [command for command in commands if marker in command]
    assert len(matching) == 1, (
        f"exactly one lint command must name {marker!r}, got {matching}"
    )
    return matching[0]


def test_the_rust_policy_forbids_unsafe_code_and_requires_docs() -> None:
    """These two levels are why the crate roots carry `//!` modules."""
    rust = _workspace_lints("rust")
    for lint, expected in REQUIRED_RUST_LINTS.items():
        assert _level(rust.get(lint)) == expected, (
            f"[workspace.lints.rust] must set {lint} = {expected!r}, "
            f"got {rust.get(lint)!r}"
        )


def test_the_clippy_policy_denies_the_hardened_lints() -> None:
    """A member cannot restore a denied pattern without an explicit allow."""
    clippy = _workspace_lints("clippy")
    for lint, expected in REQUIRED_CLIPPY_LINTS.items():
        assert _level(clippy.get(lint)) == expected, (
            f"[workspace.lints.clippy] must set {lint} = {expected!r}, "
            f"got {clippy.get(lint)!r}"
        )


def test_the_clippy_policy_keeps_pedantic_enabled() -> None:
    """The pedantic group supplies most of the individual denials."""
    pedantic = _workspace_lints("clippy").get("pedantic")
    assert _level(pedantic) == "warn", "the pedantic group must stay enabled"


def test_the_rustdoc_policy_denies_unbuildable_documentation() -> None:
    """Rustdoc warnings are denied by `RUSTDOC_FLAGS`, not by this table."""
    rustdoc = _workspace_lints("rustdoc")
    for lint, expected in REQUIRED_RUSTDOC_LINTS.items():
        assert _level(rustdoc.get(lint)) == expected, (
            f"[workspace.lints.rustdoc] must set {lint} = {expected!r}, "
            f"got {rustdoc.get(lint)!r}"
        )


def test_every_member_inherits_the_workspace_policy() -> None:
    """A member without the opt-in is linted by Cargo's defaults instead."""
    offenders = sorted(
        _relative(path)
        for path, manifest in _member_manifests().items()
        if path.relative_to(REPOSITORY_ROOT).parts[0] != "vendor"
        and manifest.get("lints") != {"workspace": True}
    )
    assert not offenders, (
        "every non-vendored workspace member must declare "
        f"[lints] workspace = true; missing from {offenders}"
    )


def test_the_vendored_crate_is_linted_under_its_own_policy() -> None:
    """`cargo vendor` normalizes the manifest, so inheritance is impossible.

    The crate still has to stay a member: as a member it is built and
    linted with the inline `[lints.*]` tables `cargo vendor` wrote.
    """
    root = _manifest(CARGO_MANIFEST)
    assert VENDORED_MEMBER in root["workspace"]["members"], (
        f"{VENDORED_MEMBER} must stay a workspace member to remain linted"
    )
    lints = _manifest(REPOSITORY_ROOT / VENDORED_MEMBER / "Cargo.toml").get("lints")
    assert isinstance(lints, dict), (
        "the vendored crate must carry its own [lints.*] tables"
    )
    assert lints, "the vendored crate's [lints.*] tables must not be empty"
    assert "workspace" not in lints, (
        "a normalized vendored manifest cannot inherit the workspace table"
    )
    clippy = lints.get("clippy")
    assert isinstance(clippy, dict), "the vendored crate must carry [lints.clippy]"
    for group in VENDORED_CLIPPY_GROUPS:
        assert _level(clippy.get(group)) == "warn", (
            f"the vendored crate must keep the {group} group at warn so "
            f"-D warnings still fails it; got {clippy.get(group)!r}"
        )
    rust = lints.get("rust")
    assert isinstance(rust, dict), "the vendored crate must carry [lints.rust]"
    assert _level(rust.get("unexpected_cfgs")) == "deny", (
        "the vendored crate must keep its unexpected_cfgs denial; "
        f"got {rust.get('unexpected_cfgs')!r}"
    )


def test_the_gate_flags_cover_the_whole_workspace() -> None:
    """Each flag keeps one dimension of the executed set from narrowing."""
    flags = _makefile_variables()["CARGO_FLAGS"]
    for flag in ("--workspace", "--all-targets", "--all-features"):
        assert flag in flags, f"CARGO_FLAGS must keep {flag}; got {flags!r}"


def test_both_gates_deny_warnings() -> None:
    """A warning that does not fail the gate is a warning nobody fixes."""
    variables = _makefile_variables()
    for name in ("RUST_FLAGS", "RUSTDOC_FLAGS"):
        words = variables[name].split()
        assert words[-2:] == ["-D", "warnings"], (
            f"{name} must end with -D warnings; got {variables[name]!r}"
        )


def test_lint_documents_the_workspace_before_linting_it() -> None:
    """Rustdoc warnings are denied through `RUSTDOCFLAGS`, not `RUST_FLAGS`."""
    command = _only_command(_recipe_commands("lint"), "$(CARGO) doc")
    assert command.startswith('RUSTDOCFLAGS="$(RUSTDOC_FLAGS)"'), (
        f"the doc lane must pass RUSTDOCFLAGS; got {command!r}"
    )
    for fragment in ("$(CARGO_LOCKED)", "--workspace", "--no-deps"):
        assert fragment in command, f"the doc lane must keep {fragment}; got {command!r}"


def test_lint_checks_every_target_with_every_feature() -> None:
    """The Clippy lane must apply the same executed set as the rest."""
    command = _only_command(_recipe_commands("lint"), "clippy")
    expected = "$(CARGO) clippy $(CARGO_LOCKED) $(CARGO_FLAGS) -- $(RUST_FLAGS)"
    assert command == expected, (
        f"the Clippy lane must run {expected!r}, got {command!r}"
    )


def test_lint_keeps_the_skill_manifest_gate_as_a_prerequisite() -> None:
    """A detached prerequisite would leave `make lint` checking no manifest."""
    text = MAKEFILE.read_text(encoding="utf-8")
    declaration = next(
        (line for line in text.splitlines() if line.startswith("lint:")),
        None,
    )
    assert declaration is not None, "the Makefile must define a lint target"
    assert "skill-manifest-check" in declaration, (
        f"lint must depend on skill-manifest-check; got {declaration!r}"
    )
