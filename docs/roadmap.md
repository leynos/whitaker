# Development roadmap

## 0. Repository scaffolding

### 0.1. Workspace foundations

- [ ] 0.1.1. Initialise the Cargo workspace with workspace members and
  `rust-toolchain` pin.
- [ ] 0.1.2. Add baseline project metadata (`README`, `LICENSE`, contributing
  guide, `CODEOWNERS`).
- [ ] 0.1.3. Establish Makefile or justfile targets for fmt, lint, test, and UI
  test orchestration.

## 1. Common infrastructure

### 1.1. Common crate helpers

- [x] 1.1.1. Implement the `common` crate helpers for attributes, context
  detection, spans, and diagnostics.
- [x] 1.1.2. Wire shared configuration loading with
  `dylint_linting::config_or_default` and serde structs.

### 1.2. UI test harness

- [x] 1.2.1. Integrate `dylint_testing` harness boilerplate for UI tests across
  all lint crates.

## 2. Core lint delivery

### 2.1. Lint crate template

- [x] 2.1.1. Establish the lint crate template with shared dependencies and UI
  test harness boilerplate.

### 2.2. Core lint implementations

- [x] 2.2.1. Implement `function_attrs_follow_docs` with targeted UI scenarios.
- [x] 2.2.2. Implement `no_expect_outside_tests` with context-aware diagnostics.
- [x] 2.2.3. Implement `module_must_have_inner_docs` ensuring each module opens
  with an inner doc comment, including UI scenarios for inline, file, and
  macro-generated modules plus localised diagnostics and behaviour-driven
  coverage.
- [ ] 2.2.4. Implement `public_fn_must_have_docs` using effective visibility
  data.
- [x] 2.2.5. Implement `test_must_not_have_example` covering code-fence
  heuristics.
- [x] 2.2.6. Implement `module_max_lines` with configurable thresholds.
- [x] 2.2.7. Implement `conditional_max_n_branches` for complex predicates.
- [x] 2.2.8. Implement `no_unwrap_or_else_panic` with optional `clippy` helpers.

### 2.3. Localisation enablement

- [x] 2.3.1. Add `fluent-templates` and `once_cell` to the workspace
  dependencies and expose a shared `common::i18n` loader.
- [x] 2.3.2. Create the `locales/` resource tree with an `en-GB` fallback and
  secondary `cy`/`gd` language samples covering every lint slug.
- [x] 2.3.3. Refactor lint diagnostics to source primary messages, notes, and
  help text from Fluent bundles with structured arguments.
- [x] 2.3.4. Allow locale selection via `DYLINT_LOCALE` and `dylint.toml`, and
  add UI smoke tests that run under at least one non-English locale.

## 3. Aggregated packaging, installer, and unified CLI

### 3.1. Suite assembly

- [x] 3.1.1. Assemble the `whitaker_suite` cdylib using constituent features and
  combined lint pass wiring.

### 3.2. Installer CLI

- [x] 3.2.1. Implement the installer CLI that builds, links, and stages all lint
  libraries.
- [x] 3.2.2. Add `test-support` Cargo feature exposing `StubExecutor` and
  `StubMismatch` for external test suites. See `installer/Cargo.toml` feature
  documentation for usage guidance and caveats.

### 3.3. Documentation examples

- [x] 3.3.1. Provide consumer guidance and workspace metadata examples in
  documentation.

### 3.4. Prebuilt lint libraries

- [x] 3.4.1. Define artefact naming, manifest schema, and verification policy.
  See `docs/adr-001-prebuilt-dylint-libraries.md`.
- [x] 3.4.2. Build CI automation to compile lint libraries for the supported
  target matrix and publish `.tar.zst` assets to the rolling release. See
  `docs/adr-001-prebuilt-dylint-libraries.md` §Decision outcome / proposed
  direction.
- [x] 3.4.3. Emit `manifest.json` for each artefact with git SHA, toolchain,
  target triple, build time, and SHA256. See
  `docs/adr-001-prebuilt-dylint-libraries.md` §Decision outcome / proposed
  direction.
- [x] 3.4.4. Extend the installer to download and verify prebuilt artefacts
  before local compilation, falling back on failure. See
  `docs/adr-001-prebuilt-dylint-libraries.md` §Decision outcome / proposed
  direction.
- [x] 3.4.5. Extract libraries to
  `~/.local/share/whitaker/lints/<toolchain>/<target>/lib` and set
  `DYLINT_LIBRARY_PATH`. See `docs/adr-001-prebuilt-dylint-libraries.md`
  §Decision outcome / proposed direction.
- [x] 3.4.6. Record download-versus-build rates and total installation time.
  See `docs/adr-001-prebuilt-dylint-libraries.md` §Migration plan.

### 3.5. Unified CLI foundation

- [ ] 3.5.1. Add a root `whitaker` binary and move the current installer
  behaviour behind an internal library boundary. See
  [Whitaker CLI design](whitaker-cli-design.md) §Public CLI surface and
  §Compatibility and migration. Requires 3.2.1.
- [ ] 3.5.2. Publish `whitaker` release artefacts with `cargo-binstall`
  metadata that mirror the existing installer packaging flow. See
  [Whitaker CLI design](whitaker-cli-design.md) §Public CLI surface and
  §Compatibility and migration. Requires 3.5.1 and 4.3.1.

### 3.6. Rule selection and configuration model

- [ ] 3.6.1. Assign stable rule codes, family selectors, and `DEFAULT`/`ALL`
  semantics for the core lint suite. See
  [Whitaker CLI design](whitaker-cli-design.md) §Rule identifiers and selection
  model. Requires 3.5.1.
- [ ] 3.6.2. Implement selector precedence across configuration and CLI flags,
  including the curated `--experimental` behaviour for implicit and explicit
  selections. See [Whitaker CLI design](whitaker-cli-design.md) §Rule
  identifiers and selection model. Requires 3.6.1.
- [ ] 3.6.3. Adopt `ortho_config` for CLI, environment, and `whitaker.toml`
  loading, while retaining one-release compatibility with `dylint.toml` and
  legacy environment keys. See [Whitaker CLI design](whitaker-cli-design.md)
  §Configuration model. Requires 3.5.1.
- [ ] 3.6.4. Add shared `--locale`, `--colour`, and `--progress` controls, and
  ensure the merged config surface remains localizable and accessible. See
  [Whitaker CLI design](whitaker-cli-design.md) §Accessibility and localization
  requirements and §Configuration model. Requires 3.6.3 and 2.3.4.

### 3.7. Unified installation and bundle state

- [ ] 3.7.1. Move dependency repair, toolchain provisioning, and bundle
  installation into `whitaker install`, including `--offline`,
  `--build-from-source`, and `--toolchain` flows. See
  [Whitaker CLI design](whitaker-cli-design.md) §`whitaker install`. Requires
  3.5.1, 3.4.5, and 4.3.2.
- [ ] 3.7.2. Resolve source builds against the CLI release version or an
  explicit revision instead of cloning `main`, and record source provenance for
  installed bundles. See [Whitaker CLI design](whitaker-cli-design.md)
  §`whitaker install`. Requires 3.7.1.
- [ ] 3.7.3. Add per-bundle manifest files with schema version, Whitaker
  version, source SHA, build date, toolchain, target, origin, and bundle-kind
  metadata. See [Whitaker CLI design](whitaker-cli-design.md) §Bundle manifests
  and `whitaker ls`. Requires 3.7.1.

### 3.8. Status, diagnostics, and repair commands

- [ ] 3.8.1. Implement `whitaker ls` text output with installed bundle
  metadata, the effective config path, and per-rule enablement states. See
  [Whitaker CLI design](whitaker-cli-design.md) §Bundle manifests and
  `whitaker ls`. Requires 3.6.2 and 3.7.3.
- [ ] 3.8.2. Add `--json` to `whitaker ls` and `whitaker doctor`, using stable
  machine-readable fields that remain untranslated across locales. See
  [Whitaker CLI design](whitaker-cli-design.md) §Accessibility and localization
  requirements and §`whitaker doctor`. Requires 3.8.1.
- [ ] 3.8.3. Record structured dependency-install and bundle-build failures
  with timestamps, phase metadata, stderr tails, advice, and log-path
  references. See [Whitaker CLI design](whitaker-cli-design.md) §Failure
  recording. Requires 3.7.1.
- [ ] 3.8.4. Implement `whitaker doctor` to summarize configuration,
  toolchains, dependencies, prebuilt availability, bundle-version drift,
  selected-lint coverage, and recent failures. See
  [Whitaker CLI design](whitaker-cli-design.md) §`whitaker doctor` and §Failure
  recording. Requires 3.7.3 and 3.8.3.
- [ ] 3.8.5. Add behaviour coverage for locale overrides, plain-progress
  output, and JSON parity between text and machine-readable status commands.
  See [Whitaker CLI design](whitaker-cli-design.md) §Accessibility and
  localization requirements. Requires 3.6.4, 3.8.2, and 3.8.4.

### 3.9. Compatibility release and documentation migration

- [ ] 3.9.1. Ship a compatibility release where `whitaker-installer`
  dispatches to `whitaker install` with a deprecation notice and `whitaker ls`
  accepts `list` as an alias. See [Whitaker CLI design](whitaker-cli-design.md)
  §Public CLI surface and §Compatibility and migration. Requires 3.7.1 and
  3.8.1.
- [ ] 3.9.2. Update `docs/users-guide.md`, `docs/developers-guide.md`,
  `docs/publishing.md`, and installer-facing workflow documentation to point to
  the unified CLI and the new configuration surface. See
  [Whitaker CLI design](whitaker-cli-design.md) §Configuration model,
  §Compatibility and migration, and §Expected outcomes. Requires 3.6.3 and
  3.9.1.
- [ ] 3.9.3. Remove wrapper script generation, `whitaker-ls`, and
  installer-first references once the compatibility release window closes. See
  [Whitaker CLI design](whitaker-cli-design.md) §Public CLI surface and
  §Compatibility and migration. Requires 3.9.1 and 3.9.2.

## 4. Quality gates and automation

### 4.1. Continuous integration

- [ ] 4.1.1. Configure CI workflows for fmt, clippy, dylint runs, and per-crate
  UI tests on a multi-OS matrix.
- [ ] 4.1.2. Add markdownlint, nixie, and other doc/tooling checks to the
  pipeline.
- [ ] 4.1.3. Enforce lint-level deny rules and fail builds on warnings across
  the workspace.

### 4.2. Release metadata

- [x] 4.2.1. Add cargo-binstall metadata to `installer/Cargo.toml` for
  published release artefacts.

### 4.3. Release workflow

- [x] 4.3.1. Implement a release workflow that builds `whitaker-installer` for
  each supported target, packages `.tgz`/`.zip` archives, and uploads them to
  GitHub Releases tagged `v<version>`.
- [x] 4.3.2. Publish repository-hosted dependency binaries for
  `cargo-dylint` and `dylint-link`, and teach the installer to prefer them
  before Cargo-based installation. (`design.execplan.dependency-binary`,
  `docs/execplans/install-dependency-binaries.md` Stage C-D)

## 5. Experimental Bumpy Road lint

### 5.1. Signal and detection pipeline

- [x] 5.1.1. Implement the per-line complexity signal builder and smoothing
  window logic.
- [x] 5.1.2. Detect bump intervals, surface diagnostics with labelled spans, and
  add configuration options.
- [x] 5.1.3. Ship UI coverage for positive and negative Bumpy Road scenarios and
  gate behind a feature flag.

## 6. Brain trust lints (`brain_type` and `brain_trait`)

### 6.1. Shared cohesion analysis

- [x] 6.1.1. Add a shared LCOM4 helper in `common` that builds a method
  graph and returns connected component counts. See
  [brain trust lints design](brain-trust-lints-design.md) §Cohesion analysis
  (LCOM4). Requires 1.1.1.
- [x] 6.1.2. Define method metadata extraction for field access and method
  calls, including macro-span filtering. See
  [brain trust lints design](brain-trust-lints-design.md) §Cohesion analysis
  (LCOM4) and §Implementation approach.

### 6.2. `brain_type` lint

- [x] 6.2.1. Implement metric collection for WMC (cognitive complexity), brain
  method detection (CC + LOC), LCOM4, and foreign reach. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_type`
  signals. Requires 6.1.1.
- [x] 6.2.2. Implement threshold evaluation and escalation rules, and surface
  measured values in diagnostics. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_type` rule
  set (initial defaults). Requires 6.2.1.
- [x] 6.2.3. Ensure macro-expanded spans are excluded or capped during CC
  calculation. See [brain trust lints design](brain-trust-lints-design.md)
  §Metric collection.

### 6.3. `brain_trait` lint

- [x] 6.3.1. Implement trait item counting, default method CC aggregation, and
  implementor burden metrics. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_trait`
  signals.
- [x] 6.3.2. Apply warning and escalation thresholds, and surface measured
  values in diagnostics. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_trait` rule
  set (initial defaults).

### 6.4. Decomposition advice

- [x] 6.4.1. Build feature vectors for methods and cluster with community
  detection to form decomposition suggestions. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.2.1.
- [x] 6.4.2. Emit concise diagnostic notes mapping clusters to extraction
  suggestions, capped for large types. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition advice.
- [x] 6.4.3. Use Verus to prove `cosine_threshold_met`'s cross-multiplied
  threshold check is equivalent to `cosine >= 0.20` for non-zero norms and
  cannot divide by zero. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.
- [x] 6.4.4. Use Verus to prove `dot_product` and `norm_squared` algebraic
  properties, including commutativity, non-negativity, and zero-result
  behaviour when vectors have no overlapping positive features. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.
- [x] 6.4.5. Use Kani to verify `build_adjacency` preserves similarity edges,
  keeps neighbour indices in bounds, and produces symmetric adjacency lists.
  See [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.
- [ ] 6.4.6. Use Kani to verify `propagate_labels` preserves valid label
  indices, returns one label per input vector, and terminates within the
  supplied iteration bound. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.

### 6.5. SARIF output

- [ ] 6.5.1. Collect brain trust diagnostics into a SARIF 2.1.0 emitter that is
  opt-in and English-only for tool ingestion. See
  [brain trust lints design](brain-trust-lints-design.md) §SARIF output.

### 6.6. Configuration, localization, and tests

- [ ] 6.6.1. Add `brain_type` and `brain_trait` configuration sections to
  `whitaker.toml` with documented defaults. See
  [brain trust lints design](brain-trust-lints-design.md) §Configuration,
  localization, and testing.
- [ ] 6.6.2. Add Fluent localization entries for both lints. See
  [brain trust lints design](brain-trust-lints-design.md) §Configuration,
  localization, and testing.
- [ ] 6.6.3. Add UI tests for positive and negative cases under
  `crates/brain_type/ui/` and `crates/brain_trait/ui/`. See
  [brain trust lints design](brain-trust-lints-design.md) §Configuration,
  localization, and testing.

### 6.7. Documentation

- [ ] 6.7.1. Update `docs/users-guide.md` with lint descriptions, configuration
  keys, and SARIF usage. See
  [brain trust lints design](brain-trust-lints-design.md) §Configuration,
  localization, and testing.

## 7. Clone detector pipeline

### 7.1. SARIF model and shared types

- [x] 7.1.1. Create the `whitaker_sarif` crate with SARIF 2.1.0 models,
  builders, and merge logic. See
  [clone detector design](whitaker-clone-detector-design.md) §Crate
  responsibilities and §SARIF schema and mapping.

### 7.2. Token pass (Type-1/Type-2)

- [x] 7.2.1. Implement `rustc_lexer` normalization, k-shingling, winnowing, and
  Rabin-Karp hashing. See
  [clone detector design](whitaker-clone-detector-design.md) §Pass A: token
  engine (rustc_lexer).
- [x] 7.2.2. Implement MinHash + LSH candidate generation with configurable
  bands and rows. See
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
- [x] 7.2.3. Emit SARIF run 0 for accepted Type-1 and Type-2 pairs with stable
  fingerprints and spans. See
  [clone detector design](whitaker-clone-detector-design.md) §SARIF emission
  (Run 0). Requires 7.1.1.
- [x] 7.2.4. Add sidecar proof workflows and Makefile targets for clone-detector
  Verus and Kani checks. Requires 7.2.2. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md).
- [x] 7.2.5. Use Verus to prove `LshConfig::new` rejects zero `bands` and
  `rows`, and enforces `bands * rows == MINHASH_SIZE`. Requires 7.2.4. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
- [ ] 7.2.6. Use Verus to prove `CandidatePair::new` canonicalizes fragment
  ordering and suppresses self-pairs. Requires 7.2.4. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
- [ ] 7.2.7. Use Kani to verify bounded `MinHasher::sketch` invariants,
  including deterministic output, duplicate-hash insensitivity, and empty-input
  failure. Requires 7.2.4. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
- [ ] 7.2.8. Use Kani to verify bounded `LshIndex` invariants, including no
  self-pairs, canonical pair ordering, repeated-band deduplication, and
  insertion-order independence. Requires 7.2.4. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.

### 7.3. AST refinement (Type-3)

- [ ] 7.3.1. Map candidate spans to `ra_ap_syntax` nodes and extract AST feature
  vectors. See [clone detector design](whitaker-clone-detector-design.md) §Pass
  B: AST engine (ra_ap_syntax).
- [ ] 7.3.2. Score Type-3 similarity and update SARIF run 1 with cosine and AST
  hash metadata. See [clone detector design](whitaker-clone-detector-design.md)
  §Scoring and acceptance (Type-3) and §SARIF update (Run 1).

### 7.4. CLI surface

- [ ] 7.4.1. Implement `cargo whitaker clones` with `scan`, `refine`, `report`,
  and `clean` subcommands. See
  [clone detector design](whitaker-clone-detector-design.md) §CLI surface.
- [ ] 7.4.2. Emit optional HTML reports and anchor deep links in SARIF results.
  See [clone detector design](whitaker-clone-detector-design.md) §Grouping and
  reporting.

### 7.5. Dylint integration

- [ ] 7.5.1. Implement the `clone_detected` lint to load SARIF results, filter
  to current crate files, and emit diagnostics with `help` guidance. See
  [clone detector design](whitaker-clone-detector-design.md) §Dylint
  integration (`clone_detected` lint).
- [ ] 7.5.2. Honour `#[allow(whitaker::clone_detected)]` and per-file allowlists
  in configuration. See
  [clone detector design](whitaker-clone-detector-design.md) §Dylint
  integration (`clone_detected` lint).

### 7.6. Incrementality and caching

- [ ] 7.6.1. Implement the per-file cache in `target/whitaker/clones-cache.bin`
  with config hashing and shard indexing. See
  [clone detector design](whitaker-clone-detector-design.md) §Incrementality
  and caching.

### 7.7. Testing and validation

- [ ] 7.7.1. Add SARIF golden tests with deterministic ordering. See
  [clone detector design](whitaker-clone-detector-design.md) §Testing strategy.
- [ ] 7.7.2. Add Dylint UI tests that consume pre-baked SARIF results. See
  [clone detector design](whitaker-clone-detector-design.md) §Testing strategy.
- [ ] 7.7.3. Add property tests for normalization invariants. See
  [clone detector design](whitaker-clone-detector-design.md) §Testing strategy.

### 7.8. Continuous integration

- [ ] 7.8.1. Add CI jobs to run token and AST passes and verify stable SARIF
  output. See [clone detector design](whitaker-clone-detector-design.md)
  §Acceptance criteria.

## 8. `rstest` fixture and test hygiene lints

### 8.1. Shared foundations

- [x] 8.1.1. Add shared `rstest` test and fixture detection helpers in `common`
  for attribute-based detection and optional expansion-trace fallback. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint A: call-site fixture extraction and §Integration constraints. Requires
  1.1.1.
- [ ] 8.1.2. Add shared user-editable span recovery helpers for macro-heavy
  test code paths, and use them to avoid diagnostics on macro-only glue. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Integration constraints and §Lint A: call-site fixture extraction. Requires
  1.1.1.
- [ ] 8.1.3. Add shared argument and paragraph fingerprint data models for
  deterministic grouping across tests. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint A: call-site fixture extraction and §Lint C: repeated fixture
  paragraph detection. Requires 8.1.1.

### 8.2. `rstest_helper_should_be_fixture` lint

- [ ] 8.2.1. Create the `rstest_helper_should_be_fixture` lint crate, register
  `RSTEST_HELPER_SHOULD_BE_FIXTURE`, and wire configuration loading defaults.
  See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint A: call-site fixture extraction. Requires 8.1.1 and 8.1.3.
- [ ] 8.2.2. Implement call-site collection in `#[rstest]` tests, including
  fixture-local classification and constant-aware argument fingerprinting. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint A: call-site fixture extraction. Requires 8.2.1.
- [ ] 8.2.3. Implement crate-post aggregation thresholds and actionable
  diagnostics with `span_lint_hir_and_then`. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint A: call-site fixture extraction. Requires 8.2.2 and 8.1.2.
- [ ] 8.2.4. Add UI pass/fail coverage for repeated no-arg helpers,
  repeated fixture/constant argument helpers, and non-trigger cases. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint A: call-site fixture extraction. Requires 8.2.3 and 1.2.1.

### 8.3. `single_binding_paragraph` lint

- [ ] 8.3.1. Create the `single_binding_paragraph` lint crate and implement the
  local statement I/O model (`defs`, `uses`, `muts`, and control-flow guards).
  See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint B: single-binding paragraph detection. Requires 8.1.2.
- [ ] 8.3.2. Implement the contiguous backward-slice algorithm, single-output
  checks, and configurable limits for paragraph length and external inputs. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint B: single-binding paragraph detection. Requires 8.3.1.
- [ ] 8.3.3. Emit diagnostics with bounded spans and explanatory notes for
  extraction candidates. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint B: single-binding paragraph detection. Requires 8.3.2.
- [ ] 8.3.4. Add UI pass/fail coverage for simple paragraphs, too-short
  candidates, control-flow-containing blocks, and intermediate reuse cases. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint B: single-binding paragraph detection. Requires 8.3.3 and 1.2.1.

### 8.4. `rstest_paragraph_should_be_fixture` lint

- [ ] 8.4.1. Create the `rstest_paragraph_should_be_fixture` lint crate and
  register `RSTEST_PARAGRAPH_SHOULD_BE_FIXTURE` with configurable assertion and
  input constraints. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint C: repeated fixture paragraph detection. Requires 8.1.1 and 8.1.3.
- [ ] 8.4.2. Reuse lint B candidate generation and implement assertion-free
  filtering plus fixture-or-constant input validation. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint C: repeated fixture paragraph detection. Requires 8.4.1 and 8.3.2.
- [ ] 8.4.3. Implement cross-test paragraph grouping, identical input
  fingerprint checks, and crate-post emission controls (`emit_once_per_group`).
  See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint C: repeated fixture paragraph detection. Requires 8.4.2.
- [ ] 8.4.4. Add UI pass/fail coverage for repeated setup paragraphs, assertion
  presence, differing inputs, and non-fixture-derived inputs. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Lint C: repeated fixture paragraph detection. Requires 8.4.3 and 1.2.1.

### 8.5. Integration, documentation, and promotion

- [ ] 8.5.1. Add all three lints to the experimental set with feature-gated
  suite wiring and default configuration stanzas. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Integration constraints and §Comparison and rollout guidance. Requires
  8.2.4, 8.3.4, and 8.4.4.
- [ ] 8.5.2. Add Fluent localization entries and diagnostic argument mappings
  for all three lint slugs. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Integration constraints. Requires 8.5.1 and 2.3.3.
- [ ] 8.5.3. Update user documentation with lint intent, configuration keys,
  and fixture-extraction remediation guidance. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Comparison and rollout guidance. Requires 8.5.1.
- [ ] 8.5.4. Define promotion criteria from experimental to standard based on
  UI stability and false-positive tuning across internal repositories. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
   §Integration constraints. Requires 8.5.1.


## 9. Ownership shape lints


### 9.1. Shared foundations

- [ ] 9.1.1. Add `common::ownership_shape` model types, evaluation helpers, and
  diagnostic argument builders. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Shared
  implementation approach and §Diagnostics and localisation. Requires 1.1.1.
- [ ] 9.1.2. Add resolved-path classifiers for clone-like operations and
  wrapper constructors, following the `def_path_str` plus parsed-segment
  pattern used by `no_std_fs_operations`. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Resolution
  strategy and §Common helper requirements. Requires 9.1.1.
- [ ] 9.1.3. Add exact borrow mappings for the initial concrete type set. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Lint 2:
  `owned_param_causes_clone` and §Common helper requirements. Requires 9.1.1.
- [ ] 9.1.4. Add crate-local MIR summary helpers for local uses, escapes, and
  boundary classification. See
  [ownership shape lints design](ownership-shape-lints-design.md) §HIR
  prefilter, MIR confirmation and §Worked exception model: Servo-style code
  must stay quiet. Requires 9.1.1 and 9.1.2.

### 9.2. `clone_only_used_by_borrow`

- [ ] 9.2.1. Create the lint crate and register
  `CLONE_ONLY_USED_BY_BORROW`. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Lint 1:
  `clone_only_used_by_borrow`. Requires 9.1.1.
- [ ] 9.2.2. Implement HIR candidate prefiltering for `clone` and `to_owned`
  forms. See [ownership shape lints design](ownership-shape-lints-design.md)
  §Lint 1: `clone_only_used_by_borrow` and §HIR prefilter, MIR confirmation.
  Requires 9.2.1 and 9.1.2.
- [ ] 9.2.3. Implement MIR use classification and original-place conflict
  checks. See [ownership shape lints design](ownership-shape-lints-design.md)
  §Lint 1: `clone_only_used_by_borrow`. Requires 9.2.2 and 9.1.4.
- [ ] 9.2.4. Add machine-applicable suggestions for direct-call and simple-let
  forms. See [ownership shape lints design](ownership-shape-lints-design.md)
  §Suggestion policy. Requires 9.2.3.
- [ ] 9.2.5. Add UI pass and fail coverage, including macro and mutable-conflict
  cases. See [ownership shape lints design](ownership-shape-lints-design.md)
  §Testing strategy. Requires 9.2.3 and 1.2.1.


### 9.4. `local_shared_ownership`

- [ ] 9.4.1. Create the lint crate and register `LOCAL_SHARED_OWNERSHIP` behind
  an experimental feature. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Lint 3:
  `local_shared_ownership` and §Rollout plan. Requires 9.1.1.
- [ ] 9.4.2. Implement wrapper-construction detection and non-escape
  classification for `Rc`, `Arc`, interior mutability, and shared-mutable
  combinations. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Scope and
  §Detection model. Requires 9.4.1, 9.1.2, and 9.1.4.
- [ ] 9.4.3. Implement callback, async, thread, trait-surface, and external-API
  suppression. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Servo-style
  exemptions and §False-positive controls. Requires 9.4.2.
- [ ] 9.4.4. Add diagnostic classes for interior-mutability-only,
  shared-handle-only, and shared-mutable-wrapper cases. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Diagnostic
  classes and §Suggestion policy. Requires 9.4.3.
- [ ] 9.4.5. Add Servo-style regression fixtures and framework-boundary
  negatives. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Worked
  exception model: Servo-style code must stay quiet and §Testing strategy.
  Requires 9.4.4 and 1.2.1.


### 9.5. Localisation, documentation, and promotion

- [ ] 9.5.1. Add Fluent entries and diagnostic argument mappings for all three
  lints. See [ownership shape lints design](ownership-shape-lints-design.md)
  §Diagnostics and localisation. Requires 9.2.5, 9.3.5, 9.4.5, and 2.3.3.
- [ ] 9.5.2. Add Welsh and Gaelic smoke coverage for the ownership-shape
  diagnostics. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Diagnostics
  and localisation and §Testing strategy. Requires 9.5.1.
- [ ] 9.5.3. Update `docs/users-guide.md` and `docs/developers-guide.md` with
  lint intent, configuration, and rollout guidance for the ownership-shape
  suite. See [ownership shape lints design](ownership-shape-lints-design.md)
  §Configuration and §Rollout plan. Requires 9.5.1.
- [ ] 9.5.4. Define promotion criteria from experimental to standard based on
  UI stability and false-positive tuning across representative repositories.
  See [ownership shape lints design](ownership-shape-lints-design.md) §Rollout
  plan. Requires 9.5.1 and 9.5.2.
### 9.3. `owned_param_causes_clone`

- [ ] 9.3.1. Create the lint crate and register `OWNED_PARAM_CAUSES_CLONE`.
  See [ownership shape lints design](ownership-shape-lints-design.md) §Lint 2:
  `owned_param_causes_clone`. Requires 9.1.1.
- [ ] 9.3.2. Implement local call-site clone-pressure collection that records
  callee identity, argument index, source shape, and retained-source evidence.
  See [ownership shape lints design](ownership-shape-lints-design.md) §Pass A:
  collect clone-pressure evidence at call sites. Requires 9.3.1, 9.1.2, and
  9.1.4.
- [ ] 9.3.3. Implement callee parameter summaries and exported, trait, FFI, and
  async suppression rules. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Pass B:
  summarise callee parameter usage and §Exemptions. Requires 9.3.2.
- [ ] 9.3.4. Add exact borrow-type help for the initial mapping set without
  rewriting unsupported signatures. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Exact borrow
  mappings and §Diagnostics. Requires 9.3.3 and 9.1.3.
- [ ] 9.3.5. Add UI coverage for private, exported, trait, async, and
  non-trigger scenarios. See
  [ownership shape lints design](ownership-shape-lints-design.md) §Testing
  strategy. Requires 9.3.4 and 1.2.1.
