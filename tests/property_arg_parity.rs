//! Differential property tests for root and legacy installer argument parity.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
};

use clap::Parser as _;
use ortho_config::NoOpLocalizer;
use proptest::prelude::*;
use whitaker::{
    cli::{Cli, Command},
    ports::InstallRequest,
};
use whitaker_installer::cli::{Cli as InstallerCli, Command as InstallerCommand, InstallArgs};

#[path = "property_arg_parity/short_clusters.rs"]
mod short_clusters;

const OPTION_PAIR_COUNT: usize = 91;

#[derive(Clone, Copy)]
#[repr(u16)]
enum OptionKind {
    TargetDir = 1 << 0,
    Lint = 1 << 1,
    IndividualLints = 1 << 2,
    Experimental = 1 << 3,
    Jobs = 1 << 4,
    Toolchain = 1 << 5,
    Cranelift = 1 << 6,
    DryRun = 1 << 7,
    Verbosity = 1 << 8,
    Quiet = 1 << 9,
    SkipDeps = 1 << 10,
    SkipWrapper = 1 << 11,
    NoUpdate = 1 << 12,
    BuildOnly = 1 << 13,
}

impl OptionKind {
    const ALL: [Self; 14] = [
        Self::TargetDir,
        Self::Lint,
        Self::IndividualLints,
        Self::Experimental,
        Self::Jobs,
        Self::Toolchain,
        Self::Cranelift,
        Self::DryRun,
        Self::Verbosity,
        Self::Quiet,
        Self::SkipDeps,
        Self::SkipWrapper,
        Self::NoUpdate,
        Self::BuildOnly,
    ];

    const fn bit(self) -> u16 { self as u16 }

    const fn name(self) -> &'static str {
        match self {
            Self::TargetDir => "target-dir",
            Self::Lint => "lint",
            Self::IndividualLints => "individual-lints",
            Self::Experimental => "experimental",
            Self::Jobs => "jobs",
            Self::Toolchain => "toolchain",
            Self::Cranelift => "cranelift",
            Self::DryRun => "dry-run",
            Self::Verbosity => "verbosity",
            Self::Quiet => "quiet",
            Self::SkipDeps => "skip-deps",
            Self::SkipWrapper => "skip-wrapper",
            Self::NoUpdate => "no-update",
            Self::BuildOnly => "build-only",
        }
    }

    fn append(self, arguments: &mut Vec<OsString>) {
        match self {
            Self::TargetDir => {
                arguments.extend(["-t", "/tmp/Whitaker install λ"].map(OsString::from));
            }
            Self::Lint => arguments.extend(["-l", "module_max_lines"].map(OsString::from)),
            Self::IndividualLints => arguments.push(OsString::from("--individual-lints")),
            Self::Experimental => arguments.push(OsString::from("--experimental")),
            Self::Jobs => arguments.extend(["-j", "2"].map(OsString::from)),
            Self::Toolchain => {
                arguments.extend(["--toolchain", "nightly-test"].map(OsString::from));
            }
            Self::Cranelift => arguments.push(OsString::from("--cranelift")),
            Self::DryRun => arguments.push(OsString::from("--dry-run")),
            Self::Verbosity => arguments.push(OsString::from("--verbosity")),
            Self::Quiet => arguments.push(OsString::from("-q")),
            Self::SkipDeps => arguments.push(OsString::from("--skip-deps")),
            Self::SkipWrapper => arguments.push(OsString::from("--skip-wrapper")),
            Self::NoUpdate => arguments.push(OsString::from("--no-update")),
            Self::BuildOnly => arguments.push(OsString::from("--build-only")),
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1))]

    #[test]
    fn root_install_matches_legacy_install_for_4096_generated_argument_sets(
        generated_masks in prop::collection::vec(0_u16..(1 << 14), 4096 - 105)
    ) {
        let mut seen_options = BTreeSet::new();
        let mut seen_pairs = BTreeSet::new();
        let mut option_counts = BTreeMap::new();
        let mut masks = Vec::with_capacity(4096);
        for kind in OptionKind::ALL {
            masks.push(kind.bit());
        }
        for (left_index, left) in OptionKind::ALL.iter().enumerate() {
            for right in OptionKind::ALL.iter().skip(left_index + 1) {
                masks.push(left.bit() | right.bit());
            }
        }
        masks.extend(generated_masks);

        let mut rotation_index = 0;
        for (case_index, mask) in masks.iter().copied().enumerate() {
            let mut selected: Vec<OptionKind> = OptionKind::ALL
                .iter()
                .copied()
                .filter(|kind| mask & kind.bit() != 0)
                .collect();
            for kind in &selected {
                seen_options.insert(kind.name());
                *option_counts.entry(kind.name()).or_insert(0_usize) += 1;
            }
            for (left_index, left) in selected.iter().enumerate() {
                for right in selected.iter().skip(left_index + 1) {
                    seen_pairs.insert((left.name(), right.name()));
                }
            }

            if !selected.is_empty() {
                if rotation_index >= selected.len() {
                    rotation_index = 0;
                }
                selected.rotate_left(rotation_index);
                rotation_index += 1;
                if case_index & 1 == 0 {
                    selected.reverse();
                }
            }
            let mut install_arguments = Vec::new();
            for kind in selected {
                kind.append(&mut install_arguments);
            }
            if mask & OptionKind::Lint.bit() != 0 {
                install_arguments.extend(["-l", "no_expect_outside_tests"].map(OsString::from));
            }
            if mask & OptionKind::Verbosity.bit() != 0 && case_index & 1 == 0 {
                install_arguments.push(OsString::from("-v"));
            }

            let root_result = parse_root(&install_arguments);
            let legacy_result = parse_legacy(&install_arguments, case_index & 1 == 0);
            match (root_result, legacy_result) {
                (Ok(root), Ok(legacy)) => {
                    prop_assert!(matches!(root.command, Command::Install(_)));
                    let Command::Install(parsed) = &root.command else {
                        prop_assert!(false, "root install parser returned a different command");
                        continue;
                    };
                    let root_arguments = root.install_arguments(parsed);
                    let legacy_arguments = legacy.install_args();
                    prop_assert_eq!(
                        root_arguments.clone(),
                        legacy_arguments.clone(),
                        "argument mapping drifted for case {}: {:?}",
                        case_index,
                        install_arguments,
                    );
                    let request = InstallRequest { arguments: root_arguments };
                    let expected_request = InstallRequest {
                        arguments: legacy_arguments.clone(),
                    };
                    prop_assert_eq!(request, expected_request);
                }
                (Err(root), Err(legacy)) => {
                    prop_assert_eq!(
                        root.exit_code(),
                        legacy.exit_code(),
                        "rejection exit code drifted for case {}: {:?}",
                        case_index,
                        install_arguments,
                    );
                }
                (root, legacy) => {
                    prop_assert!(false,
                        "parser acceptance differed for case {case_index}: root={root:?}, legacy={legacy:?}, args={install_arguments:?}");
                }
            }
        }

        prop_assert_eq!(seen_options.len(), OptionKind::ALL.len(), "not every installer option appeared");
        prop_assert_eq!(seen_pairs.len(), OPTION_PAIR_COUNT, "not every option pair co-occurred");
        for kind in OptionKind::ALL {
            prop_assert!(
                option_counts.get(kind.name()).copied().unwrap_or_default() >= 205,
                "option --{} appeared in fewer than 5% of the 4096 cases",
                kind.name(),
            );
        }
    }
}

fn parse_root(arguments: &[OsString]) -> Result<Cli, clap::Error> {
    let mut raw = vec![OsString::from("whitaker"), OsString::from("install")];
    raw.extend(arguments.iter().cloned());
    Cli::try_parse_localized_from(raw, &NoOpLocalizer::new())
}

fn parse_legacy(
    arguments: &[OsString],
    explicit_subcommand: bool,
) -> Result<InstallerCli, clap::Error> {
    let mut raw = vec![OsString::from("whitaker-installer")];
    if explicit_subcommand {
        raw.push(OsString::from("install"));
    }
    raw.extend(arguments.iter().cloned());
    InstallerCli::try_parse_from(raw)
}

#[test]
fn root_install_type_is_the_installer_install_type() {
    let parsed = parse_root(&[OsString::from("--dry-run")]).expect("root install should parse");
    let Command::Install(arguments) = parsed.command else {
        panic!("root command should parse install");
    };
    assert!(
        arguments.execution.dry_run,
        "root install options should use installer args"
    );
    let _: InstallArgs = arguments;
}

#[test]
fn explicit_installer_install_subcommand_is_supported() {
    let parsed = parse_legacy(&[OsString::from("--dry-run")], true)
        .expect("legacy explicit install should parse");
    assert!(matches!(parsed.command, Some(InstallerCommand::Install(_))));
}
