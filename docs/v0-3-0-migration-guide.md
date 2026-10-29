# Whitaker 0.3.0 migration guide

This guide covers source-breaking public API changes when upgrading library
consumers from the 0.2.x line to Whitaker 0.3.0. The release standardizes
Rust-facing identifiers on en-GB-oxendict spelling. Existing consumers must
update source references to the replacement names below; compatibility aliases
are not provided for the removed spellings.

## Migration scope

The changes affect Rust imports, type names, field access, and method calls.
They do not change the SARIF 2.1.0 serialized representation. Serde renames
keep the JSON keys `artifacts` and `artifactLocation`, so no serialized-data
migration is needed.

## `whitaker_common` APIs

Update the locale helper import and calls:

```rust
use whitaker_common::i18n::normalise_locale;

let locale = normalise_locale(Some("en-GB"));
```

becomes:

```rust
use whitaker_common::i18n::normalize_locale;

let locale = normalize_locale(Some("en-GB"));
```

The complete function mapping is:

| 0.2.x name         | 0.3.0 name         |
| ------------------ | ------------------ |
| `normalise_locale` | `normalize_locale` |
| `rasterise_signal` | `rasterize_signal` |

Update complexity-signal rasterization in the same way. The deprecated
`rasterise_signal` compatibility wrapper is removed; callers must use
`rasterize_signal`:

```rust
use whitaker_common::complexity_signal::rasterize_signal;

let signal = rasterize_signal(function_lines, segments)?;
```

## `bumpy_road_function` API

Rename the settings normalizer at its import and call sites:

```rust
use bumpy_road_function::analysis::{normalize_settings, Settings};

fn normalized(settings: Settings) -> Settings {
    normalize_settings(settings)
}
```

The old `normalise_settings` function is replaced by `normalize_settings`.

The complete settings mapping is:

| 0.2.x name           | 0.3.0 name           |
| -------------------- | -------------------- |
| `normalise_settings` | `normalize_settings` |

## `whitaker_sarif` APIs

The SARIF model uses `Artefact` and `ArtefactLocation` for Rust identifiers.
Update type imports and values as follows:

```rust
use whitaker_sarif::{Artefact, ArtefactLocation};

let artefact = Artefact {
    location: ArtefactLocation {
        uri: "src/main.rs".into(),
        uri_base_id: None,
    },
    mime_type: Some("text/x-rust".into()),
};
```

The complete type mapping is:

| 0.2.x name                         | 0.3.0 name                         |
| ---------------------------------- | ---------------------------------- |
| `whitaker_sarif::Artifact`         | `whitaker_sarif::Artefact`         |
| `whitaker_sarif::ArtifactLocation` | `whitaker_sarif::ArtefactLocation` |

Update renamed fields and builder methods at the same time:

```rust
use whitaker_sarif::{Artefact, ArtefactLocation, PhysicalLocation, RunBuilder};

let physical_location = PhysicalLocation {
    artefact_location: ArtefactLocation {
        uri: "src/main.rs".into(),
        uri_base_id: None,
    },
    region: None,
};

let run = RunBuilder::new("tool", "0.3.0")
    .with_artefact(Artefact {
        location: ArtefactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: None,
        },
        mime_type: None,
    })
    .build();

assert_eq!(run.artefacts.len(), 1);
assert_eq!(physical_location.artefact_location.uri, "src/main.rs");
```

The field and method mappings are:

| 0.2.x name                           | 0.3.0 name                           |
| ------------------------------------ | ------------------------------------ |
| `Run.artifacts`                      | `Run.artefacts`                      |
| `PhysicalLocation.artifact_location` | `PhysicalLocation.artefact_location` |
| `RunBuilder::with_artifact`          | `.with_artefact`                     |

The Rust names change while the SARIF wire names remain fixed. The
`Run.artefacts` field carries `#[serde(rename = "artifacts")]`, and the
`PhysicalLocation::artefact_location` field carries
`#[serde(rename = "artifactLocation")]`. Consequently, serialized JSON still
uses the SARIF 2.1.0 keys `artifacts` and `artifactLocation`; no update to
stored SARIF data or downstream SARIF readers is required.

## Installer dependency query

`whitaker_installer::deps::check_dylint_tools` now returns
`Result<DylintToolStatus>` so callers can distinguish a missing or unpinned
tool from a failed availability query. Propagate the result before deciding
whether to install:

```rust
let status = check_dylint_tools(&executor)?;
if !status.all_installed() {
    install_dylint_tools(&executor, &status)?;
}
```

The error enum also adds `InstallerError::DependencyCheck`. Update exhaustive
matches on `InstallerError` to handle this query failure. A missing tool or a
version mismatch remains a successful query with a `false` status field.
Unexpected command failures or version output without a semantic version now
return an error instead of being treated as permission to install.

## Installer bootstrap and wrappers

This release makes the Dylint driver bootstrap reproducible. Whitaker no longer
lets Dylint clone public Clippy to build its driver; the installer provisions a
verified Clippy snapshot instead, and never falls back to public Clippy. See
[ADR 009](adr-009-verified-clippy-snapshot-for-dylint-driver-bootstrap.md) for
the rationale and
[the users' guide](users-guide.md#the-installed-dylint-bootstrap) for the
behaviour.

### Regenerate existing wrappers

The `whitaker` and `whitaker-ls` wrappers no longer embed `DYLINT_LIBRARY_PATH`
or toolchain paths. They call `whitaker-installer run -- ...`. Wrappers written
by an earlier release do not do this, so re-run `whitaker-installer` to
regenerate them. The run also provisions the verified Clippy snapshot and writes
`bootstrap.json` beside it.

### The `run` subcommand

`whitaker-installer run -- ARGS...` runs `cargo dylint` with `ARGS` passed
through unchanged. It overwrites `DYLINT_LIBRARY_PATH`, `DYLINT_CLIPPY_REPO`,
and `DYLINT_DRIVER_PATH` for the child process, and passes every other
variable, including `RUSTC_WRAPPER` and `RUSTFLAGS`, through. It requires the
`cargo-dylint` on `PATH` to report the Dylint release the bootstrap was built
for (6.3.0).

### Offline installs

Pass `--clippy-snapshot-bundle FILE` to install a Clippy snapshot bundle, with
its `.sha256` sidecar beside it, instead of downloading the published one. The
bundle is verified exactly as a downloaded one is, which allows offline and
air-gapped installs.

### Skipping the wrappers

Users who relied on `--skip-wrapper` should use the `DYLINT_LIBRARY_PATH`,
`DYLINT_CLIPPY_REPO`, and `DYLINT_DRIVER_PATH` exports that the installer now
prints. Without the wrappers, let one `cargo dylint` run finish before starting
others, because nothing else serializes the first driver build.

### Library API changes

Crates that depend on `whitaker_installer` see these changes:

- The public `bootstrap` and `clippy_snapshot` modules are new.
- `InstallerError` gains the `ClippySnapshot` and `Bootstrap` variants. Update
  exhaustive matches.
- The CLI gains `Command::Run(RunArgs)`. Update exhaustive matches on
  `Command`.
- `wrapper::generate_wrapper_scripts` takes one argument, `dirs`, instead of
  two. The `library_path` argument is removed because the wrappers no longer
  embed it:

  ```rust
  let result = generate_wrapper_scripts(&dirs)?;
  ```
