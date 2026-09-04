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
