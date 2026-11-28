//! Differential tests that `whitaker check` selects exactly what `cargo clippy`
//! selects.
//!
//! Each case copies a fixture workspace, runs `cargo clippy` with the flags
//! under test, runs `whitaker check` with the same flags through a bootstrap
//! whose stub delegates the lint run to a real `cargo check`, and compares the
//! compiled units. The comparison is over Cargo's own `compiler-artifact`
//! messages — package id, target name and target kinds — not over text, so a
//! difference in ordering or spelling cannot hide a coverage difference.
//!
//! The `-v` listing is compared against the same artefact set: the packages
//! Whitaker says it will check must be the packages Cargo did check, which is
//! what makes the listing trustworthy as a coverage display.

#![cfg(unix)]

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use rstest::rstest;

mod e2e_check_selection_support;
mod e2e_check_support;

use e2e_check_selection_support::{SelectionFixture, WhitakerRun, scratch_directory};

/// One differential case: a fixture, the directory to run in, and the flags.
#[derive(Debug, Clone, Copy)]
struct Case<'a> {
    /// The fixture directory name under `tests/data/cargo_selection`.
    fixture: &'a str,
    /// The directory to run in, relative to the fixture root.
    cwd: &'a str,
    /// The selection flags given to both commands.
    flags: &'a [&'a str],
}

/// Where a run happens and what it carries.
struct RunRequest<'a> {
    /// The directory both commands run in.
    cwd: &'a Path,
    /// The selection flags given to both commands.
    flags: &'a [&'a str],
    /// The manifest the metadata query reads, when the run is outside the
    /// workspace and cannot inherit it from the directory.
    manifest: Option<&'a str>,
}

/// Everything one differential case observed.
struct Observation {
    /// The units `cargo clippy` compiled, as comparison keys.
    clippy: Vec<String>,
    /// The Whitaker run and its artefacts and listing.
    run: WhitakerRun,
    /// Cargo's package ids mapped to package names.
    names: BTreeMap<String, String>,
}

/// Run both commands for `case` and collect their evidence.
///
/// # Errors
///
/// Returns a description of the first fixture, spawn or message-parsing step
/// that failed.
fn observe(case: Case<'_>) -> Result<Observation, String> {
    let fixture = SelectionFixture::new(case.fixture)?;
    let cwd = fixture.cwd(case.cwd);
    observe_in(
        &fixture,
        &RunRequest {
            cwd: &cwd,
            flags: case.flags,
            manifest: None,
        },
    )
}

/// Run both commands for `request` and collect their evidence.
fn observe_in(fixture: &SelectionFixture, request: &RunRequest<'_>) -> Result<Observation, String> {
    let clippy = fixture.clippy(request.cwd, request.flags)?;
    let run = SelectionFixture::whitaker(request.cwd, request.flags)?;
    let names = fixture.package_names(request.cwd, request.manifest)?;
    Ok(Observation {
        clippy: clippy.keys().iter().cloned().collect(),
        run,
        names,
    })
}

/// Require the observation to be non-vacuous and to match Clippy exactly.
///
/// The non-vacuity checks are part of the assertion: a fixture selection that
/// compiled nothing, or a listing that printed nothing, would let both
/// comparisons pass without evidence.
fn assert_matches(case: Case<'_>, observed: &Observation) {
    assert!(
        !observed.clippy.is_empty(),
        "the reference run must compile something for {} {:?}, or the comparison is vacuous",
        case.fixture,
        case.flags
    );
    assert!(
        !observed.run.listed.is_empty(),
        "the -v listing must be non-empty for {} {:?}, or its comparison is vacuous",
        case.fixture,
        case.flags
    );

    let whitaker: Vec<String> = observed.run.artefacts.keys().iter().cloned().collect();
    assert_eq!(
        whitaker, observed.clippy,
        "whitaker check {:?} in fixture {} ({}) must compile the same units as cargo clippy with \
         the same flags",
        case.flags, case.fixture, case.cwd
    );

    let mut checked = BTreeSet::new();
    let mut unknown = Vec::new();
    for id in observed.run.artefacts.package_ids() {
        match observed.names.get(&id) {
            Some(name) => {
                checked.insert(name.clone());
            }
            None => unknown.push(id),
        }
    }
    assert!(
        unknown.is_empty(),
        "every compiled package must appear in cargo metadata, missing {unknown:?}"
    );
    assert_eq!(
        observed.run.listed, checked,
        "the -v listing for {} {:?} must name exactly the packages Cargo checked",
        case.fixture, case.flags
    );
}

/// Build a scenario for `fixture` at `cwd` with `flags`.
const fn scenario<'a>(fixture: &'a str, cwd: &'a str, flags: &'a [&'a str]) -> Case<'a> {
    Case {
        fixture,
        cwd,
        flags,
    }
}

/// A single package selects its own library and binary by default.
#[test]
fn a_single_package_selects_itself() {
    let case = scenario("single", ".", &[]);
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// Target selectors cover the same targets on both sides.
#[rstest]
#[case::all_targets(scenario("single", ".", &["--all-targets"]))]
#[case::tests(scenario("single", ".", &["--tests"]))]
#[case::lib(scenario("single", ".", &["--lib"]))]
#[case::bins(scenario("single", ".", &["--bins"]))]
#[case::examples(scenario("single", ".", &["--examples"]))]
#[case::benches(scenario("single", ".", &["--benches"]))]
fn target_selectors_match_cargo_clippy(#[case] case: Case<'_>) {
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// A workspace root package does not hide its members when asked for them.
#[rstest]
#[case::root_package_default(scenario("root_members", ".", &[]))]
#[case::workspace(scenario("root_members", ".", &["--workspace"]))]
#[case::workspace_and_package(scenario("root_members", ".", &["--workspace", "-p", "member_a"]))]
#[case::workspace_exclude(scenario("root_members", ".", &["--workspace", "--exclude", "member_a"]))]
#[case::package_colon_version(scenario("root_members", ".", &["-p", "member_a:0.1"]))]
#[case::package(scenario("root_members", ".", &["-p", "member_a"]))]
#[case::package_glob(scenario("root_members", ".", &["-p", "member_*"]))]
#[case::package_bracket_pattern(scenario("root_members", ".", &["-p", "member_[ab]"]))]
#[case::package_partial_version(scenario("root_members", ".", &["-p", "member_a@0.1"]))]
#[case::deprecated_all(scenario("root_members", ".", &["--all"]))]
fn a_root_package_workspace_matches_cargo_clippy(#[case] case: Case<'_>) {
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// Target selectors decide which members have work, and the listing says so.
///
/// A library-only member contributes nothing to `--bins`, so Cargo checks only
/// the member with a binary. The listing must name that member alone.
#[rstest]
#[case::workspace(scenario("mixed", ".", &["--workspace"]))]
#[case::bins(scenario("mixed", ".", &["--workspace", "--bins"]))]
#[case::benches(scenario("mixed", ".", &["--workspace", "--benches"]))]
#[case::named_bin_glob(scenario("mixed", ".", &["--workspace", "--bin", "with*"]))]
fn a_workspace_honours_target_eligibility(#[case] case: Case<'_>) {
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// `default-members` decides the default, at the root and inside the root
/// package's own directory.
#[rstest]
#[case::at_the_root(scenario("root_default_members", ".", &[]))]
#[case::inside_the_root_package(scenario("root_default_members", "src", &[]))]
fn default_members_decide_at_the_root(#[case] case: Case<'_>) {
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// Selecting the workspace ignores a narrower `-p`, as Cargo does.
#[test]
fn the_workspace_selector_wins_over_a_package_selector() {
    let case = scenario("root_default_members", ".", &["--workspace"]);
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// A virtual workspace defaults to every member.
#[test]
fn a_virtual_workspace_defaults_to_every_member() {
    let case = scenario("virtual", ".", &[]);
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// A virtual workspace with `default-members` defaults to those members.
#[test]
fn a_virtual_workspace_honours_default_members() {
    let case = scenario("virtual_default_members", ".", &[]);
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// Running inside a member selects that member.
#[rstest]
#[case::inside_member(scenario("root_members", "member_a", &[]))]
#[case::inside_default_member(scenario("root_default_members", "member_a", &[]))]
#[case::inside_virtual_member(scenario("virtual", "member_b", &[]))]
fn running_inside_a_member_selects_it(#[case] case: Case<'_>) {
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// Globs select and exclude the same members as Cargo.
#[rstest]
#[case::package_glob(scenario("glob", ".", &["-p", "alpha-*"]))]
#[case::caret_class(scenario("glob", ".", &["-p", "[^a]*"]))]
#[case::workspace_exclude_glob(scenario("glob", ".", &["--workspace", "--exclude", "alpha-*"]))]
fn glob_selectors_match_cargo_clippy(#[case] case: Case<'_>) {
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// A manifest path selects the package Cargo would select from it.
#[test]
fn a_manifest_path_selects_the_named_package() {
    let case = scenario(
        "root_default_members",
        ".",
        &["--manifest-path", "member_a/Cargo.toml"],
    );
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
}

/// A manifest path lets the run work from outside the workspace.
///
/// Dylint reads its target directory from the manifest before it forwards
/// anything to Cargo, so Whitaker must pass the option in its own prefix. The
/// run here starts in a directory that is not part of any workspace.
#[test]
fn a_manifest_path_works_from_outside_the_workspace() {
    let fixture =
        SelectionFixture::new("root_default_members").expect("the fixture should be copied");
    let manifest_path = fixture.cwd("member_a/Cargo.toml");
    let manifest = manifest_path
        .to_str()
        .expect("the fixture path is UTF-8")
        .to_owned();
    let outside = scratch_directory().expect("a scratch directory should be available");
    let flags = ["--manifest-path", manifest.as_str()];

    let observed = observe_in(
        &fixture,
        &RunRequest {
            cwd: outside.path(),
            flags: &flags,
            manifest: Some(&manifest),
        },
    )
    .expect("the differential case should run");

    let case = scenario("root_default_members", "<outside the workspace>", &flags);
    assert_matches(case, &observed);
    assert_eq!(
        observed.run.listed,
        BTreeSet::from(["member_a".to_owned()]),
        "the manifest must select its own package"
    );
}

/// `--all` is Cargo's workspace alias and carries its deprecation notice.
#[test]
fn the_deprecated_alias_selects_the_workspace_and_says_so() {
    let case = scenario("root_members", ".", &["--all"]);
    let observed = observe(case).expect("the differential case should run");

    assert_matches(case, &observed);
    assert!(
        observed
            .run
            .stderr
            .contains("warning: --all is deprecated in favour of --workspace"),
        "the deprecated alias must be named on stderr, got:\n{}",
        observed.run.stderr
    );
    assert_eq!(
        observed.run.listed.len(),
        3,
        "--all must select every member, as --workspace does"
    );
}
