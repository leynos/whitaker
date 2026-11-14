//! Staging of the pinned fixture project and the paths that locate it.
//!
//! The fixture inputs live in `tests/data` and are included byte-for-byte
//! because `EXPECTED_SCORES` anchors on their exact line numbers. Writing them
//! and resolving their paths stays separate from invoking the tools, so a
//! fixture change cannot silently move a run's target directory.

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

// Fixture inputs are pinned byte-for-byte because `EXPECTED_SCORES` anchors on
// their exact line numbers.
pub(crate) const FIXTURE_MANIFEST: &str = include_str!("../data/fixture_manifest.toml");
pub(crate) const FIXTURE_CLIPPY_CONFIG: &str = include_str!("../data/fixture_clippy.toml");
pub(crate) const FIXTURE_DYLINT_CONFIG: &str = include_str!("../data/fixture_dylint.toml");
pub(crate) const FIXTURE_SOURCE: &str = include_str!("../data/fixture_lib.rs");

/// Write the fixture project files beneath `root`.
pub(crate) fn write_fixture(root: &Utf8Path) -> anyhow::Result<()> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .context("failed to open the fixture root")?;
    directory
        .create_dir_all("src")
        .context("failed to create the fixture `src` directory")?;
    directory.write("Cargo.toml", FIXTURE_MANIFEST)?;
    directory.write("clippy.toml", FIXTURE_CLIPPY_CONFIG)?;
    directory.write("dylint.toml", FIXTURE_DYLINT_CONFIG)?;
    directory.write("src/lib.rs", FIXTURE_SOURCE)?;
    Ok(())
}

/// Find the 1-based line of the first fixture line containing `needle`.
pub(crate) fn fixture_line(needle: &str) -> anyhow::Result<u32> {
    let index = FIXTURE_SOURCE
        .lines()
        .position(|line| line.contains(needle))
        .with_context(|| format!("fixture source has no line containing `{needle}`"))?;
    u32::try_from(index + 1).context("fixture line number does not fit in `u32`")
}

/// Return a UTF-8 view of a temporary directory's path.
pub(crate) fn utf8_temp_path(directory: &TempDir) -> anyhow::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary path is not UTF-8: {}", path.display()))
}
