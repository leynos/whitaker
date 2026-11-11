# Architectural decision record (ADR) 0014: policy and adapter boundaries for the Clippy snapshot

## Status

Accepted (2026-11-08): Separate the Clippy snapshot acceptance rules from the
Git, filesystem, and operational adapters, so every rule is decided on values
and every observation sits behind a port.

## Date

2026-11-08.

## Context and problem statement

[ADR 0009](adr-0009-verified-clippy-snapshot-for-the-dylint-driver-bootstrap.md)
made the installer own a verified Clippy snapshot. It specified what the
snapshot must satisfy, but every acceptance rule was decided beside the Git and
filesystem calls that observed it. Testing "is this repository acceptable?"
meant building a real repository, testing "is this generation installable?"
meant touching a real directory, and an adapter failure could not be injected
at all. The decision and the observation had no boundary between them.

## Decision drivers

- An acceptance rule must be testable from a literal value, without Git, a
  filesystem, or a subprocess.
- Orchestration must not reach for Git or the filesystem directly.
- An adapter must translate infrastructure failure at its own boundary and
  apply no acceptance rule.
- Production wiring must have exactly one composition root.
- Existing collaborators must be reused rather than duplicated.

## Decision outcome / proposed direction

The module separates the decision from the observation.

- `policy.rs` owns the acceptance rules as pure functions over evidence and
  recorded output. Nothing there reads a file, runs a command, or records
  telemetry. `selection.rs` keeps `select_revision` as the pure kernel; policy
  evaluates its result.
- `ports.rs` owns the driven ports the orchestration needs:
  `RepositoryQueries` (shape, self-containment, integrity, first-parent
  toolchain history, and a file at a commit), `BundleOperations` (header
  verification, head listing, and staging clone), `SnapshotStore` (root
  creation, path resolution, presence probes, digest, text read, lock, staging,
  and publish), and `MetadataStore` (read and write). `RootLock` and `Staging`
  are RAII guards. Release downloads reuse the existing
  `DependencyArchiveDownloader` rather than adding a second downloader port.
- `git.rs` implements `RepositoryQueries` and `BundleOperations`; `store.rs`
  implements `SnapshotStore` and `MetadataStore`. An adapter returns the
  closest honest observation and translates every infrastructure failure into
  `ClippySnapshotError` at that boundary. It also translates Git's listings
  into the values policy compares, such as `RefEntry`, `BundleHead`, and the
  bare and shallow observations as booleans, so policy never parses another
  tool's line format. The failed-integrity diagnostic is worded there too, so
  policy accepts or rejects the recorded result without naming Git's flags. It
  applies no snapshot acceptance rule, except that a failed
  `git bundle verify`, a failed staging clone, a bundle head listing with a
  line that names no head, and metadata that cannot be decoded or names an
  unknown schema are reported as `Invalid`.
- `verify_header` is an observation with one bounded write.
  `git bundle verify` refuses to run outside a repository, so the adapter
  creates a prefixed scratch repository under the system temporary directory,
  or a parent a test supplies, and attempts its removal before returning
  whether the command succeeded, failed, or never ran because initialization
  failed. A removal that fails after a successful command is reported as `Io`
  against the scratch directory, naming the removal. When both the command and
  the removal fail, the `Git` or `Invalid` diagnosis is kept and the cleanup
  failure is appended to it, so neither is lost. What the adapter never does is
  leave scratch state silently behind. The bundle and the caller's snapshot
  paths are never touched.
- `list_heads` stays a pure read. `git bundle list-heads` needs no repository,
  so the adapter creates no scratch state for it, and the test
  `listing_heads_creates_no_scratch_repository` fails if it ever does. A
  listing line that names no head is reported as `Invalid`, so an incomplete
  listing never passes as complete evidence.
- `SnapshotSupport<'_>` is the composition root for one operation: borrowed
  trait objects for each port. `SnapshotSupport::production()` wires the
  process-wide `SnapshotGit` and `ProductionStore`. The public entry points
  (`verify_repository`, `check_installed`, `install_bundle`, `provision`) use
  it, while the explicit-support functions `verify_with`,
  `check_installed_with`, `install_with`, `reuse_with`, `from_sidecar_with`, and
  `provision_support` accept it for tests.
- Query paths record no metrics. Provision, download, and install keep their
  existing telemetry at the command boundaries, and the Git adapter keeps its
  per-command debug events. Policy contributes neither.
- Tests follow the same split. `policy_tests.rs` exercises acceptance rules on
  values, the `orchestration_tests/` modules drive verification and
  installation through the scripted ports in `fakes.rs` and assert both call
  order and what was never called, and `verify_tests.rs` and `tests.rs` keep
  real Git and a real filesystem so the adapters themselves stay honest.

The ports were chosen after sweeping the installer for existing collaborators.
Release downloads already had `DependencyArchiveDownloader`, the producer
already used `RepositoryArchiveDownloader`, the Git adapter reuses the existing
`run_git` helper, and both bundle construction and the production store hash
through `compute_sha256`; all of these are reused rather than duplicated.
Nothing in the installer described a repository query, a bundle operation, or a
snapshot-root operation, so `RepositoryQueries`, `BundleOperations`,
`SnapshotStore`, and `MetadataStore` are new. Each is owned by this module,
implemented only by `git.rs` or `store.rs`, and reached by orchestration through
`SnapshotSupport<'_>` rather than a concrete adapter type.

A new acceptance rule belongs in `policy.rs` with a value test. A new
observation belongs behind an existing port, or a new port only when none fits,
implemented by an adapter that translates failures. Orchestration never calls
Git or the filesystem directly.

## Consequences

- Every acceptance rule is covered by a value test that needs no repository.
- A new observation needs a port method and an adapter implementation, which is
  more ceremony than a direct call.
- The adapters carry the translation surface, so an infrastructure failure
  reaches the caller as a domain variant rather than as a raw error.
