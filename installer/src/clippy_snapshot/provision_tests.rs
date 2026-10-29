//! Release provisioning against a fake downloader, with no network access.
//!
//! The fake plays the part of the real downloader: it serves a bundle and its
//! `.sha256` sidecar from a fixture directory, refuses to leave bytes behind
//! when they disagree with the sidecar, and records every request so reuse
//! can be shown to download nothing.

use super::*;
use crate::clippy_snapshot::check_installed;
use crate::clippy_snapshot::test_fixtures::{
    DEFAULT_HISTORY, DEFAULT_SELECTED, FixtureRepo, write_sidecar,
};
use crate::dependency_binaries::DependencyBinaryInstallError;
use rstest::{fixture, rstest};
use std::path::Path;
use std::sync::Mutex;

const PINNED_REFS: [&str; 2] = ["refs/heads/main", "HEAD"];

/// How the fake responds to a request for the bundle.
#[derive(Clone, Copy)]
enum Behaviour {
    /// Serve the bundle and a matching sidecar.
    Serve,
    /// Fail as a dropped connection would.
    Fail,
    /// Serve a sidecar whose digest does not match the bundle.
    MismatchedSidecar,
}

/// A downloader that serves fixture files and records what was requested.
struct FakeDownloader {
    served: Utf8PathBuf,
    behaviour: Behaviour,
    requests: Mutex<Vec<String>>,
}

impl FakeDownloader {
    /// Serve the bundle in `served` (named by its asset name) as `behaviour` says.
    fn new(served: Utf8PathBuf, behaviour: Behaviour) -> Self {
        Self {
            served,
            behaviour,
            requests: Mutex::new(Vec::new()),
        }
    }

    /// The asset names requested so far, in order.
    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("lock requests").clone()
    }
}

impl DependencyArchiveDownloader for FakeDownloader {
    /// Mimic the real downloader: write the bytes, check the sidecar, and
    /// delete the bytes unless the two agree.
    fn download(
        &self,
        filename: &str,
        destination: &Path,
    ) -> Result<(), DependencyBinaryInstallError> {
        self.requests
            .lock()
            .expect("lock requests")
            .push(filename.to_owned());
        if matches!(self.behaviour, Behaviour::Fail) {
            return Err(DependencyBinaryInstallError::Download {
                url: filename.to_owned(),
                reason: "connection reset".to_owned(),
            });
        }
        std::fs::copy(self.served.join(filename), destination)?;
        let sidecar = std::fs::read_to_string(self.served.join(format!("{filename}.sha256")))?;
        let expected = sidecar.split_whitespace().next().unwrap_or_default();
        let actual = crate::test_utils::sha256_hex(&std::fs::read(destination)?);
        if expected == actual {
            return Ok(());
        }
        std::fs::remove_file(destination)?;
        Err(DependencyBinaryInstallError::Checksum {
            archive: destination.to_owned(),
            expected: expected.to_owned(),
            actual,
        })
    }
}

/// A release asset directory, the manifest pinning it, and a fresh root.
struct Release {
    repo: FixtureRepo,
    manifest: ClippySnapshotManifest,
    served: Utf8PathBuf,
    root: Utf8PathBuf,
}

impl Release {
    /// A fake downloader serving this release as `behaviour` says.
    fn downloader(&self, behaviour: Behaviour) -> FakeDownloader {
        FakeDownloader::new(self.served.clone(), behaviour)
    }

    /// Provision from the release through `downloader`.
    fn provision(&self, downloader: &FakeDownloader) -> Result<Utf8PathBuf, ClippySnapshotError> {
        provision_with(
            &BundleSource::Release,
            &self.manifest,
            &self.root,
            downloader,
        )
    }

    /// The entries under the root whose names start with `prefix`.
    fn entries_starting_with(&self, prefix: &str) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        entries
            .map(|entry| entry.expect("read root entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| name.starts_with(prefix))
            .collect()
    }
}

#[fixture]
fn release() -> Release {
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    let manifest = repo.manifest(repo.tip(), &repo.commits[DEFAULT_SELECTED]);
    let served = repo.path("served");
    std::fs::create_dir_all(&served).expect("create served directory");
    let bundle = repo.bundle(&manifest.bundle_asset_name(), &PINNED_REFS);
    let asset = served.join(manifest.bundle_asset_name());
    std::fs::rename(&bundle, &asset).expect("move bundle into served directory");
    std::fs::rename(format!("{bundle}.sha256"), format!("{asset}.sha256"))
        .expect("move sidecar into served directory");
    let root = repo.path("root");
    Release {
        repo,
        manifest,
        served,
        root,
    }
}

#[rstest]
fn release_source_requests_the_pinned_asset_and_installs_it(release: Release) {
    let downloader = release.downloader(Behaviour::Serve);

    let generation = release
        .provision(&downloader)
        .expect("the release installs");

    let expected = format!("whitaker-clippy-snapshot-{}.bundle", release.repo.tip());
    assert_eq!(release.manifest.bundle_asset_name(), expected);
    assert_eq!(downloader.requests(), vec![expected]);
    assert!(
        generation.is_absolute(),
        "DYLINT_CLIPPY_REPO must be absolute"
    );
    assert_eq!(generation, release.root.join(release.repo.tip()));
    check_installed(&generation, release.repo.tip()).expect("the generation verifies");
    assert!(release.entries_starting_with(".download-").is_empty());
}

#[rstest]
fn a_second_provision_reuses_the_generation_without_downloading(release: Release) {
    let first = release.downloader(Behaviour::Serve);
    let generation = release.provision(&first).expect("first provision");
    let second = release.downloader(Behaviour::Serve);

    let again = release.provision(&second).expect("second provision");

    assert_eq!(again, generation);
    assert_eq!(first.requests().len(), 1);
    assert!(second.requests().is_empty(), "reuse must not download");
}

#[rstest]
fn a_download_failure_leaves_no_generation_or_temporary_directory(release: Release) {
    let downloader = release.downloader(Behaviour::Fail);

    let error = release
        .provision(&downloader)
        .expect_err("a failed download must fail provisioning");

    assert!(
        matches!(error, ClippySnapshotError::Download { .. }),
        "{error}"
    );
    assert_eq!(downloader.requests().len(), 1);
    assert!(!release.root.join(release.repo.tip()).exists());
    assert!(release.entries_starting_with(".download-").is_empty());
    assert!(release.entries_starting_with(".incoming-").is_empty());
}

#[rstest]
fn a_mismatched_sidecar_fails_with_the_digest_error_and_installs_nothing(release: Release) {
    let asset = release.served.join(release.manifest.bundle_asset_name());
    std::fs::write(
        format!("{asset}.sha256"),
        format!(
            "{}  {}\n",
            "0".repeat(64),
            release.manifest.bundle_asset_name()
        ),
    )
    .expect("corrupt sidecar");
    let downloader = release.downloader(Behaviour::MismatchedSidecar);

    let error = release
        .provision(&downloader)
        .expect_err("a checksum mismatch must fail provisioning");

    assert!(
        matches!(&error, ClippySnapshotError::Download { reason, .. }
            if reason.contains("checksum verification failed")),
        "{error}"
    );
    assert!(!release.root.join(release.repo.tip()).exists());
    assert!(release.entries_starting_with(".download-").is_empty());
}

#[rstest]
fn a_served_non_bundle_with_a_matching_sidecar_is_rejected_at_install(release: Release) {
    // Corrupting bundle and sidecar together satisfies the downloader's
    // checksum, so only installation's own bundle checks can refuse it.
    let asset = release.served.join(release.manifest.bundle_asset_name());
    std::fs::write(&asset, b"not a bundle").expect("replace bundle");
    write_sidecar(&asset);
    let downloader = release.downloader(Behaviour::Serve);

    release
        .provision(&downloader)
        .expect_err("a non-bundle must not install");

    assert!(!release.root.join(release.repo.tip()).exists());
}

/// A provisioning run, with the metric labels it should record.
#[derive(Clone, Copy)]
enum MetricScenario {
    /// Nothing installed; the release serves the bundle.
    Install,
    /// Nothing installed; the download fails.
    FailDownload,
    /// The generation is already installed.
    Reuse,
}

impl MetricScenario {
    /// The `(source, outcome)` labels this scenario records.
    const fn labels(self) -> (&'static str, &'static str) {
        match self {
            Self::Install => ("release", "ok"),
            Self::FailDownload => ("release", "failed"),
            Self::Reuse => ("existing", "ok"),
        }
    }
}

#[rstest]
#[case::installs_from_the_release(MetricScenario::Install)]
#[case::fails_to_download(MetricScenario::FailDownload)]
#[case::reuses_the_generation(MetricScenario::Reuse)]
fn provisioning_records_its_source_and_outcome(release: Release, #[case] scenario: MetricScenario) {
    use crate::clippy_snapshot::telemetry::PROVISION_TOTAL;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    if matches!(scenario, MetricScenario::Reuse) {
        release
            .provision(&release.downloader(Behaviour::Serve))
            .expect("first provision");
    }
    let behaviour = match scenario {
        MetricScenario::FailDownload => Behaviour::Fail,
        MetricScenario::Install | MetricScenario::Reuse => Behaviour::Serve,
    };
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let downloader = release.downloader(behaviour);

    // The outcome itself is asserted by the tests above; only the metric here.
    let _result = metrics::with_local_recorder(&recorder, || release.provision(&downloader));

    let (source, outcome) = scenario.labels();
    let recorded = snapshotter.snapshot().into_vec();
    let has_metric = recorded.iter().any(|(key, _, _, value)| {
        let labels: Vec<_> = key.key().labels().map(|l| (l.key(), l.value())).collect();
        key.key().name() == PROVISION_TOTAL
            && labels.contains(&("source", source))
            && labels.contains(&("outcome", outcome))
            && matches!(value, DebugValue::Counter(1))
    });
    assert!(has_metric, "expected {source}/{outcome}; got {recorded:#?}");
}
