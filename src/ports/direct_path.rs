//! The proven user-managed library path a direct `check` run loads.
//!
//! This is a port value rather than a rule: the rules that decide whether a
//! path names exactly one Whitaker suite live in `src/library_path`, which
//! depends on nothing above it. The type lives here because it crosses the
//! port boundary — `CheckPrepared::Direct` carries one — and a domain module
//! that owns it would have to import the port's error vocabulary to refuse a
//! path, which is the cycle this arrangement avoids.

use std::ffi::{OsStr, OsString};

/// A user-managed library path already shown to name exactly one suite.
///
/// The field is private so that holding one is evidence rather than a claim:
/// the value can only be built from a classification the adapter produced by
/// the same listing the readiness check uses. Without that, a caller could
/// hand `check` a path Dylint cannot load and receive a status
/// indistinguishable from a lint denial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectLibraryPath(OsString);

impl DirectLibraryPath {
    /// Keep `path` after the adapter's listing approved it.
    ///
    /// Crate-visible rather than public: the caller has already judged the
    /// listing and is recording that verdict, so the type must not be
    /// constructible from a path alone.
    #[must_use]
    pub(crate) const fn from_approved(path: OsString) -> Self { Self(path) }

    /// The `DYLINT_LIBRARY_PATH` value to export to the child.
    #[must_use]
    pub fn as_os_str(&self) -> &OsStr { &self.0 }
}
