"""Give a copied Makefile the Clippy snapshot its test recipes require.

`make test`, `make coverage`, and `make publish-check` depend on
`make clippy-snapshot`, which reads the pinned commit from
`installer/clippy-snapshot.toml` and requires the generation it names to carry
its `whitaker-clippy-snapshot.json` metadata. Harnesses that copy the Makefile
into a scratch workspace and stub Cargo cannot install a real snapshot, so
they provide the manifest and a pre-built generation instead.

Examples
--------
>>> import tempfile
>>> from pathlib import Path
>>> with tempfile.TemporaryDirectory() as scratch:
...     arguments = provide_clippy_snapshot(Path(scratch))
...     arguments[0].startswith("CLIPPY_SNAPSHOT_ROOT=")
True
"""

from __future__ import annotations

import shutil
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MANIFEST = REPOSITORY_ROOT / "installer" / "clippy-snapshot.toml"


def pinned_commit() -> str:
    """Return the Clippy commit the committed manifest pins.

    Returns
    -------
    str
        The full commit object ID.
    """
    for line in MANIFEST.read_text(encoding="utf-8").splitlines():
        if line.startswith("commit = "):
            return line.split('"')[1]
    msg = f"{MANIFEST} pins no commit"
    raise ValueError(msg)


def provide_clippy_snapshot(workspace: Path) -> list[str]:
    """Copy the manifest into `workspace` and pre-build the pinned generation.

    Parameters
    ----------
    workspace
        The scratch directory holding the copied Makefile.

    Returns
    -------
    list[str]
        `make` variable assignments pointing the recipes at the generation and
        at a scratch driver root.
    """
    installer = workspace / "installer"
    installer.mkdir(parents=True, exist_ok=True)
    shutil.copy2(MANIFEST, installer / "clippy-snapshot.toml")
    root = workspace / "clippy-snapshot"
    generation = root / pinned_commit()
    generation.mkdir(parents=True, exist_ok=True)
    (generation / "whitaker-clippy-snapshot.json").write_text("{}\n", encoding="utf-8")
    return [
        f"CLIPPY_SNAPSHOT_ROOT={root}",
        f"DYLINT_DRIVER_ROOT={workspace / 'dylint-drivers'}",
    ]
