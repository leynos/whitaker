//! Tests for the Cargo selector surface.

use rstest::rstest;

use super::CargoOptions;

/// Render a set of options and compare it with the literal Cargo vector.
#[rstest]
#[case::empty(CargoOptions::default(), &[])]
#[case::workspace(
    CargoOptions { workspace: true, ..CargoOptions::default() },
    &["--workspace"]
)]
#[case::deprecated_all(
    CargoOptions { all: true, ..CargoOptions::default() },
    &["--all"]
)]
#[case::packages(
    CargoOptions {
        packages: vec!["a".to_owned(), "b*".to_owned()],
        ..CargoOptions::default()
    },
    &["-p", "a", "-p", "b*"]
)]
#[case::excludes(
    CargoOptions {
        workspace: true,
        excludes: vec!["b".to_owned()],
        ..CargoOptions::default()
    },
    &["--workspace", "--exclude", "b"]
)]
#[case::targets(
    CargoOptions {
        lib: true,
        bins: true,
        bin: vec!["tool".to_owned()],
        tests: true,
        test: vec!["smoke".to_owned()],
        examples: true,
        example: vec!["demo".to_owned()],
        benches: true,
        bench: vec!["load".to_owned()],
        all_targets: true,
        ..CargoOptions::default()
    },
    &[
        "--lib", "--bins", "--bin", "tool", "--tests", "--test", "smoke",
        "--examples", "--example", "demo", "--benches", "--bench", "load",
        "--all-targets",
    ]
)]
#[case::features(
    CargoOptions {
        features: vec!["one two".to_owned(), "three".to_owned()],
        all_features: true,
        no_default_features: true,
        ..CargoOptions::default()
    },
    &[
        "--features", "one two", "--features", "three", "--all-features",
        "--no-default-features",
    ]
)]
#[case::build(
    CargoOptions {
        target: vec!["x86_64-unknown-linux-gnu".to_owned()],
        profile: Some("ci".to_owned()),
        release: false,
        locked: true,
        frozen: false,
        offline: true,
        jobs: Some("4".to_owned()),
        ..CargoOptions::default()
    },
    &[
        "--target", "x86_64-unknown-linux-gnu", "--profile", "ci",
        "--locked", "--offline", "-j", "4",
    ]
)]
#[case::manifest_is_dylints(
    CargoOptions {
        manifest_path: Some("app/Cargo.toml".into()),
        ..CargoOptions::default()
    },
    &[]
)]
#[case::target_dir_is_rejected_before_rendering(
    CargoOptions {
        target_dir: Some("target".into()),
        ..CargoOptions::default()
    },
    &[]
)]
fn options_render_as_cargo_arguments(#[case] options: CargoOptions, #[case] expected: &[&str]) {
    let rendered = options.tokens();
    let wanted: Vec<std::ffi::OsString> = expected.iter().map(|s| (*s).into()).collect();

    assert_eq!(
        rendered, wanted,
        "the rendered vector must match Cargo's own spelling"
    );
}

/// Parse job counts in the spellings Cargo accepts.
#[rstest]
#[case::separate(&["--jobs", "4"], "4")]
#[case::attached(&["-j4"], "4")]
#[case::negative(&["--jobs", "-1"], "-1")]
#[case::negative_equals(&["--jobs=-1"], "-1")]
fn job_counts_parse_as_cargo_accepts_them(#[case] flags: &[&str], #[case] expected: &str) {
    let mut arguments = vec!["whitaker", "check"];
    arguments.extend_from_slice(flags);
    let cli =
        crate::cli::Cli::try_parse_localized_from(arguments, &ortho_config::NoOpLocalizer::new())
            .expect("the job flag should parse");
    let crate::cli::Command::Check(check) = cli.command else {
        panic!("the arguments should select `check`");
    };

    assert_eq!(
        check.cargo.jobs.as_deref(),
        Some(expected),
        "`{flags:?}` must reach the jobs option unchanged"
    );
}

/// Parse both of Cargo's release spellings.
#[rstest]
#[case::long(&["--release"])]
#[case::short(&["-r"])]
fn both_release_spellings_set_the_selector(#[case] flags: &[&str]) {
    let mut arguments = vec!["whitaker", "check"];
    arguments.extend_from_slice(flags);
    let cli =
        crate::cli::Cli::try_parse_localized_from(arguments, &ortho_config::NoOpLocalizer::new())
            .expect("the release flag should parse");
    let crate::cli::Command::Check(check) = cli.command else {
        panic!("the arguments should select `check`");
    };

    assert!(
        check.cargo.release,
        "`{flags:?}` must set the release selector"
    );
}

/// Keep `--release` available as its own selector, not only `--profile`.
#[test]
fn the_release_selector_renders_as_itself() {
    let options = CargoOptions {
        release: true,
        ..CargoOptions::default()
    };

    assert_eq!(
        options.tokens(),
        ["--release"],
        "`--release` must reach Cargo unchanged"
    );
}
