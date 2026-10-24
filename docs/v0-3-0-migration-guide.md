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

## Installer Rust APIs

The installer API also changes in 0.3.0. Update implementations of `BaseDirs`
to use its shorter method names:

| 0.2.x method          | 0.3.0 method      |
| --------------------- | ----------------- |
| `home_dir()`          | `home()`          |
| `bin_dir()`           | `executables()`   |
| `whitaker_data_dir()` | `whitaker_data()` |

`InstallArgs` groups related fields into `lint_selection`, `execution` and
`skip` values:

| 0.2.x field        | 0.3.0 field                       |
| ------------------ | --------------------------------- |
| `individual_lints` | `lint_selection.individual_lints` |
| `experimental`     | `lint_selection.experimental`     |
| `cranelift`        | `execution.cranelift`             |
| `dry_run`          | `execution.dry_run`               |
| `skip_deps`        | `skip.skip_deps`                  |
| `skip_wrapper`     | `skip.skip_wrapper`               |
| `no_update`        | `skip.no_update`                  |

The clap groups are flattened, so command-line flags and their spellings are
unchanged.

The install metrics rate accessors now return integer permille (`u64`, from 0
to 1000) instead of floating-point rates:

| 0.2.x method      | 0.3.0 method               |
| ----------------- | -------------------------- |
| `download_rate()` | `download_rate_permille()` |
| `build_rate()`    | `build_rate_permille()`    |

Each unit is a tenth of a percentage point: 333 permille represents 33.3%.
Packaging functions now borrow their parameter structs. Pass a reference to
`package_artefact`, `package_installer` and `package_dependency_binary`:

```rust
// Before 0.3.0
package_artefact(artefact_params)?;
package_installer(installer_params)?;
package_dependency_binary(dependency_params)?;

// From 0.3.0
package_artefact(&artefact_params)?;
package_installer(&installer_params)?;
package_dependency_binary(&dependency_params)?;
```

The binstall metadata helpers `load_cargo_toml()` and
`extract_binstall_table()` (available to tests and the `test-support` feature)
now return `Result<toml::Table, String>`. Propagate or handle these errors
instead of assuming the manifest and metadata table are present:

```rust
// Before 0.3.0
let manifest = load_cargo_toml();
let binstall = extract_binstall_table(&manifest);

// From 0.3.0; the caller returns Result<_, String>.
let manifest = load_cargo_toml()?;
let binstall = extract_binstall_table(&manifest)?;
```

For example, update method names, field access and packaging calls at the
caller:

```rust
// Before 0.3.0
let home = base_dirs.home_dir();
let experimental = args.experimental;
let rate_percent = metrics.download_rate() * 100.0;
package_installer(params)?;

// From 0.3.0
let home = base_dirs.home();
let experimental = args.lint_selection.experimental;
let rate_permille = metrics.download_rate_permille(); // 333 represents 33.3%.
package_installer(&params)?;
```
