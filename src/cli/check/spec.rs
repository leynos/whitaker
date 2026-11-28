//! Cargo package-ID specification matching for the coverage listing.
//!
//! `-p` and `--exclude` accept Cargo's package-ID specifications: a bare name,
//! `name@version` with a full or partial version, a glob pattern, or a
//! `source#fragment` form whose source is a path URL. The listing resolves
//! those forms against the workspace members it read from `cargo metadata`;
//! Cargo remains the authority, so a form this resolver cannot express leaves
//! the listing unavailable rather than printing a guessed set.
//!
//! The supported forms are documented in Cargo's package-ID specification
//! reference. Registry sources cannot name a workspace member, and percent
//! escapes in a path URL are compared literally.

use super::glob;
use crate::ports::{PackageFacts, PackageSpec};

/// A version requirement taken from a package-ID specification.
///
/// Cargo accepts a partial version (`1`, `1.2`) as well as a complete one, so
/// the text is compared component-wise rather than parsed as a full `SemVer`
/// requirement.
#[derive(Debug, Clone, Copy)]
struct VersionSpec<'a>(&'a str);

impl<'a> VersionSpec<'a> {
    /// Wrap the version text from a specification.
    const fn new(text: &'a str) -> Self { Self(text) }

    /// Borrow the version text.
    const fn as_str(self) -> &'a str { self.0 }
}

/// Report whether a selector names a package.
///
/// # Examples
///
/// ```
/// use whitaker::{
///     cli::check::spec::matches,
///     ports::{PackageFacts, PackageSpec},
/// };
///
/// let package = PackageFacts {
///     name: "app".to_owned(),
///     version: "0.1.0".to_owned(),
///     manifest_path: "/work/app/Cargo.toml".into(),
///     targets: Vec::new(),
/// };
/// assert!(matches(&PackageSpec::from("app"), &package));
/// assert!(matches(&PackageSpec::from("app@0.1"), &package));
/// assert!(!matches(&PackageSpec::from("app@0.2"), &package));
/// ```
#[must_use]
pub fn matches(spec: &PackageSpec, package: &PackageFacts) -> bool {
    if glob::is_pattern(spec) {
        return glob::matches(spec, package);
    }
    if let Some((source, fragment)) = split_source(spec) {
        return source_matches(source, package)
            && (fragment.is_empty() || fragment_matches(fragment, package));
    }
    named_matches(spec, package)
}

/// Match the forms built from a package name.
fn named_matches(spec: &PackageSpec, package: &PackageFacts) -> bool {
    let text = spec.as_str();
    if text.starts_with("registry+") || text.starts_with("git+") {
        // A registry or Git package is never a workspace member.
        return false;
    }
    match split_name_version(text) {
        Some((name, version)) => name == package.name && version_matches(version, &package.version),
        None => text == package.name,
    }
}

/// Split `name@version` or `name:version`.
///
/// Cargo accepts both separators in a package-ID specification. The source
/// forms are handled before this point, so a `:` here cannot be a URL scheme.
fn split_name_version(text: &str) -> Option<(&str, VersionSpec<'_>)> {
    text.split_once('@')
        .or_else(|| text.split_once(':'))
        .map(|(name, version)| (name, VersionSpec::new(version)))
}

/// Split a path-sourced specification into its source and fragment.
///
/// `path+file:///work/app#member_a@0.1.0` names the workspace member in that
/// directory. A specification without a fragment names the package at the URL,
/// so the source alone is the constraint.
fn split_source(spec: &PackageSpec) -> Option<(&str, &str)> {
    let text = spec.as_str();
    if !(text.starts_with("path+file://") || text.starts_with("file://")) {
        return None;
    }
    Some(text.split_once('#').unwrap_or((text, "")))
}

/// Match a source URL against the package's directory.
///
/// A path source names the package directory; the manifest path is accepted
/// too, because both spellings occur in handwritten specifications. Percent
/// escapes are compared literally, so an escaped path may fail to resolve and
/// leave the listing unavailable rather than guess.
fn source_matches(source: &str, package: &PackageFacts) -> bool {
    let source_path = source
        .strip_prefix("path+file://")
        .or_else(|| source.strip_prefix("file://"));
    source_path.is_some_and(|path| {
        let trimmed = path.strip_suffix('/').unwrap_or(path);
        trimmed == package.directory().as_str() || trimmed == package.manifest_path.as_str()
    })
}

/// Match the fragment of a source specification.
fn fragment_matches(fragment: &str, package: &PackageFacts) -> bool {
    if fragment.is_empty() {
        return false;
    }
    if let Some((name, version)) = split_name_version(fragment) {
        return name == package.name && version_matches(version, &package.version);
    }
    if fragment == package.name {
        return true;
    }
    // A fragment that starts with a digit is a version (package names start
    // with a letter), so it names the member's version rather than its name.
    fragment.starts_with(|character: char| character.is_ascii_digit())
        && version_matches(VersionSpec::new(fragment), &package.version)
}

/// Report whether a (possibly partial) version matches a package version.
///
/// `1`, `1.2` and `1.2.3` all match `1.2.3`; the comparison is component-wise
/// so a partial specification cannot match a different minor or patch. A
/// pre-release suffix stays attached to its component, so `0.1.0` does not
/// match `0.1.0-alpha`.
fn version_matches(spec: VersionSpec<'_>, version: &str) -> bool {
    let spec_parts: Vec<&str> = spec.as_str().split('.').collect();
    let version_parts: Vec<&str> = version.split('.').collect();
    spec_parts.len() <= 3
        && spec_parts.len() <= version_parts.len()
        && spec_parts
            .iter()
            .zip(version_parts.iter())
            .all(|(wanted, actual)| wanted == actual)
}

#[cfg(test)]
mod tests {
    //! The selector forms Cargo accepts, resolved against one package.

    use rstest::rstest;

    use super::matches;
    use crate::ports::{PackageFacts, PackageSpec, TargetFacts};

    /// One workspace member at `/work/app/member_a`.
    fn package() -> PackageFacts {
        PackageFacts {
            name: "member_a".to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: "/work/app/member_a/Cargo.toml".into(),
            targets: vec![TargetFacts {
                name: "member_a".to_owned(),
                kinds: vec!["lib".to_owned()],
            }],
        }
    }

    /// Match each documented form, and refuse the near misses.
    #[rstest]
    #[case::name("member_a", true)]
    #[case::other_name("member_b", false)]
    #[case::exact_version("member_a@0.1.0", true)]
    #[case::minor("member_a@0.1", true)]
    #[case::colon_version("member_a:0.1.0", true)]
    #[case::colon_minor("member_a:0.1", true)]
    #[case::major("member_a@0", true)]
    #[case::wrong_version("member_a@0.2", false)]
    #[case::pre_release_is_not_partial("member_a@0.1.0-alpha", false)]
    #[case::name_pattern("member_*", true)]
    #[case::bracket_pattern("member_[ab]", true)]
    #[case::pkgid_name_and_version("path+file:///work/app/member_a#member_a@0.1.0", true)]
    #[case::pkgid_version("path+file:///work/app/member_a#0.1.0", true)]
    #[case::pkgid_colon_version("path+file:///work/app/member_a#member_a:0.1.0", true)]
    #[case::pkgid_partial("path+file:///work/app/member_a#0.1", true)]
    #[case::pkgid_name("path+file:///work/app/member_a#member_a", true)]
    #[case::pkgid_directory_only("path+file:///work/app/member_a", true)]
    #[case::pkgid_trailing_slash("path+file:///work/app/member_a/#member_a", true)]
    #[case::pkgid_manifest("file:///work/app/member_a/Cargo.toml", true)]
    #[case::pkgid_wrong_name("path+file:///work/app/member_a#member_b", false)]
    #[case::pkgid_other_directory("path+file:///work/app/member_b#member_a", false)]
    #[case::registry_source("registry+https://example.invalid#member_a@0.1.0", false)]
    fn selector_forms_are_resolved(#[case] spec: &str, #[case] expected: bool) {
        assert_eq!(
            matches(&PackageSpec::from(spec), &package()),
            expected,
            "selector {spec:?}"
        );
    }
}
