"""Contracts for the dev-build footprint that concordat rule 0.2.1 sets.

Rule 0.2.1 keeps developer and CI build trees small. The `dev` profile in
`Cargo.toml` turns off incremental compilation and debug information, and no
override table, nor any `[profile.dev]` in `.cargo/config.toml`, may switch
either one back on.

Cargo's native `[unstable]` embed-metadata key is the only route to the
metadata behaviour. Raw rustc metadata flags in `rustflags` broke build
scripts, so any flag mentioning `metadata` is refused outright. The key that
the pinned nightly needs depends on the pin's date, so the pin selects it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import tomllib
import typing as typ
from datetime import date
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
#: Inclusive date windows of the `[unstable]` embed-metadata key.
OLD_KEY_FROM = date(2025, 6, 26)
OLD_KEY_THROUGH = date(2026, 7, 18)
UNPROVEN_FROM = date(2026, 7, 19)
UNPROVEN_THROUGH = date(2026, 7, 30)
NEW_KEY_FROM = date(2026, 7, 31)
#: Window labels returned by `classify_channel`.
OLD_KEY = "old-key"
NEW_KEY = "new-key"
UNPROVEN = "unproven"
UNCONSTRAINED = "unconstrained"
UNKNOWN = "unknown"
DATED_NIGHTLY = re.compile(r"nightly-\d{4}-\d{2}-\d{2}")

Table = dict[str, typ.Any]


def classify_channel(channel: str) -> str:
    """Return the window that a toolchain channel falls in.

    Only dated nightlies carry an embed-metadata requirement, so any other
    channel is unconstrained. A dated nightly outside the three windows, or a
    date that does not exist, is unknown, so the contract fails rather than
    passes.
    """
    if DATED_NIGHTLY.fullmatch(channel) is None:
        return UNCONSTRAINED
    try:
        day = date.fromisoformat(channel.removeprefix("nightly-"))
    except ValueError:
        return UNKNOWN
    if OLD_KEY_FROM <= day <= OLD_KEY_THROUGH:
        return OLD_KEY
    if UNPROVEN_FROM <= day <= UNPROVEN_THROUGH:
        return UNPROVEN
    if day >= NEW_KEY_FROM:
        return NEW_KEY
    return UNKNOWN


def _load(name: str) -> Table:
    """Parse a committed TOML file from the repository root."""
    with (ROOT / name).open("rb") as handle:
        return tomllib.load(handle)


def _table(data: Table, *path: str) -> Table:
    """Return the nested table at `path`, or an empty table if absent."""
    node: typ.Any = data
    for key in path:
        node = node.get(key, {})
    return node


def _dev_tables(dev: Table) -> list[tuple[str, Table]]:
    """Return `profile.dev` and its nested override tables, named."""
    tables = [("dev", dev)]
    tables += [
        (f"dev.package.{name}", table)
        for name, table in _table(dev, "package").items()
    ]
    tables.append(("dev.build-override", _table(dev, "build-override")))
    return tables


def _footprint_leaks(dev: Table) -> list[str]:
    """Return the dev-profile tables that re-enable incremental or debug."""
    leaks = []
    for name, table in _dev_tables(dev):
        if table.get("incremental") is True:
            leaks.append(f"[profile.{name}] sets incremental = true")
        if table.get("debug", 0) not in (0, False):
            leaks.append(f"[profile.{name}] sets debug = {table['debug']!r}")
    return leaks


def _rustflag_tables(cfg: Table) -> list[tuple[str, Table]]:
    """Return the `[build]` table and every `[target.*]` table, named."""
    tables = [("build", _table(cfg, "build"))]
    tables += [
        (f"target.{name}", table) for name, table in _table(cfg, "target").items()
    ]
    return tables


def _rustflags(table: Table) -> list[str]:
    """Return a table's `rustflags` entries as strings, whatever their shape."""
    raw = table.get("rustflags", [])
    if isinstance(raw, str):
        return [raw]
    return [str(flag) for flag in raw]


def test_the_dev_profile_turns_incremental_and_debug_off() -> None:
    """Commit `[profile.dev]` with incremental compilation and debug info off."""
    dev = _table(_load("Cargo.toml"), "profile", "dev")
    assert dev.get("incremental") is False, "[profile.dev] must set incremental = false"
    assert dev.get("debug") == 0, "[profile.dev] must set debug = 0"


def test_no_cargo_toml_dev_override_reenables_the_footprint() -> None:
    """No `Cargo.toml` package or build-override table may re-enable either."""
    dev = _table(_load("Cargo.toml"), "profile", "dev")
    leaks = _footprint_leaks(dev)
    assert leaks == [], "; ".join(leaks)


def test_no_cargo_config_dev_profile_reenables_the_footprint() -> None:
    """No `.cargo/config.toml` `[profile.dev]` table may re-enable either."""
    dev = _table(_load(".cargo/config.toml"), "profile", "dev")
    leaks = _footprint_leaks(dev)
    assert leaks == [], "; ".join(leaks)


def test_no_rustflags_pass_raw_rustc_metadata_flags() -> None:
    """Refuse raw metadata flags in `rustflags`; Cargo's native key is the route."""
    cfg = _load(".cargo/config.toml")
    for name, table in _rustflag_tables(cfg):
        for flag in _rustflags(table):
            assert "metadata" not in flag, (
                f"[{name}] rustflags pass {flag!r}: use Cargo's native "
                "[unstable] embed-metadata setting instead of raw rustc flags"
            )


def test_the_unstable_table_matches_the_pinned_window() -> None:
    """Pick the `[unstable]` embed-metadata key that the pinned date requires."""
    toolchain = _table(_load("rust-toolchain.toml"), "toolchain")
    channel = str(toolchain.get("channel", ""))
    window = classify_channel(channel)
    assert window != UNPROVEN, (
        f"pin {channel} is in the unproven window 2026-07-19 through "
        "2026-07-30; its [unstable] key is not yet proven"
    )
    assert window != UNKNOWN, f"pin {channel!r} is outside the classified windows"
    unstable = _table(_load(".cargo/config.toml"), "unstable")
    if window == OLD_KEY:
        assert unstable.get("no-embed-metadata") is True, (
            "pins through 2026-07-18 need [unstable] no-embed-metadata = true"
        )
        assert "embed-metadata" not in unstable, (
            "pins through 2026-07-18 must not set [unstable] embed-metadata"
        )
    if window == NEW_KEY:
        assert unstable.get("embed-metadata") is False, (
            "pins from 2026-07-31 need [unstable] embed-metadata = false"
        )
        assert "no-embed-metadata" not in unstable, (
            "pins from 2026-07-31 must not set [unstable] no-embed-metadata"
        )


@pytest.mark.parametrize(
    ("channel", "window"),
    [
        ("nightly-2025-06-25", UNKNOWN),
        ("nightly-2025-06-26", OLD_KEY),
        ("nightly-2026-05-28", OLD_KEY),
        ("nightly-2026-07-18", OLD_KEY),
        ("nightly-2026-07-19", UNPROVEN),
        ("nightly-2026-07-30", UNPROVEN),
        ("nightly-2026-07-31", NEW_KEY),
        ("nightly-2026-02-30", UNKNOWN),
        ("stable", UNCONSTRAINED),
    ],
)
def test_the_pin_window_classifies_channels(channel: str, window: str) -> None:
    """Classify boundary dates and non-nightly channels into their windows."""
    assert classify_channel(channel) == window
