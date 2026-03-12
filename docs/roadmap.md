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

## 3. Aggregated packaging and installer

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
- [ ] 6.4.2. Emit concise diagnostic notes mapping clusters to extraction
  suggestions, capped for large types. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition advice.
- [ ] 6.4.3. Use Verus to prove `cosine_threshold_met`'s cross-multiplied
  threshold check is equivalent to `cosine >= 0.20` for non-zero norms and
  cannot divide by zero. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.
- [ ] 6.4.4. Use Verus to prove `dot_product` and `norm_squared` algebraic
  properties, including commutativity, non-negativity, and zero-result
  behaviour when vectors have no overlapping positive features. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.
- [ ] 6.4.5. Use Kani to verify `build_adjacency` preserves similarity edges,
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

- [ ] 7.2.1. Implement `rustc_lexer` normalization, k-shingling, winnowing, and
  Rabin-Karp hashing. See
  [clone detector design](whitaker-clone-detector-design.md) §Pass A: token
  engine (rustc_lexer).
- [ ] 7.2.2. Implement MinHash + LSH candidate generation with configurable
  bands and rows. See
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
- [ ] 7.2.3. Emit SARIF run 0 for accepted Type-1 and Type-2 pairs with stable
  fingerprints and spans. See
  [clone detector design](whitaker-clone-detector-design.md) §SARIF emission
  (Run 0). Requires 7.1.1.

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

- [ ] 8.1.1. Add shared `rstest` test and fixture detection helpers in `common`
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
