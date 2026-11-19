//! Read the embedded locale tree and check its file-level shape.
//!
//! The parity gates compare every locale directory under `common/locales/`
//! with the `en-GB` source. This module owns reading that tree through a
//! capability handle, parsing each locale into one catalogue, and the checks
//! that look at whole files rather than parsed patterns.

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use whitaker_common::i18n::locales_root;

use super::{
    inputs::LocaleTag,
    rules::SOURCE_TAG,
    summary::{Catalogue, summarize},
};

/// Bidirectional controls that catalogues must not contain; see the
/// translator's guide, section 8.
pub const BIDI_CONTROLS: [char; 12] = [
    '\u{200e}', '\u{200f}', '\u{061c}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Map each resource file name to its raw text for one locale.
pub type LocaleFiles = BTreeMap<String, String>;

/// Open the locale tree that the static loader embeds.
pub fn open_locales() -> Result<Dir, String> {
    let root = Utf8PathBuf::from_path_buf(locales_root())
        .map_err(|path| format!("locale root is not UTF-8: {}", path.display()))?;
    Dir::open_ambient_dir(&root, ambient_authority())
        .map_err(|error| format!("cannot open {root}: {error}"))
}

/// Read every file in the directory for `tag`.
pub fn read_locale(root: &Dir, tag: LocaleTag<'_>) -> Result<LocaleFiles, String> {
    let directory = root
        .open_dir(tag.as_str())
        .map_err(|error| format!("cannot open locale `{tag}`: {error}"))?;
    let mut files = LocaleFiles::new();
    let entries = directory
        .entries()
        .map_err(|error| format!("cannot list locale `{tag}`: {error}"))?;
    for entry in entries {
        let name = entry
            .and_then(|found| found.file_name())
            .map_err(|error| format!("cannot read an entry of `{tag}`: {error}"))?;
        let text = directory
            .read_to_string(&name)
            .map_err(|error| format!("cannot read `{tag}/{name}`: {error}"))?;
        files.insert(name, text);
    }
    Ok(files)
}

/// List the locale directories present on disk, sorted by tag.
pub fn locale_tags(root: &Dir) -> Result<Vec<String>, String> {
    let entries = root
        .entries()
        .map_err(|error| format!("cannot list the locale root: {error}"))?;
    let mut tags = Vec::new();
    for entry in entries {
        let name = entry
            .and_then(|found| found.file_name())
            .map_err(|error| format!("cannot read a locale root entry: {error}"))?;
        tags.push(name);
    }
    tags.sort_unstable();
    Ok(tags)
}

/// Parse every file of a locale into one catalogue, reporting syntax errors.
pub fn parse_locale(
    tag: LocaleTag<'_>,
    files: &LocaleFiles,
    findings: &mut Vec<String>,
) -> Catalogue {
    let mut catalogue = Catalogue::new();
    for (name, text) in files {
        match summarize(text) {
            Ok(parsed) => catalogue.extend(parsed),
            Err(errors) => findings.push(format!("{tag}/{name}: syntax errors: {errors}")),
        }
    }
    catalogue
}

/// The parsed source catalogue and the file names every locale must match.
pub struct Source {
    /// The parsed `en-GB` catalogue.
    pub catalogue: Catalogue,
    /// The resource file names every locale must also provide.
    pub file_names: BTreeSet<String>,
}

/// Read and parse the source catalogue, recording its own syntax errors.
pub fn read_source(root: &Dir, findings: &mut Vec<String>) -> Result<Source, String> {
    let tag = LocaleTag::new(SOURCE_TAG);
    let files = read_locale(root, tag)?;
    Ok(Source {
        catalogue: parse_locale(tag, &files, findings),
        file_names: files.into_keys().collect(),
    })
}

/// Report a file set that differs from the source, and bidirectional controls.
pub fn file_findings(tag: LocaleTag<'_>, files: &LocaleFiles, source: &Source) -> Vec<String> {
    let names: BTreeSet<String> = files.keys().cloned().collect();
    let file_set = (names != source.file_names)
        .then(|| format!("{tag}: file set differs from {SOURCE_TAG}: {names:?}"));
    let controls = files
        .iter()
        .filter(|(_, text)| text.contains(BIDI_CONTROLS))
        .map(|(name, _)| format!("{tag}/{name}: contains a bidirectional control"));
    file_set.into_iter().chain(controls).collect()
}
