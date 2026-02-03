# Architectural decision record (ADR) 001: prebuilt Dylint lint library distribution

## Status

Accepted (2026-02-03): Distribute prebuilt Dylint lint libraries as rolling
release artefacts aligned to the pinned toolchain and target triple, with a
download-first installer flow and a local compilation fallback.

## Date

2026-02-03.

## Context and problem statement

Whitaker's first-run experience currently requires building every Dylint lint
library locally, which is slow on common developer machines and in CI. Dylint
lint libraries are dynamic libraries that must be built with the exact Rust
toolchain version used by the consumer. The workspace already pins a nightly
toolchain in `rust-toolchain.toml`, and the installer is responsible for
installing it. On Linux, compatibility also depends on the glibc baseline of
the build environment.

The problem is to reduce installation and update time without sacrificing
correctness or portability. The solution must honour the pinned toolchain,
provide safe verification, and retain a reliable fallback for unsupported
platforms or failed downloads.

## Decision drivers

- Reduce first-run and update latency for supported platforms.
- Keep lint libraries aligned with the pinned toolchain and workspace commit.
- Preserve a deterministic fallback path that builds locally.
- Maintain Linux portability by targeting a conservative glibc baseline.
- Ensure artefacts are verifiable before use.

## Requirements

### Functional requirements

- Provide prebuilt lint libraries for common target triples.
- Allow the installer to prefer downloads while retaining local compilation.
- Store libraries in a predictable per-toolchain, per-target directory.
- Surface clear warnings when downloads are unavailable or invalid.

### Technical requirements

- Build artefacts with the exact toolchain pinned in `rust-toolchain.toml`.
- Package a manifest and checksum with each artefact.
- Use an artefact naming scheme that is deterministic and traceable.

## Options considered

### Option A: rolling prebuilt lint libraries with download-first installer

Build and publish lint libraries for a small target matrix, attach them to a
rolling release tag, and have the installer download and verify the matching
artefact before falling back to a local build.

### Option B: local compilation only

Continue to build all lint libraries locally, avoiding release automation and
artefact distribution.

### Option C: prebuilt artefacts only for versioned releases

Publish lint libraries only alongside tagged releases and require downloads to
match tags, leaving other commits to local compilation.

Screen reader note: The following table compares the options across the key
decision drivers.

| Topic                  | Option A                     | Option B           | Option C                      |
| ---------------------- | ---------------------------- | ------------------ | ----------------------------- |
| Install speed          | Fast for common platforms    | Slow               | Fast on tagged releases only  |
| Toolchain alignment    | Explicit via toolchain pin   | Implicit via local | Explicit for tags only        |
| Operational complexity | Moderate CI + release wiring | Low                | Moderate, plus tag discipline |
| Artefact storage       | Rolling retention            | None               | Per release                   |
| Offline support        | Fallback build available     | Always available   | Fallback build for non-tagged |

_Table 1: Trade-offs between distribution approaches._

## Decision outcome / proposed direction

Adopt Option A. Whitaker will distribute prebuilt Dylint lint libraries as
rolling release assets per supported target, built with the pinned toolchain,
and the installer will attempt a verified download before compiling locally.

Concrete decisions:

- Keep `rust-toolchain.toml` as the single source of the lint toolchain and
  require both CI and the installer to use it.
- Build a target matrix covering `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`,
  and `x86_64-pc-windows-msvc`. Linux builds target a conservative glibc
  baseline using the oldest supported runner image.
- Name artefacts `whitaker-lints-<git_sha>-<toolchain>-<target>.tar.zst`.
- Ship a `manifest.json` and checksum in each artefact. The manifest captures
  the git SHA, toolchain, target triple, build time, artefact list, and the
  checksum of the archive.

Screen reader note: The following JSON snippet illustrates the manifest format.

```json
{
  "git_sha": "abc1234",
  "toolchain": "nightly-2025-09-18",
  "target": "x86_64-unknown-linux-gnu",
  "generated_at": "2026-02-03T00:00:00Z",
  "files": [
    "libwhitaker_lints@nightly-2025-09-18-x86_64-unknown-linux-gnu.so"
  ],
  "sha256": "..."
}
```

- The installer will detect the target triple, download the matching artefact
  from the rolling release, verify the checksum (and signature if provided),
  extract to `~/.local/share/whitaker/lints/<toolchain>/<target>/lib`, and set
  `DYLINT_LIBRARY_PATH` to that directory. Failures trigger a local build and a
  warning.

## Goals and non-goals

Goals:

- Reduce install and update time on common platforms.
- Provide deterministic, verifiable artefacts tied to toolchain and commit.
- Keep local compilation as a dependable fallback.

Non-goals:

- Guarantee prebuilt support for every target or libc variant.
- Eliminate local builds entirely.
- Provide a universal musl target without explicit demand.

## Migration plan

1. Document the artefact naming scheme, manifest schema, and verification
   policy.
2. Add CI automation to build the target matrix and publish rolling release
   assets.
3. Extend the installer to perform download, verification, extraction, and
   fallback compilation.
4. Record download-versus-build metrics and total install time.

## Known risks and limitations

- Glibc compatibility may still exclude older distributions, requiring a local
  build on those hosts.
- Rolling releases risk stale assets if CI fails; the installer must fail
  closed and fall back to local compilation.
- Supply-chain risks require checksum or signature verification and careful
  handling of extraction paths.

## Architectural rationale

The decision aligns with the existing toolchain pinning strategy and keeps the
lint distribution model deterministic. A rolling release tag keeps the latest
artefacts discoverable without coupling every commit to a tagged release, while
the download-first installer preserves a reliable local build path for
unsupported platforms or transient failures.
