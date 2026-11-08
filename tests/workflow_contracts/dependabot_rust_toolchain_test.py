"""The toolchain entry keeps Dependabot's bump out of the unattended merge.

`rust-toolchain.toml` is coupled to the `ra_ap_syntax` snapshot, the Dylint and
`clippy_utils` revisions and `installer/clippy-snapshot.toml`, so a bare
Dependabot bump of it does not build and must wait for a person. The shared
automerge workflow merges a green Dependabot pull request that carries its
required label, which defaults to `dependencies`. These tests hold the two
repository-owned facts that keep the bump out of that path: the entry exists
(the user's ruling) and it does not carry the label the automerge requires.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
import yaml
from ubicloud_workflow_support import REPOSITORY_ROOT, load_workflow

CONFIG: typ.Final = REPOSITORY_ROOT / ".github" / "dependabot.yml"

#: The label the shared automerge workflow requires unless the caller overrides
#: `required-label`; `test_the_automerge_caller_keeps_the_default_label` holds it.
AUTOMERGE_LABEL: typ.Final = "dependencies"


def _entries() -> list[dict[str, typ.Any]]:
    """Return the `rust-toolchain` entries of the Dependabot configuration."""
    document = yaml.safe_load(CONFIG.read_text(encoding="utf-8"))
    assert isinstance(document, dict), f"{CONFIG} must parse to a mapping"
    updates = document.get("updates")
    assert isinstance(updates, list), "the configuration must declare `updates`"
    return [e for e in updates if e.get("package-ecosystem") == "rust-toolchain"]


def test_the_toolchain_file_has_one_daily_entry() -> None:
    """Bump the pinned nightly: one entry, at the root, checked daily."""
    entries = _entries()
    assert len(entries) == 1, f"expected one rust-toolchain entry; got {len(entries)}"
    (entry,) = entries
    assert entry.get("directory") == "/", (
        f"the entry must cover the root toolchain file; got {entry.get('directory')!r}"
    )
    interval = entry.get("schedule", {}).get("interval")
    assert interval == "daily", f"the entry must run daily; got {interval!r}"


def test_the_entry_does_not_carry_the_automerge_label() -> None:
    """Keep a toolchain bump out of the automerge, which needs a person's edits."""
    (entry,) = _entries()
    labels = entry.get("labels", [])
    assert AUTOMERGE_LABEL not in labels, (
        f"a toolchain bump must not merge unattended; drop {AUTOMERGE_LABEL!r} "
        f"from the entry's labels {labels}"
    )


def test_the_automerge_caller_keeps_the_default_label() -> None:
    """Fail if the caller changes `required-label`, which would void the check above."""
    workflow = load_workflow("dependabot-automerge.yml")
    for name, job in workflow["jobs"].items():
        overrides = (job.get("with") or {}).get("required-label")
        assert overrides is None, (
            f"job {name} sets required-label to {overrides!r}; the label check "
            "in this module assumes the shared default"
        )
