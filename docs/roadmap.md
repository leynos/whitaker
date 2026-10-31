# Development roadmap

## 0. Repository scaffolding

### 0.1. Workspace foundations

- [ ] 0.1.1. Initialize the Cargo workspace with workspace members and
  `rust-toolchain` pin.
- [ ] 0.1.2. Add baseline project metadata (`README`, `LICENSE`, contributing
  guide, `CODEOWNERS`).
- [ ] 0.1.3. Establish Makefile or justfile targets for fmt, lint, test, and UI
  test orchestration. Requires 0.1.1.

## 1. Common infrastructure

### 1.1. Common crate helpers

- [x] 1.1.1. Implement the `common` crate helpers for attributes, context
  detection, spans, and diagnostics.
- [x] 1.1.2. Wire shared configuration loading with
  `dylint_linting::config_or_default` and serde structs. Requires 1.1.1.

### 1.2. UI test harness

- [x] 1.2.1. Integrate `dylint_testing` harness boilerplate for UI tests across
  all lint crates.

### 1.3. Conditional Dylint `expect` support

- [ ] 1.3.1. Add the `crates/whitaker_support_macros` proc-macro crate and
  implement `#[whitaker_support::dylint_expect(...)]` with `lib`, `lints`, and
  optional `reason` arguments. See
  [ADR 002](adr-002-dylint-expect-attribute-macro.md) §Decision outcome /
  proposed direction and §Functional requirements.
- [ ] 1.3.2. Add the `crates/whitaker_support` re-export crate, wire both
  support crates into the workspace, and document the supported attribute
  surface with API examples. See
  [ADR 002](adr-002-dylint-expect-attribute-macro.md) §Decision outcome /
  proposed direction and §Migration plan. Requires 1.3.1.
- [ ] 1.3.3. Add compatibility coverage that proves the attribute stays warning
  free in non-Dylint builds, Clippy runs, and Dylint runs with the matching
  `dylint_lib` value. See [ADR 002](adr-002-dylint-expect-attribute-macro.md)
  §Technical requirements and §Migration plan. Requires 1.3.2.
- [ ] 1.3.4. Document intended usage, narrow-scope review guidance, and
  pre-expansion limitations for the conditional `expect` attribute. See
  [ADR 002](adr-002-dylint-expect-attribute-macro.md) §Decision outcome /
  proposed direction and §Known risks and limitations. Requires 1.3.3.

## 2. Core lint delivery

### 2.1. Lint crate template

- [x] 2.1.1. Establish the lint crate template with shared dependencies and UI
  test harness boilerplate. Requires 1.1.1 and 1.2.1.

### 2.2. Core lint implementations

RFC 0003 work is in scope for source-authored free functions and supported
associated functions in configured Rust test-source paths during test-harness
compilation. It is out of scope for normal-target functions, generated
expansion items, closures, trait declarations without bodies, rustdoc JSON,
external source scanners, downstream `rstest-bdd` rollout, and the isolated
published-GPUI workspace invocation.

- [x] 2.2.1. Implement `function_attrs_follow_docs` with targeted UI scenarios.
  Requires 2.1.1.
- [x] 2.2.2. Implement `no_expect_outside_tests` with context-aware
  diagnostics. Requires 2.1.1.
- [x] 2.2.3. Implement `module_must_have_inner_docs` ensuring each module opens
  with an inner doc comment, including UI scenarios for inline, file, and
  macro-generated modules plus localized diagnostics and behaviour-driven
  coverage. Requires 2.1.1.
- [ ] 2.2.4. Implement `public_fn_must_have_docs` using effective visibility
  data. Requires 2.1.1.
- [x] 2.2.5. Implement `test_must_not_have_example` covering code-fence
  heuristics. Requires 2.1.1.
- [x] 2.2.6. Implement `module_max_lines` with configurable thresholds.
  Requires 1.1.2 and 2.1.1.
- [x] 2.2.7. Implement `conditional_max_n_branches` for complex predicates.
  Requires 2.1.1.
- [x] 2.2.8. Implement `no_unwrap_or_else_panic` with optional `clippy`
  helpers. Requires 2.1.1.
- [ ] 2.2.9. Replace ad hoc conditional `allow` and `expect` sequences in
  Whitaker-managed code with `#[whitaker_support::dylint_expect(...)]`, keeping
  suppressions narrowly scoped to the item they justify. See
  [ADR 002](adr-002-dylint-expect-attribute-macro.md) §Migration plan. Requires
  1.3.3.
- [ ] 2.2.10. Implement `string_continuation_style` as a post-expansion early
  lint, including source-preserving `concat!()` suggestions, format-capture and
  literal-context exemptions, localized diagnostics, and suite integration. See
  [RFC 0002](rfcs/0002-string-continuation-style.md).
  Requires 2.1.1 and 2.3.4.
- [ ] 2.2.11. Deliver the `missing_docs_in_test_functions` pre-expansion AST
  pass and candidate-selection predicate for test-harness compilations. See
  [RFC 0003](rfcs/0003-missing-docs-in-test-functions.md) §§6-7. Requires 2.1.1.
  - Success: diagnostics occur only for source-authored, undocumented
    functions in test harnesses, with an identifier span when available; both
    `///` and `#[doc = "..."]` are recognized as documentation.
- [ ] 2.2.12. Add typed include/exclude configuration with workspace-relative,
  slash-normalized path matching and safe invalid-pattern handling. See
  [RFC 0003](rfcs/0003-missing-docs-in-test-functions.md) §6. Requires 1.1.2
  and 2.2.11.
  - Success: exclusions take precedence, separators behave equivalently,
    non-relative or invalid patterns produce actionable diagnostics, and no
    pattern can widen enforcement beyond the workspace.
- [ ] 2.2.13. Register `missing_docs_in_test_functions` in the normal Whitaker
  suite and expose it through the standard suite distribution. See
  [RFC 0003](rfcs/0003-missing-docs-in-test-functions.md) §§6-7. Requires
  2.1.1, 2.2.11, and 2.2.12.
  - Success: one suite invocation discovers the lint and applies its configured
    path policy without a bespoke runner.
- [ ] 2.2.14. Localize the lint's diagnostic, help text, and UI messages through
  Whitaker's existing localization infrastructure. See
  [RFC 0003](rfcs/0003-missing-docs-in-test-functions.md) §§6-7. Requires 2.3.4
  and 2.2.13.
  - Success: the default locale and at least one non-English locale render the
    same rule outcome with translated human-facing text.
- [ ] 2.2.15. Add UI fixtures using a deterministic pre-baked SARIF diagnostic-
  input contract for undocumented included functions and compiler-supported
  associated functions, `///` and `#[doc = "..."]` documentation, procedural
  attributes, excluded paths, normal library sources compiled under a harness,
  generated-code immunity, and the two-attribute suppression workaround for
  `missing_docs_in_test_functions`. See
  [RFC 0003](rfcs/0003-missing-docs-in-test-functions.md) §§7 and 9. Requires
  1.1.2, 2.1.1, 2.2.11, 2.2.12, and 2.3.4.
  - Success: at least nine pass/fail cases cover each listed behaviour,
    including an undocumented compiler-supported associated function whose
    diagnostic targets its identifier; the suppression fixture includes an
    unsuppressed control function and proves that `#[allow(unknown_lints)]`
    plus `#[allow(missing_docs_in_test_functions)]` suppresses only the
    annotated item.
- [ ] 2.2.16. Add substantive Rust `proptest` coverage for the pure
  candidate-selection and path-configuration logic. See
  [RFC 0003](rfcs/0003-missing-docs-in-test-functions.md) §9. Requires 1.1.2,
  2.1.1, and 2.2.12.
  - Success: generated combinations cover harness status, expansion provenance,
    documentation presence, include/exclude matches, slash and backslash
    separators, and valid/invalid patterns, asserting the diagnostic predicate,
    exclusion precedence, normalization, and safe rejection.

### 2.3. Localization enablement

- [x] 2.3.1. Add `fluent-templates` and `once_cell` to the workspace
  dependencies and expose a shared `common::i18n` loader. Requires 1.1.1.
- [x] 2.3.2. Create the `locales/` resource tree with an `en-GB` fallback and
  secondary `cy`/`gd` language samples covering every lint slug. Requires 2.3.1.
- [x] 2.3.3. Refactor lint diagnostics to source primary messages, notes, and
  help text from Fluent bundles with structured arguments. Requires 2.3.2.
- [x] 2.3.4. Allow locale selection via `DYLINT_LOCALE` and `dylint.toml`, and
  add UI smoke tests that run under at least one non-English locale. Requires
  2.3.3.

## 3. Aggregated packaging, installer, and unified CLI

### 3.1. Suite assembly

- [x] 3.1.1. Assemble the `whitaker_suite` cdylib using constituent features and
  combined lint pass wiring.

### 3.2. Installer CLI

- [x] 3.2.1. Implement the installer CLI that builds, links, and stages all
  lint libraries. Requires 3.1.1.
- [x] 3.2.2. Add `test-support` Cargo feature exposing `StubExecutor` and
  `StubMismatch` for external test suites. See `installer/Cargo.toml` feature
  documentation for usage guidance and caveats. Requires 3.2.1.

### 3.3. Documentation examples

- [x] 3.3.1. Provide consumer guidance and workspace metadata examples in
  documentation. Requires 3.2.1.

### 3.4. Prebuilt lint libraries

- [x] 3.4.1. Define artefact naming, manifest schema, and verification policy.
  See `docs/adr-001-prebuilt-dylint-libraries.md`.
- [x] 3.4.2. Build CI automation to compile lint libraries for the supported
  target matrix and publish `.tar.zst` assets to the rolling release. See
  `docs/adr-001-prebuilt-dylint-libraries.md` §Decision outcome / proposed
  direction. Requires 3.1.1 and 3.4.1.
- [x] 3.4.3. Emit `manifest.json` for each artefact with git SHA, toolchain,
  target triple, build time, and SHA256. See
  `docs/adr-001-prebuilt-dylint-libraries.md` §Decision outcome / proposed
  direction. Requires 3.4.2.
- [x] 3.4.4. Extend the installer to download and verify prebuilt artefacts
  before local compilation, falling back on failure. See
  `docs/adr-001-prebuilt-dylint-libraries.md` §Decision outcome / proposed
  direction. Requires 3.2.1 and 3.4.3.
- [x] 3.4.5. Extract libraries to
  `~/.local/share/whitaker/lints/<toolchain>/<target>/lib` and set
  `DYLINT_LIBRARY_PATH`. See `docs/adr-001-prebuilt-dylint-libraries.md`
  §Decision outcome / proposed direction. Requires 3.4.4.
- [x] 3.4.6. Record download-versus-build rates and total installation time.
  See `docs/adr-001-prebuilt-dylint-libraries.md` §Migration plan. Requires
  3.4.5.

### 3.5. Unified CLI foundation

- [ ] 3.5.1. Add a root `whitaker` binary and move the current installer
  behaviour behind an internal library boundary. See
  [Whitaker CLI design](whitaker-cli-design.md) §Public CLI surface and
  §Compatibility and migration. Requires 3.2.1.
- [ ] 3.5.2. Implement `whitaker check` as the default linting path, including
  lazy dependency repair, `--no-install`, forwarded cargo arguments after `--`,
  and operational error handling that preserves lint-failure exit semantics. See
  [Whitaker CLI design](whitaker-cli-design.md) §Public CLI surface and §
  `whitaker check`. Requires 3.6.2, 3.6.4, and 3.7.1.
- [ ] 3.5.3. Publish `whitaker` release artefacts with `cargo-binstall`
  metadata that mirror the existing installer packaging flow. See
  [Whitaker CLI design](whitaker-cli-design.md) §Public CLI surface and
  §Compatibility and migration. Requires 3.5.1, 4.2.1, and 4.3.1.

### 3.6. Rule selection and configuration model

- [ ] 3.6.1. Assign stable rule codes, family selectors, and `DEFAULT`/`ALL`
  semantics for the core lint suite. See
  [Whitaker CLI design](whitaker-cli-design.md) §Rule identifiers and selection
  model. Requires 3.5.1.
- [ ] 3.6.2. Implement selector precedence across configuration and CLI flags,
  including the curated `--experimental` behaviour for implicit and explicit
  selections. See [Whitaker CLI design](whitaker-cli-design.md) §Rule
  identifiers and selection model. Requires 3.6.1 and 3.6.3.
- [x] 3.6.3. Adopt `ortho_config` for CLI, environment, and `whitaker.toml`
  loading, while retaining one-release compatibility with `dylint.toml` and
  legacy environment keys. The configuration-only `whitaker-cli` package
  delivers this item without an executable, as the design specifies. Roadmap
  item 3.5.1 owns the root binary and its command integration, including the
  binary-level parse, stream and exit-status checks that this item does not
  claim. See [Whitaker CLI design](whitaker-cli-design.md) §Configuration model.
- [ ] 3.6.4. Add shared `--locale`, `--colour`, and `--progress` controls, and
  ensure the merged config surface remains localizable and accessible. See
  [Whitaker CLI design](whitaker-cli-design.md) §Accessibility and localization
  requirements and §Configuration model. Requires 2.3.4 and 3.6.3.

### 3.7. Unified installation and bundle state

- [ ] 3.7.1. Move dependency repair, toolchain provisioning, and bundle
  installation into `whitaker install`, including `--offline`,
  `--build-from-source`, and `--toolchain` flows. See
  [Whitaker CLI design](whitaker-cli-design.md) §`whitaker install`. Requires
  3.4.5, 3.5.1, and 4.3.2.
- [ ] 3.7.2. Resolve source builds against the CLI release version or an
  explicit revision instead of cloning `main`, and record source provenance for
  installed bundles. See [Whitaker CLI design](whitaker-cli-design.md) §
  `whitaker install`. Requires 3.7.1.
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
  requirements and §`whitaker doctor`. Requires 3.8.1 and 3.8.4.
- [ ] 3.8.3. Record structured dependency-install and bundle-build failures
  with timestamps, phase metadata, stderr tails, advice, and log-path
  references. See [Whitaker CLI design](whitaker-cli-design.md) §Failure
  recording. Requires 3.7.1.
- [ ] 3.8.4. Implement `whitaker doctor` to summarize configuration,
  toolchains, dependencies, prebuilt availability, bundle-version drift,
  selected-lint coverage, and recent failures. See
  [Whitaker CLI design](whitaker-cli-design.md) §`whitaker doctor` and §Failure
  recording. Requires 3.6.2, 3.7.3, and 3.8.3.
- [ ] 3.8.5. Add behaviour coverage for locale overrides, plain-progress
  output, and JSON parity between text and machine-readable status commands. See
  [Whitaker CLI design](whitaker-cli-design.md) §Accessibility and
  localization requirements. Requires 3.6.4 and 3.8.2.

### 3.9. Compatibility release and documentation migration

- [ ] 3.9.1. Ship a compatibility release where `whitaker-installer` dispatches
  to `whitaker install` with a deprecation notice and `whitaker ls` accepts
  `list` as an alias. See [Whitaker CLI design](whitaker-cli-design.md) §Public
  CLI surface and §Compatibility and migration. Requires 3.7.1 and 3.8.1.
- [ ] 3.9.2. Update `docs/users-guide.md`, `docs/developers-guide.md`,
  `docs/publishing.md`, and installer-facing workflow documentation to point to
  the unified CLI and the new configuration surface. See
  [Whitaker CLI design](whitaker-cli-design.md) §Configuration model,
  §Compatibility and migration, and §Expected outcomes. Requires 3.6.3 and
  3.9.1.
- [ ] 3.9.3. Remove wrapper script generation, `whitaker-ls`, and
  installer-first references once the compatibility release window closes. See
  [Whitaker CLI design](whitaker-cli-design.md) §Public CLI surface and
  §Compatibility and migration. Requires 3.9.2.

## 4. Quality gates and automation

### 4.1. Continuous integration

- [ ] 4.1.1. Configure CI workflows for fmt, clippy, dylint runs, and per-crate
  UI tests on a multi-OS matrix. Requires 0.1.3 and 1.2.1.
- [ ] 4.1.2. Add markdownlint, nixie, and other doc/tooling checks to the
  pipeline. Requires 4.1.1.
- [ ] 4.1.3. Enforce lint-level deny rules and fail builds on warnings across
  the workspace. Requires 4.1.1.
- [x] 4.1.4. Add end-to-end behavioural integration tests for `excluded_crates`
  in `no_std_fs_operations` using runtime-generated `TempDir` fixture projects
  and `cargo dylint` subprocess invocation. Closes
  [`#111`](https://github.com/leynos/whitaker/issues/111).

### 4.2. Release metadata

- [x] 4.2.1. Add cargo-binstall metadata to `installer/Cargo.toml` for
  published release artefacts. Requires 3.2.1.

### 4.3. Release workflow

- [x] 4.3.1. Implement a release workflow that builds `whitaker-installer` for
  each supported target, packages `.tgz`/`.zip` archives, and uploads them to
  GitHub Releases tagged `v<version>`. Requires 3.2.1.
- [x] 4.3.2. Publish repository-hosted dependency binaries for `cargo-dylint`
  and `dylint-link`, and teach the installer to prefer them before Cargo-based
  installation. (`design.execplan.dependency-binary`,
  `docs/execplans/install-dependency-binaries.md` Stage C-D) Requires 3.2.1.

## 5. Experimental Bumpy Road lint

### 5.1. Signal and detection pipeline

- [x] 5.1.1. Implement the per-line complexity signal builder and smoothing
  window logic. Requires 2.1.1.
- [x] 5.1.2. Detect bump intervals, surface diagnostics with labelled spans,
  and add configuration options. Requires 5.1.1.
- [x] 5.1.3. Ship UI coverage for positive and negative Bumpy Road scenarios
  and gate behind a feature flag. Requires 5.1.2.

## 6. Brain trust lints (`brain_type` and `brain_trait`)

### 6.1. Shared cohesion analysis

- [x] 6.1.1. Add a shared LCOM4 helper in `common` that builds a method graph
  and returns connected component counts. See
  [brain trust lints design](brain-trust-lints-design.md) §Cohesion analysis
  (LCOM4). Requires 1.1.1.
- [x] 6.1.2. Define method metadata extraction for field access and method
  calls, including macro-span filtering. See
  [brain trust lints design](brain-trust-lints-design.md) §Cohesion analysis
  (LCOM4) and §Implementation approach. Requires 6.1.1.

### 6.2. `brain_type` lint

- [x] 6.2.1. Implement metric collection for WMC (cognitive complexity), brain
  method detection (CC + LOC), LCOM4, and foreign reach. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_type`
  signals. Requires 6.1.2.
- [x] 6.2.2. Implement threshold evaluation and escalation rules, and surface
  measured values in diagnostics. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_type` rule
  set (initial defaults). Requires 6.2.1.
- [x] 6.2.3. Ensure macro-expanded spans are excluded or capped during CC
  calculation. See [brain trust lints design](brain-trust-lints-design.md)
  §Metric collection. Requires 6.2.1.

### 6.3. `brain_trait` lint

- [x] 6.3.1. Implement trait item counting, default method CC aggregation, and
  implementor burden metrics. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_trait`
  signals. Requires 6.2.1.
- [x] 6.3.2. Apply warning and escalation thresholds, and surface measured
  values in diagnostics. See
  [brain trust lints design](brain-trust-lints-design.md) §`brain_trait` rule
  set (initial defaults). Requires 6.3.1.

### 6.4. Decomposition advice

- [x] 6.4.1. Build feature vectors for methods and cluster with community
  detection to form decomposition suggestions. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.2.1.
- [x] 6.4.2. Emit concise diagnostic notes mapping clusters to extraction
  suggestions, capped for large types. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.2.2, 6.3.2, and 6.4.1.
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
- [x] 6.4.5. Verify `build_adjacency` preserves similarity edges, keeps
  neighbour indices in bounds, and produces symmetric, sorted adjacency lists.
  A generated Proptest checks these invariants through the production builder;
  adjacency Kani models exceeded the 8 GiB proof cap. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.
- [x] 6.4.6. Use Kani to verify `propagate_labels` preserves valid label
  indices, returns one label per input vector, and terminates within the
  supplied iteration bound. Verification harnesses are complete modulo the CBMC
  state-explosion blocker documented in the design notes. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.4.1.

### 6.5. SARIF output

- [ ] 6.5.1. Collect brain trust diagnostics into a SARIF 2.1.0 emitter that is
  opt-in and English-only for tool ingestion. See
  [brain trust lints design](brain-trust-lints-design.md) §SARIF output.
  Requires 6.2.2 and 6.3.2.

### 6.6. Configuration, localization, and tests

- [ ] 6.6.1. Add `brain_type` and `brain_trait` configuration sections to
  `whitaker.toml` with documented defaults. See
  [brain trust lints design](brain-trust-lints-design.md) §Configuration,
  localization, and testing. Requires 3.6.3, 6.2.2, and 6.3.2.
- [ ] 6.6.2. Add Fluent localization entries for both lints. See [brain trust
  lints design](brain-trust-lints-design.md) §Configuration, localization, and
  testing. Requires 2.3.3, 6.2.2, and 6.3.2.
- [ ] 6.6.3. Add UI tests for positive and negative cases under
  `crates/brain_type/ui/` and `crates/brain_trait/ui/`. See
  [brain trust lints design](brain-trust-lints-design.md) §Configuration,
  localization, and testing. Requires 1.2.1, 6.2.2, and 6.3.2.

### 6.7. Documentation

- [ ] 6.7.1. Update `docs/users-guide.md` with lint descriptions, configuration
  keys, and SARIF usage. See
  [brain trust lints design](brain-trust-lints-design.md) §Configuration,
  localization, and testing. Requires 6.5.1 and 6.6.1.

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
  Requires 7.2.1.
- [x] 7.2.3. Emit SARIF run 0 for accepted Type-1 and Type-2 pairs with stable
  fingerprints and spans. See
  [clone detector design](whitaker-clone-detector-design.md) §SARIF emission
  (Run 0). Requires 7.1.1 and 7.2.2.
- [x] 7.2.4. Add sidecar proof workflows and Makefile targets for
  clone-detector Verus and Kani checks. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md).
  Requires 7.2.2.
- [x] 7.2.5. Use Verus to prove `LshConfig::new` rejects zero `bands` and
  `rows`, and enforces `bands * rows == MINHASH_SIZE`. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
  Requires 7.2.4.
- [x] 7.2.6. Use Verus to prove `CandidatePair::new` canonicalizes fragment
  ordering and suppresses self-pairs. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
  Requires 7.2.4.
- [x] 7.2.7. Use Kani to verify bounded `MinHasher::sketch` invariants,
  including deterministic output, duplicate-hash insensitivity, and empty-input
  failure. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
  Requires 7.2.4.
- [x] 7.2.8. Use Kani to verify bounded `LshIndex` invariants, including no
  self-pairs, canonical pair ordering, repeated-band deduplication, and
  insertion-order independence. See
  [ADR 003](adr-003-formal-proof-strategy-for-clone-detector-pipeline.md) and
  [clone detector design](whitaker-clone-detector-design.md) §MinHash and LSH.
  Requires 7.2.4.

### 7.3. AST refinement (Type-3)

- [x] 7.3.1. Map candidate spans to `ra_ap_syntax` nodes and extract AST
  feature vectors. Evidence: `ra_ap_syntax = "=0.0.334"` is exact-pinned;
  `build.rs` exports the exact parser-version component and `hashing.rs`
  combines it with the AST schema revision in `PARSER_SCHEMA_VERSION`.
  `crates/whitaker_clones_core/tests/build_script_integration.rs` provides
  hermetic temporary-workspace Cargo validation of the parser pin, covering an
  exact pin and its emitted parser version, rejection of a loose/non-exact pin,
  and rejection of a missing workspace dependency. See
  [clone detector design](whitaker-clone-detector-design.md) §Pass B: AST
  engine (ra_ap_syntax). Requires 7.2.3.
- [ ] 7.3.2. Score Type-3 similarity and update SARIF run 1 with cosine and AST
  hash metadata. See [clone detector design](whitaker-clone-detector-design.md)
  §Scoring and acceptance (Type-3) and §SARIF update (Run 1). Requires 7.3.1.

### 7.4. CLI surface

- [ ] 7.4.1. Implement `cargo whitaker clones` with `scan`, `refine`, `report`,
  and `clean` subcommands. See
  [clone detector design](whitaker-clone-detector-design.md) §CLI surface.
  Requires 7.3.2.
- [ ] 7.4.2. Emit optional HTML reports and anchor deep links in SARIF results.
  See [clone detector design](whitaker-clone-detector-design.md) §Grouping and
  reporting. Requires 7.4.1.

### 7.5. Dylint integration

- [ ] 7.5.1. Implement the `clone_detected` lint to load SARIF results, filter
  to current crate files, and emit diagnostics with `help` guidance. See
  [clone detector design](whitaker-clone-detector-design.md) §Dylint integration
  (`clone_detected` lint). Requires 7.1.1.
- [ ] 7.5.2. Honour `#[allow(whitaker::clone_detected)]` and per-file
  allowlists in configuration. See
  [clone detector design](whitaker-clone-detector-design.md) §Dylint integration
  (`clone_detected` lint). Requires 7.5.1.

### 7.6. Incrementality and caching

- [ ] 7.6.1. Implement the per-file cache in `target/whitaker/clones-cache.bin`
  with config hashing and shard indexing. See
  [clone detector design](whitaker-clone-detector-design.md) §Incrementality
  and caching. Requires 7.3.1.

### 7.7. Testing and validation

- [ ] 7.7.1. Add SARIF golden tests with deterministic ordering. See [clone
  detector design](whitaker-clone-detector-design.md) §Testing strategy.
  Requires 7.3.2.
- [ ] 7.7.2. Add Dylint UI tests that consume pre-baked SARIF results. See
  [clone detector design](whitaker-clone-detector-design.md) §Testing strategy.
  Requires 1.2.1 and 7.5.1.
- [ ] 7.7.3. Add property tests for normalization invariants. See [clone
  detector design](whitaker-clone-detector-design.md) §Testing strategy.
  Requires 7.2.1.

### 7.8. Continuous integration

- [ ] 7.8.1. Add CI jobs to run token and AST passes and verify stable SARIF
  output. See [clone detector design](whitaker-clone-detector-design.md)
  §Acceptance criteria. Requires 7.4.1.

## 8. `rstest` fixture and test hygiene lints

### 8.1. Shared foundations

- [x] 8.1.1. Add shared `rstest` test and fixture detection helpers in `common`
  for attribute-based detection and optional expansion-trace fallback. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint A: call-site fixture extraction and §Integration constraints. Requires
  1.1.1.
- [x] 8.1.2. Add shared user-editable span recovery helpers for macro-heavy
  test code paths, and use them to avoid diagnostics on macro-only glue. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Integration constraints and §Lint A: call-site fixture extraction. Requires
  8.1.1.
- [x] 8.1.3. Add shared argument and paragraph fingerprint data models for
  deterministic grouping across tests. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint A: call-site fixture extraction and §Lint C: repeated fixture
  paragraph detection. Requires 8.1.1.

### 8.2. `rstest_helper_should_be_fixture` lint

- [x] 8.2.1. Create the `rstest_helper_should_be_fixture` lint crate, register
  `RSTEST_HELPER_SHOULD_BE_FIXTURE`, and wire configuration loading defaults.
  See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint A: call-site fixture extraction. Requires 8.1.1.
- [x] 8.2.2. Implement call-site collection in `#[rstest]` tests, including
  fixture-local classification and constant-aware argument fingerprinting. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint A: call-site fixture extraction. Requires 8.2.1, 8.1.2, and 8.1.3.
  Status: the lint now passively collects local helper call evidence from strict
  `#[rstest]` test bodies, deduplicates generated cases by callee definition
  path and recovered source span, and uses `CallSiteLocation` HIR local IDs to
  retain and deterministically order calls that share a recovered span, records
  fixture-local, literal, `const`, and `static` argument fingerprints, and
  keeps diagnostics disabled until 8.2.3. Parameterized collector unit cases
  cover duplicate and distinct source spans, `proptest` cases assert
  insertion-order independence and deterministic call ordering, while the
  Cargo-backed `collection_zero_diagnostic` UI harness provides the behavioural
  and end-to-end validation boundary. The private `collect_call_sites` method
  keeps the driver boundary explicit. Validation has passed for
  `cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features`,
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie`.
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
  §Lint B: single-binding paragraph detection. Requires 1.2.1 and 8.3.3.

### 8.4. `rstest_paragraph_should_be_fixture` lint

- [ ] 8.4.1. Create the `rstest_paragraph_should_be_fixture` lint crate and
  register `RSTEST_PARAGRAPH_SHOULD_BE_FIXTURE` with configurable assertion and
  input constraints. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint C: repeated fixture paragraph detection. Requires 1.1.2 and 8.1.1.
- [ ] 8.4.2. Reuse lint B candidate generation and implement assertion-free
  filtering plus fixture-or-constant input validation. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint C: repeated fixture paragraph detection. Requires 8.1.3, 8.3.2, and
  8.4.1.
- [ ] 8.4.3. Implement cross-test paragraph grouping, identical input
  fingerprint checks, and crate-post emission controls (`emit_once_per_group`).
  See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint C: repeated fixture paragraph detection. Requires 8.4.2.
- [ ] 8.4.4. Add UI pass/fail coverage for repeated setup paragraphs, assertion
  presence, differing inputs, and non-fixture-derived inputs. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Lint C: repeated fixture paragraph detection. Requires 1.2.1 and 8.4.3.

### 8.5. Integration, documentation, and promotion

- [ ] 8.5.1. Add all three lints to the experimental set with feature-gated
  suite wiring and default configuration stanzas. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Integration constraints and §Comparison and rollout guidance. Requires
  8.2.4, 8.3.4, and 8.4.4.
- [ ] 8.5.2. Add Fluent localization entries and diagnostic argument mappings
  for all three lint slugs. See
  [rstest fixture and test hygiene lints](lints-for-rstest-fixtures-and-test-hygiene.md)
  §Integration constraints. Requires 2.3.3, 8.2.4, 8.3.4, and 8.4.4.
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
  [ownership shape lints design](ownership-shape-lints-design.md) §11 Shared
  implementation approach and §16 Diagnostics and localization. Requires 1.1.1.
- [ ] 9.1.2. Add resolved-path classifiers for clone-like operations and
  wrapper constructors, following the `def_path_str` plus parsed-segment
  pattern used by `no_std_fs_operations`. See
  [ownership shape lints design](ownership-shape-lints-design.md) §11.4
  Resolution strategy and §15 Common helper requirements. Requires 9.1.1.
- [ ] 9.1.3. Add exact borrow mappings for the initial concrete type set. See
  [ownership shape lints design](ownership-shape-lints-design.md) §13 Lint 2:
  `owned_param_causes_clone` and §15 Common helper requirements. Requires 9.1.1.
- [ ] 9.1.4. Add crate-local MIR summary helpers for local uses, escapes, and
  boundary classification. See
  [ownership shape lints design](ownership-shape-lints-design.md) §11.5
  High-level Intermediate Representation (HIR) prefilter, Mid-level
  Intermediate Representation (MIR) confirmation and §10 Worked exception
  model: Servo-style code must stay quiet. Requires 9.1.2.

### 9.2. `clone_only_used_by_borrow`

- [ ] 9.2.1. Create the lint crate and register `CLONE_ONLY_USED_BY_BORROW`.
  See [ownership shape lints design](ownership-shape-lints-design.md) §12 Lint
  1: `clone_only_used_by_borrow`. Requires 9.1.1.
- [ ] 9.2.2. Implement HIR candidate prefiltering for `clone` and `to_owned`
  forms. See [ownership shape lints design](ownership-shape-lints-design.md)
  §12 Lint 1: `clone_only_used_by_borrow` and §11.5 High-level Intermediate
  Representation (HIR) prefilter, Mid-level Intermediate Representation (MIR)
  confirmation. Requires 9.1.2 and 9.2.1.
- [ ] 9.2.3. Implement MIR use classification and original-place conflict
  checks. See [ownership shape lints design](ownership-shape-lints-design.md)
  §12 Lint 1: `clone_only_used_by_borrow`. Requires 9.1.4 and 9.2.2.
- [ ] 9.2.4. Add machine-applicable suggestions for direct-call and simple-let
  forms. See [ownership shape lints design](ownership-shape-lints-design.md)
  §12.5 Suggestion policy. Requires 9.2.3.
- [ ] 9.2.5. Add UI pass and fail coverage, including macro and
  mutable-conflict cases. See
  [ownership shape lints design](ownership-shape-lints-design.md) §18 Testing
  strategy. Requires 1.2.1 and 9.2.3.

### 9.3. `owned_param_causes_clone`

- [ ] 9.3.1. Create the lint crate and register `OWNED_PARAM_CAUSES_CLONE`. See
  [ownership shape lints design](ownership-shape-lints-design.md) §13 Lint 2:
  `owned_param_causes_clone`. Requires 9.1.1.
- [ ] 9.3.2. Implement local call-site clone-pressure collection that records
  callee identity, argument index, source shape, and retained-source evidence.
  See [ownership shape lints design](ownership-shape-lints-design.md) §13.2.1
  Pass A: collect clone-pressure evidence at call sites. Requires 9.1.4 and
  9.3.1.
- [ ] 9.3.3. Implement callee parameter summaries and exported, trait, foreign
  function interface (FFI), and async suppression rules. See
  [ownership shape lints design](ownership-shape-lints-design.md) §13.2.2 Pass
  B: summarize callee parameter usage and §13.3 Exemptions. Requires 9.3.2.
- [ ] 9.3.4. Add exact borrow-type help for the initial mapping set without
  rewriting unsupported signatures. See
  [ownership shape lints design](ownership-shape-lints-design.md) §13.4 Exact
  borrow mappings and §13.5 Diagnostics. Requires 9.1.3 and 9.3.3.
- [ ] 9.3.5. Add UI coverage for private, exported, trait, async, and
  non-trigger scenarios. See
  [ownership shape lints design](ownership-shape-lints-design.md) §18 Testing
  strategy. Requires 1.2.1 and 9.3.4.

### 9.4. `local_shared_ownership`

- [ ] 9.4.1. Create the lint crate and register `LOCAL_SHARED_OWNERSHIP` behind
  an experimental feature. See
  [ownership shape lints design](ownership-shape-lints-design.md) §14 Lint 3:
  `local_shared_ownership` and §19 Rollout plan. Requires 9.1.1.
- [ ] 9.4.2. Implement wrapper-construction detection and non-escape
  classification for `Rc`, `Arc`, interior mutability, and shared-mutable
  combinations. See
  [ownership shape lints design](ownership-shape-lints-design.md) §14.2 Scope
  and §14.3 Detection model. Requires 9.1.4 and 9.4.1.
- [ ] 9.4.3. Implement callback, async, thread, trait-surface, and external-API
  suppression. See
  [ownership shape lints design](ownership-shape-lints-design.md) §14.6
  Servo-style exemptions and §14.7 `local_shared_ownership` false-positive
  controls. Requires 9.4.2.
- [ ] 9.4.4. Add diagnostic classes for interior-mutability-only,
  shared-handle-only, and shared-mutable-wrapper cases. See
  [ownership shape lints design](ownership-shape-lints-design.md) §14.4
  Diagnostic classes and §14.5 `local_shared_ownership` suggestion policy.
  Requires 9.4.3.
- [ ] 9.4.5. Add Servo-style regression fixtures and framework-boundary
  negatives. See
  [ownership shape lints design](ownership-shape-lints-design.md) §10 Worked
  exception model: Servo-style code must stay quiet and §18 Testing strategy.
  Requires 1.2.1 and 9.4.4.

### 9.5. Localization, documentation, and promotion

- [ ] 9.5.1. Add Fluent entries and diagnostic argument mappings for all three
  lints. See [ownership shape lints design](ownership-shape-lints-design.md)
  §16 Diagnostics and localization. Requires 2.3.3, 9.2.5, 9.3.5, and 9.4.5.
- [ ] 9.5.2. Add Welsh and Gaelic smoke coverage for the ownership-shape
  diagnostics. See
  [ownership shape lints design](ownership-shape-lints-design.md) §16
  Diagnostics and localization and §18 Testing strategy. Requires 2.3.4 and
  9.5.1.
- [ ] 9.5.3. Update `docs/users-guide.md` and `docs/developers-guide.md` with
  lint intent, configuration, and rollout guidance for the ownership-shape
  suite. See [ownership shape lints design](ownership-shape-lints-design.md)
  §17 Configuration and §19 Rollout plan. Requires 9.2.5, 9.3.5, and 9.4.5.
- [ ] 9.5.4. Define promotion criteria from experimental to standard based on
  UI stability and false-positive tuning across representative repositories. See
  [ownership shape lints design](ownership-shape-lints-design.md) §19 Rollout
  plan. Requires 9.5.2.

## 10. Async-trait architecture hygiene lints

### 10.1. Shared family analysis and configuration

- [ ] 10.1.1. Add `whitaker::async_trait_hygiene` modules behind the
  `dylint-driver` feature and implement the `AsyncTraitFamilyIndex` data model.
  See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Shared helper placement in Whitaker and §AsyncTraitFamilyIndex fields.
  Requires 1.1.1.
- [ ] 10.1.2. Implement dyn-use closure, supertrait closure, boxed-future alias
  detection, sibling lookup, and adapter discovery with deterministic ordering.
  See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Algorithms and §AsyncTraitFamilyIndex fields. Requires 10.1.1.
- [ ] 10.1.3. Add shared configuration types and loaders for suite defaults,
  allowlists, alias policy, and `Send` policy overrides. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Configuration model. Requires 1.1.2 and 10.1.1.

### 10.2. Inventory and family-completeness lints

- [ ] 10.2.1. Create the `async_trait_concrete_only` and
  `async_trait_signature_markers_present` lint crates with diagnostics wired to
  the shared family index. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Proposed lint table. Requires 10.1.2 and 10.1.3.
- [ ] 10.2.2. Create the `async_dyn_family_requires_native_sibling` and
  `async_dyn_family_requires_blanket_adapter` lint crates, including
  family-scoped allowlist support. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Proposed lint table and §Migration waves and default levels. Requires
  10.2.1.
- [ ] 10.2.3. Add UI coverage for dyn-required supertrait cases,
  high-confidence async-trait marker detection, missing sibling traits, and
  missing blanket adapters. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Proposed lint table. Requires 1.2.1 and 10.2.2.

### 10.3. Signature-shape and migration-ratchet lints

- [ ] 10.3.1. Create the `async_dyn_direct_impl_prefers_native` and
  `async_dyn_boxed_future_alias_required` lint crates and implement their
  family-shape checks. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Proposed lint table. Requires 10.1.2 and 10.1.3.
- [ ] 10.3.2. Create the `native_async_future_must_be_send` and
  `native_async_multi_borrow_requires_named_lifetime` lint crates, including
  per-family non-`Send` policy overrides. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Proposed lint table and §Pending architectural decisions and trade-offs.
  Requires 10.3.1.
- [ ] 10.3.3. Add UI coverage for direct dyn impls, raw boxed-future returns,
  missing `+ Send`, allowed non-`Send` families, and multi-borrow lifetime
  fixes. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Proposed lint table. Requires 1.2.1 and 10.3.2.

### 10.4. Suite wiring, rollout, and documentation

- [ ] 10.4.1. Add the async-trait hygiene lints either to a specialist suite or
  to `whitaker_suite` behind an explicit opt-in feature, and expose selector
  defaults for inventory versus ratchet waves. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Suite plumbing and §Migration waves and default levels. Requires 3.6.1,
  10.2.3, and 10.3.3.
- [ ] 10.4.2. Add CI and maintainer guidance that capture timing evidence
  outside lint execution and document crate-local truth versus workspace-level
  audit limits. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §CI and xtask responsibilities versus lints. Requires 4.1.1 and 10.4.1.
- [ ] 10.4.3. Update `docs/users-guide.md` and `docs/developers-guide.md` with
  the migration-wave model, configuration keys, and suppression guidance for
  async-trait hygiene findings. See
  [async-trait architecture hygiene Dylint suite design](async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md)
  §Configuration model and §Migration waves and default levels. Requires
  10.4.1.

## 11. Test-support dead-code and masked-expectation analysis

### 11.1. Detector lints and module-graph discovery

- [ ] 11.1.1. Create the `test_support_dead_code` lint crate and implement the
  fast detector for dead-code suppressions in integration-test support modules.
  See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Proposed rule contracts. Requires 1.1.1.
- [ ] 11.1.2. Create the `masked_dead_code_expectations` lint crate and detect
  `#[expect(dead_code)]` under `allow(unfulfilled_lint_expectations)` in the
  same support-module scope. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Proposed rule contracts. Requires 11.1.1.
- [ ] 11.1.3. Add integration-test module-graph discovery for `tests/*.rs`
  harnesses, ordinary `mod` edges, and explicit `#[path]` edges so workspace
  analysis can identify importer targets precisely. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Multi-pass execution path. Requires 11.1.1.

### 11.2. Overlay builds and workspace analysis

- [ ] 11.2.1. Implement overlay workspace creation and text-edit planning that
  strips only `dead_code` suppressions, preserves unrelated lints, and inserts
  probe items without mutating the working tree. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Multi-pass execution path. Requires 11.1.3.
- [ ] 11.2.2. Add per-target `cargo check --message-format=json` replay plus a
  collector that records organic and synthetic use sites for support items. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Multi-pass execution path and §Diagnostic model and report format. Requires
  11.2.1.
- [ ] 11.2.3. Add masked-expectation replay runs that remove
  `allow(unfulfilled_lint_expectations)` only where needed and classify stale
  expectations per importer target. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Multi-pass execution path. Requires 11.1.2 and 11.2.2.

### 11.3. Reporting, `whitaker check` integration, and validation

- [ ] 11.3.1. Integrate the merged dead-code and stale-expectation report into
  `whitaker check`, with per-item classifications for globally dead,
  single-target live, shared, and synthetic-only support items. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Diagnostic model and report format. Requires 3.5.2 and 11.2.3.
- [ ] 11.3.2. Add JSON output for the merged report so CI and follow-on tools
  can consume item inventories, importer targets, and stale-expectation data.
  See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Diagnostic model and report format. Requires 11.3.1.
- [ ] 11.3.3. Add fixtures and behaviour coverage derived from the Axinite and
  mdtablefix worked examples, including `#[path]` traversal, genuine shared
  helpers, and synthetic keepalive shims. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Worked examples and §Constraints, failure modes, and rollout. Requires
  11.3.1.
- [ ] 11.3.4. Update `docs/users-guide.md` and `docs/developers-guide.md` with
  the cleanup workflow, v1 non-goals, and guidance on when not to replace
  `allow(dead_code)` with `expect(dead_code)`. See
  [technical design for `test_support_dead_code` and `masked_dead_code_expectations`](technical-design-for-test-support-dead-code-and-masked-dead-code-expectations.md)
  §Constraints, failure modes, and rollout. Requires 11.3.1.

## 12. Architecture and compile-time hygiene enforcement

### 12.1. Shared policy schema and lint taxonomy

- [ ] 12.1.1. Add shared `[workspace.metadata.whitaker]` policy parsing for
  layers, forbidden dependencies, feature islands, compile-hygiene thresholds,
  and allowlists. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Policy storage and shared configuration model. Requires 1.1.2.
- [ ] 12.1.2. Register Whitaker lint groups and naming conventions for
  `arch_*`, `hygiene_*`, `tests_*`, and `advisory_*` rules so CI and users can
  enable coherent policy sets. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Whitaker lint namespaces and naming conventions and §Severity levels and
  build gating. Requires 12.1.1.
- [ ] 12.1.3. Document suppression guidance for early, late, and pre-expansion
  Whitaker lints, including `cfg_attr(dylint_lib = "whitaker", ...)` and
  `#[allow(unknown_lints)]` escape hatches. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Feature-matrix and “unknown lint” ergonomics. Requires 12.1.2.

### 12.2. Whitaker lint MVP for architecture and source-level hygiene

- [ ] 12.2.1. Implement `whitaker::arch_hexagonal_layer_boundary` with
  resolution-aware internal and external dependency checks, and port the
  Wildside-style regression fixtures into Dylint UI tests. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §`whitaker::arch_hexagonal_layer_boundary`. Requires 1.2.1 and 12.1.1.
- [ ] 12.2.2. Implement `whitaker::hygiene_public_api_leaks_optional_dep` and
  `whitaker::hygiene_feature_island_breach`, including feature-matrix coverage
  for guarded and unguarded heavy-dependency paths. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §`whitaker::hygiene_public_api_leaks_optional_dep` and §
  `whitaker::hygiene_feature_island_breach`. Requires 1.2.1 and 12.1.1.
- [ ] 12.2.3. Implement `whitaker::tests_ui_test_macro_outside_app` as a
  pre-expansion lint and keep `whitaker::advisory_async_trait_clear_misuse` as
  an advisory-only rule with a conservative mode. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §`whitaker::tests_ui_test_macro_outside_app` and §
  `whitaker::advisory_async_trait_clear_misuse`. Requires 1.2.1 and 12.1.3.

### 12.3. `cargo-compile-hygiene` tool and checks

- [ ] 12.3.1. Create the `cargo-compile-hygiene` Cargo subcommand with `check`,
  `check --json`, `explain`, and `baseline --write` entrypoints, using
  `cargo metadata --format-version 1` as the primary graph input. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Tool shape and why it is a Cargo subcommand and §Commands and CLI UX.
  Requires 12.1.1.
- [ ] 12.3.2. Implement the `integration_target_budget`,
  `heavy_dependency_not_optional`, and `duplicate_major_version_hotspots`
  checks with shortest-path or hotspot explanations in reports. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Checks. Requires 12.3.1.
- [ ] 12.3.3. Implement the `tls_backend_multiplicity` and
  `package_boundary_purity` checks, including per-configuration graph analysis
  for feature-matrix runs. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Checks and §Invocation patterns and feature-matrix handling. Requires
  12.3.1.
- [ ] 12.3.4. Add human-readable and JSON reporting with finding IDs, metrics,
  remediation guidance, and CI-oriented exit semantics. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Output formats and example reports. Requires 12.3.2 and 12.3.3.

### 12.4. CI, rollout, and migration guidance

- [ ] 12.4.1. Add default, minimal, and all-features CI jobs that run both the
  Whitaker lint set and `cargo-compile-hygiene` across the same feature matrix.
  See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Invocation patterns and feature-matrix handling. Requires 4.1.1, 12.2.1,
  12.2.2, 12.2.3, and 12.3.4.
- [ ] 12.4.2. Update `docs/users-guide.md`, `docs/developers-guide.md`, and
  CLI-facing workflow documentation with shared policy examples, report
  interpretation guidance, and migration sequencing for downstream adopters. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Comparison table: what belongs where and §Migration guidance for Axinite and
  Gauss. Requires 12.2.1, 12.2.2, 12.2.3, and 12.3.4.
- [ ] 12.4.3. Define staged rollout criteria for promoting selected
  architecture and compile-hygiene findings from report-only or `warn` status
  to CI-failing policy. See
  [Whitaker and cargo-compile-hygiene technical design](whitaker-and-cargo-compile-hygiene-technical-design.md)
  §Severity levels and build gating and §Integration, CI, and rollout.
  Requires 12.4.1.

## 13. CodeRabbit-derived lint candidates

### 13.1. Immediate and Clippy-first candidates

- [ ] 13.1.1. Implement `test_helper_must_return_result` for configured
  test-step and fixture functions that panic instead of propagating failure
  through `Result`. Control analysis of plain helper functions called from
  those functions through configuration. See
  [RFC 0001](rfcs/0001-coderabbit-derived-lint-candidates.md) §Tier 1:
  immediate and Clippy-first candidates. Requires 8.1.1 and 8.1.2.
- [ ] 13.1.2. Enable and trial Clippy's `missing_assert_message` against the
  source corpus, then implement `assertion_context_incomplete` only for
  surviving mismatch branches whose error payload omits either expected or
  actual context. See
  [RFC 0001](rfcs/0001-coderabbit-derived-lint-candidates.md) §Tier 1:
  immediate and Clippy-first candidates. Requires 8.1.2.
- [ ] 13.1.3. Trial Clippy's `disallowed_methods` for direct environment calls,
  then implement `no_std_env_operations` only if centrally configured boundary
  exemptions, operation-specific severity, or context-sensitive remediation
  remain unmet. See [RFC 0001](rfcs/0001-coderabbit-derived-lint-candidates.md)
  §Tier 1: immediate and Clippy-first candidates. Requires 2.1.1.
- [ ] 13.1.4. Add implemented candidates to the experimental suite, localize
  their diagnostics, document Clippy prerequisites and configuration, and
  define promotion criteria based on false-positive tuning across downstream
  repositories. Requires 2.3.4, 13.1.1, 13.1.2, and 13.1.3.

### 13.2. Follow-up candidates

- [ ] 13.2.1. Implement `error_context_discarded` for mechanically provable
  error-type or context erasure at propagation sites. See
  [RFC 0001](rfcs/0001-coderabbit-derived-lint-candidates.md) §Tier 2:
  recommended as follow-ups. Requires 13.1.4.
- [ ] 13.2.2. Implement `no_drop_to_silence_unused` for `drop(binding)` calls
  used only to suppress an otherwise-unused parameter. See
  [RFC 0001](rfcs/0001-coderabbit-derived-lint-candidates.md) §Tier 2:
  recommended as follow-ups. Requires 1.3.3 and 13.1.4.

### 13.3. Deferred candidates

- [ ] 13.3.1. Reassess and, if the ownership analysis supports a sufficiently
  precise rule, implement `fallible_gap_before_guard` for fallible operations
  between resource acquisition and guard construction. See
  [RFC 0001](rfcs/0001-coderabbit-derived-lint-candidates.md) §Tier 3: recorded
  but deferred. Requires 9.1.4.
- [ ] 13.3.2. Reassess and, if existing duplicate detection is too coarse for
  test-specific guidance, implement `parameterizable_duplicate_tests` using the
  established clone fingerprints. See
  [RFC 0001](rfcs/0001-coderabbit-derived-lint-candidates.md) §Tier 3: recorded
  but deferred. Requires 7.5.1 and 8.5.4.

## 14. Compiler-validated ownership rewrite pipeline

### 14.1. Rewrite substrate spike

- [ ] 14.1.1. Implement the backend-neutral spike contract and shared fixtures
  for borrow-only clone removal, private owned-parameter conversion,
  Polonius-only conditional borrowing, and remove/mutate/reinsert collapse. See
  [ADR 007](adr-007-rust-rewrite-materialization-substrate.md)
  §Comparative spike.
- [ ] 14.1.2. Implement the spike once with `ast-grep-core` and
  `ast-grep-language`, and once with `ra_ap_syntax::SyntaxEditor`, including
  stale-anchor, trivia-preservation, overlapping-edit, CRLF, Unicode,
  macro-span, and Rust 2024 adversarial fixtures. See
  [ADR 007](adr-007-rust-rewrite-materialization-substrate.md) §Primary rewrite
  fixtures and §Adversarial fixtures. Requires 14.1.1.
- [ ] 14.1.3. Record source-fidelity, recipe-complexity, build-cost,
  dependency-churn, and runtime evidence; select one production backend and pin
  its version. See [ADR 007](adr-007-rust-rewrite-materialization-substrate.md)
  §Decision rule and §Spike exit artefacts. Requires 14.1.2.

### 14.2. Compiler-validated rewrite checker

- [ ] 14.2.1. Add versioned `RewriteIntent`, `TextEditPlan`,
  `RewriteCheckRequest`, and `RewriteCheckReport` models with deterministic
  serialization and stable diagnostic keys. See
  [RFC 0007](rfcs/0007-compiler-validated-rewrite-checking.md) §Proposed
  architecture. Requires 14.1.3.
- [ ] 14.2.2. Generalize the Phase 11 overlay builder for copy-isolated
  candidate workspaces and equivalent baseline and candidate Cargo invocations.
  See [RFC 0007](rfcs/0007-compiler-validated-rewrite-checking.md) §Overlay
  construction. Requires 11.2.1 and 14.2.1.
- [ ] 14.2.3. Implement same-nightly NLL and `-Zpolonius=next` replay,
  capability probing, normalized diagnostic deltas, and classifications for
  NLL-valid, Polonius-only, rejected, stale, divergent, and inconclusive
  candidates. See [RFC 0007](rfcs/0007-compiler-validated-rewrite-checking.md)
  §Classification algorithm. Requires 11.2.2 and 14.2.2.
- [ ] 14.2.4. Add content-addressed validation caching, retained-overlay
  reproduction bundles, human-readable output, and stable JSON output. See
  [RFC 0007](rfcs/0007-compiler-validated-rewrite-checking.md) §Caching and
  §CLI surface. Requires 14.2.3.

### 14.3. Borrow-workaround lint family

- [ ] 14.3.1. Add shared BOR-family models and detectors for
  `lookup_then_relookup`, `remove_then_reinsert`, and `take_then_restore`. See
  [RFC 0008](rfcs/0008-borrow-workaround-lint-family.md) §Rule contracts.
  Requires 9.1.4 and 14.2.1.
- [ ] 14.3.2. Add detectors for `index_round_trip_for_mutation`,
  `staged_keys_before_mutation`, and `split_search_and_mutate_pass`. See
  [RFC 0008](rfcs/0008-borrow-workaround-lint-family.md) §Rule contracts.
  Requires 14.3.1.
- [ ] 14.3.3. Add detectors for `snapshot_around_mutation`,
  `borrow_scope_fence`, and `single_use_borrow_split_helper`. See
  [RFC 0008](rfcs/0008-borrow-workaround-lint-family.md) §Rule contracts.
  Requires 14.3.1.
- [ ] 14.3.4. Add UI and corpus fixtures that cover true workarounds,
  reallocation hazards, rollback and undo semantics, callback and async
  boundaries, external API constraints, and cases rejected by both checkers. See
  [RFC 0008](rfcs/0008-borrow-workaround-lint-family.md) §Testing strategy.
  Requires 14.3.2 and 14.3.3.
- [ ] 14.3.5. Keep BOR findings review-only unless a direct candidate can be
  materialized and compiler-validated; expose confidence and checker
  requirement in diagnostics. See
  [RFC 0008](rfcs/0008-borrow-workaround-lint-family.md) §Diagnostics and
  reporting. Requires 14.2.4 and 14.3.4.

### 14.4. Validated ownership and borrow rewriter

- [ ] 14.4.1. Implement materializers for Phase 9
  `clone_only_used_by_borrow`, `owned_param_causes_clone`, and
  `local_shared_ownership` intents. See
  [RFC 0009](rfcs/0009-validated-ownership-and-borrow-rewriter.md) §Existing
  ownership-lint materializers. Requires 9.2.3, 9.3.4, 9.4.4, and 14.1.3.
- [ ] 14.4.2. Implement ranked stable-compatible and Polonius-only
  materializers for `BOR001` through `BOR009`. See
  [RFC 0009](rfcs/0009-validated-ownership-and-borrow-rewriter.md)
  §Borrow-workaround materializers. Requires 14.3.5 and 14.4.1.
- [ ] 14.4.3. Add semantic-anchor revalidation, alternative ranking,
  dependency and conflict graphs, deterministic diffs, and rejection of stale,
  ambiguous, or overlapping plans. See
  [RFC 0009](rfcs/0009-validated-ownership-and-borrow-rewriter.md) §Semantic
  anchors and staleness and §Planning, dependencies, and conflicts. Requires
  14.4.2.
- [ ] 14.4.4. Implement preview, transactional apply, formatting and test
  gates, application journals, byte-exact rollback, and explicit opt-in for
  Polonius-only edits. See
  [RFC 0009](rfcs/0009-validated-ownership-and-borrow-rewriter.md)
  §Transactional application. Requires 14.2.4 and 14.4.3.

### 14.5. CLI integration and rollout

- [ ] 14.5.1. Integrate candidate discovery, validation, preview, and apply
  flows into `whitaker check` and the unified configuration model, with project
  policy for `nll` versus `polonius-next`. Requires 3.5.2, 3.6.3, 14.2.4, and
  14.4.4.
- [ ] 14.5.2. Add localized diagnostics and stable JSON and SARIF fields for
  rule, recipe, validation profile, portability requirement, confidence, and
  application status. Requires 2.3.4 and 14.5.1.
- [ ] 14.5.3. Dogfood the pipeline across representative parser, graph,
  editor, game-state, and async-service repositories; record validated
  findings, rejected candidates, compile cost, and suppression churn. Requires
  14.5.2.
- [ ] 14.5.4. Define promotion criteria for standard diagnostics and automated
  application based on false-positive rate, semantic review outcomes, compiler
  stability, and measured agent-development throughput. Requires 14.5.3.

## 15. Macro-aware cognitive complexity

Idea: if Whitaker charges cognitive complexity only to constructs the author
wrote, using one hygiene-based classifier, then function scores stop depending
on dependency features such as `tracing/log` and match Clippy exactly on
macro-free code. Downstream projects then get a complexity gate they can keep
enabled, instead of restructuring code around generated branching.

This phase implements phases 1, 2, 3 and 5 of
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §12. It settles the
compiler behaviour the classifier depends on, proves the classifier in
isolation, and ships the lint as an experimental suite member with differential
and end-to-end evidence. It closes by testing whether the Sonar-faithful model
should become the shared measure. Adoption by the existing complexity lints is
deliberately held back to phase 16, because it changes stable behaviour.

### 15.1. Settle how the pinned compiler records desugaring provenance

This step answers whether rustc stacks a desugaring mark on top of the
enclosing syntax context or re-roots it behind `ExpnData::call_site`. The
answer fixes the adapter's chain walk and decides whether a `for` loop or `?`
inside a macro body can be classified at all. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.2 and §14.

- [ ] 15.1.1. Measure desugaring mark placement on the pinned toolchain and
  record the authored-versus-generated classifier as an architectural decision
  record (ADR).
  - Requires 1.2.1.
  - Probe `for`, `while`, `?`, `.await` and `async` written in source, in
    macro arguments, and inside `macro_rules!` bodies. Log each node's mark
    chain and `ExpnData::call_site`.
  - Re-establish issue 504's `ExpnKind` census and `tracing/log` measurements
    on `nightly-2026-05-28`.
  - Record the classification rule, chain-walk decision and reuse policy in
    the ADR, and reference it from
    [Whitaker Dylint suite design](whitaker-dylint-suite-design.md).
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §§3.2, 7.2
    and 7.10.
  - Success: the accepted ADR lists the observed chain and required
    classification for every probe construct, and the re-measured tables
    either match issue 504 or record the differences.

### 15.2. Prove the authored-versus-generated classifier

This step answers whether one compiler-free policy and a thin compiler adapter
can classify real HIR nodes correctly. Its outcome is the single provenance
rule that the lint in step 15.3 and every lint in phase 16 consume. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.2 and §7.10.

- [ ] 15.2.1. Implement `NodeOrigin`, `ExpansionStep` and `classify` in
  `common/src/node_origin.rs`.
  - Requires 15.1.1.
  - Enumerate every chain of length 0 to 4 exhaustively, and add `proptest`
    properties for desugaring transparency and macro opacity over unbounded
    lengths.
  - Document the walker rules and reuse policy in
    [developers' guide](developers-guide.md): classify the node's own span,
    never prune generated subtrees, and recover spans only for diagnostics.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.2, §7.10
    and §11.1.
  - Success: all 341 enumerated chains satisfy the three properties, and the
    module builds and tests without `rustc_private` in the default nextest
    lane.
- [ ] 15.2.2. Implement the `src/hir/` adapter `node_origin` and
  `is_generated`, with an exhaustive `ExpnKind` and `DesugaringKind` mapping.
  - Requires 15.2.1.
  - Walk the chain as the ADR from 15.1.1 prescribes, and never follow a
    macro expansion's `call_site`.
  - Parameterize the synthetic hygiene constructor in `src/hir/tests.rs` by
    `ExpnKind` and parent context.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.2
    (Table 5) and §11.2.
  - Success: the five synthetic contexts in RFC 0010 §11.2 classify as
    required, and the mapping has no wildcard arm, so a new `DesugaringKind`
    variant fails to compile.

### 15.3. Deliver the Clippy-compatible macro-aware lint

This step answers whether charging by origin, while always descending,
reproduces Clippy's arithmetic on macro-free code and removes generated
branching everywhere else. It produces the lint that phase 16 measures the
other complexity lints against. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.

- [ ] 15.3.1. Create `crates/macro_aware_cognitive_complexity` with its
  configuration, pure scoring policy and feature-off stub.
  - Requires 2.1.1 and 15.2.1.
  - Parse `max_complexity` (default 25, strictly-greater-than comparison) with
    `serde(default, deny_unknown_fields)`.
  - Implement the authored tally: baseline, `if`, multi-arm `match`, guard and
    the consecutive-return-aware adjustment. Test it model-based against an
    oracle of Clippy's arithmetic, with a `proptest` property that generated
    charges never change the score.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.4 and
    §7.7.
  - Success: policy tests run in the ordinary test lane without the
    `dylint-driver` feature.
- [ ] 15.3.2. Implement the HIR walker and `check_fn` driver that charge
  authored nodes and always descend.
  - Requires 15.2.2 and 15.3.1.
  - Anchor bodies on the identifier or the closure's `fn_decl_span`. Treat
    generated closures and `async` blocks as transparent, skip `#[test]`
    functions, and unwrap `async fn` bodies.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.3, §7.5
    and §7.9.
  - Success: every user-interface (UI) fixture listed in RFC 0010 §11.3 passes
    at `max_complexity = 0`. That includes each Table 7 row, identity-macro
    invariance and the `when!` mixed-provenance case.
- [ ] 15.3.3. Emit localized diagnostics reporting the score, the limit and the
  excluded generated units.
  - Requires 2.3.3 and 15.3.2.
  - Add `en-GB`, `cy` and `gd` Fluent resources, and emit the excluded-units
    note only when the count is non-zero.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §7.8.
  - Success: UI `.stderr` files pin the primary message, note and help in
    `en-GB`, and a locale-selection test covers `cy` and `gd`.

### 15.4. Prove parity and dependency-feature invariance end to end

This step answers whether the lint's two headline guarantees hold against the
real Clippy and against real dependency graphs, rather than only against
fixtures written to match the design. Its evidence decides whether the lint is
trustworthy enough to promote and to serve as phase 16's reference. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §8.1 and §11.

- [ ] 15.4.1. Add the differential parity suite against Clippy's
  `cognitive_complexity`.
  - Requires 15.3.3.
  - Build a macro-free fixture crate under both tools at threshold 0 and
    compare scores per function. On a fixture with macros but no generated
    `return`, assert that Clippy's score minus Whitaker's equals the reported
    excluded units.
  - Enumerate any bodies Clippy skips because of desugaring marks.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §11.4.
  - Success: the suite passes against the pinned toolchain's Clippy, and every
    enumerated divergence is listed in [users' guide](users-guide.md).
- [ ] 15.4.2. Add the `tracing/log` feature-matrix end-to-end suite.
  - Requires 15.3.3.
  - Generate two standalone fixture projects that differ only in `tracing`'s
    `log` feature, following `crates/no_std_fs_operations/tests/test_support`.
    Include a `#[tracing::instrument]` `async fn`.
  - Isolate each project's target directory, and run the suite in its own
    nextest group with a timeout and registry access.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §3.1 and
    §11.5.
  - Success: `cargo tree -e features` shows the difference, the four-call
    probe scores 1 in both projects, and the diagnostics are byte-identical.

### 15.5. Ship the lint as an experimental suite member

This step answers whether users can opt in through the suite and installer
without changing the stable lint set. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §8.2 and §10.

- [ ] 15.5.1. Register `macro_aware_cognitive_complexity` behind
  `experimental-macro-aware-cognitive-complexity` in the suite and installer.
  - Requires 3.1.1, 3.2.1 and 15.3.3.
  - Update `suite/Cargo.toml`, `suite/src/lints.rs`, `suite/src/driver.rs` and
    `installer/src/resolution.rs`, following
    `rstest_helper_should_be_fixture`.
  - Success: the suite builds with and without the feature, `--experimental`
    installs the lint, and the default installation is unchanged.
- [ ] 15.5.2. Document the lint and the migration from Clippy.
  - Requires 15.4.1 and 15.5.1.
  - In [users' guide](users-guide.md), describe the lint as a structural proxy
    for reading effort. Cover its configuration, exclusion semantics, known
    blind spots and the migration from `clippy::cognitive_complexity`.
  - Add a lint section to
    [Whitaker Dylint suite design](whitaker-dylint-suite-design.md) that
    references the ADR from 15.1.1 and RFC 0010.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §5.5, §7.9
    and §10.
  - Success: every configuration and diagnostic example in the users' guide
    matches UI fixture output.

### 15.6. Test the Sonar-faithful model and decide promotion

This step answers whether the empirically validated Sonar model, fed by the
same classifier, ranks real functions more usefully than Clippy's model. Its
outcome selects the shared measure that phase 16 adopts. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §5.2 and §12.

- [ ] 15.6.1. Add `model = "sonar"` to the lint, backed by
  `CognitiveComplexityBuilder`.
  - Requires 15.3.2.
  - Map HIR onto Sonar increments: `if` with `else if` and `else` as hybrid
    increments, `match` as a switch, loops recognized through `LoopSource`,
    labelled `break` and `continue`, boolean operator sequences, and closures
    as nesting.
  - Ensure generated structures neither charge nor raise the nesting depth,
    and resolve RFC 0010 open question 2 (`.await`) for this model.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §5.1 and
    §7.4, and [brain trust lints design](brain-trust-lints-design.md)
    §Implementation decisions (6.2.3).
  - Success: UI fixtures reproduce the white paper's worked examples, as
    transliterated into Rust, and no generated wrapper raises an authored
    construct's nesting increment.
- [ ] 15.6.2. Run the corpus evaluation and record the promotion and
  shared-measure decision.
  - Requires 15.4.1 and 15.6.1.
  - Score Whitaker, Netsuke and a sample of public crates under Clippy, the
    version-1 model and the Sonar model. Explain every per-function difference
    and hand-inspect the 20 highest-scoring functions under each model.
  - Decide the default model, default threshold, lint name and stable
    promotion (RFC 0010 open questions 6 and 7).
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §11.6 and
    §14.
  - Success: a published report and an ADR amendment name the shared measure
    and threshold scale that phase 16 adopts.

## 16. Harmonized complexity measurement across the suite (breaking)

Idea: suppose `conditional_max_n_branches`, `bumpy_road_function` and the brain
trust lints all consume the classifier and the shared measure chosen in 15.6.2.
Users then get one consistent account of complexity across every Whitaker
complexity lint. A single breaking release is repaid by removing both
generated-branch false positives and the silent skipping of source `for` loops
and macro arguments. If corpus results show that the lints lose detection
quality when moved onto the shared measure, the idea is falsified.

These lints currently disagree with each other: `bumpy_road_function`
over-excludes, `conditional_max_n_branches` under-excludes, and the brain trust
design plans to pass `from_expansion()` to its builder. This phase moves them
all onto the shared measure and ships the result as one breaking change with a
migration guide, rather than as a series of silent behaviour changes. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §3.3, §9 and §10.

### 16.1. Ratify the harmonized complexity contract

This step answers what "harmonized" means for three lints that measure
different signals: predicate breadth, per-line bumps and per-type weighted
method complexity. Its contract fixes the configuration, defaults and
diagnostics that every later step changes. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §9 and §14.

- [ ] 16.1.1. Record an ADR that defines the shared complexity measure and how
  each complexity lint consumes it.
  - Requires 15.6.2.
  - Define the three consumers:
    - `conditional_max_n_branches` counts authored predicate atoms;
    - `bumpy_road_function` derives its per-line signal from the shared
      measure's increments;
    - the brain trust lints compute weighted method complexity (WMC) and
      brain-method complexity with the shared measure and threshold scale.
  - List every renamed or deprecated configuration key, every re-baselined
    default and every changed diagnostic, per lint.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §9 and open
    question 8, [brain trust lints design](brain-trust-lints-design.md)
    §Implementation decisions (6.2.3) and (6.3.1), and
    [Whitaker Dylint suite design](whitaker-dylint-suite-design.md) §12.
  - Success: the accepted ADR is the complete change list that 16.5.2's
    migration guide must cover.

### 16.2. Move `conditional_max_n_branches` onto the classifier

This step answers whether predicate counting can exclude generated conditions
without losing authored ones. It is the smallest adoption, so it exercises the
contract before the riskier Bumpy Road change. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §9.

- [ ] 16.2.1. Exclude generated conditions from `conditional_max_n_branches`,
  classifying each match guard by its own span.
  - Requires 15.2.2 and 16.1.1.
  - Skip `inspect_condition` for generated `If` nodes and generated `while`
    loops, and leave traversal unchanged.
  - Apply the configuration and diagnostic changes the ADR assigns to this
    lint.
  - Success: `assert!(a && b && c)` is no longer flagged, while an authored
    three-predicate `if` in a macro argument and a source three-predicate
    `while` still are.

### 16.3. Move `bumpy_road_function` onto the shared measure

This step answers whether deriving the bump signal from the shared measure
preserves Bumpy Road's detection quality once subtree pruning is gone. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §3.3 and §9, and
[Whitaker Dylint suite design](whitaker-dylint-suite-design.md) §12.

- [ ] 16.3.1. Replace `bumpy_road_function`'s `from_expansion()` pruning with
  charge-gating.
  - Requires 15.2.2 and 16.1.1.
  - Replace the item and body guards with `is_generated` on the anchor.
    `SegmentBuilder::visit_expr` records nothing for a generated node but
    still walks its children, and the `push_segment` guard uses the
    classifier.
  - Keep the `DesugaringKind::WhileLoop` handling and the closure policy.
  - Success: UI fixtures show that branching in source `for` loops and macro
    arguments contributes bumps and macro-generated branching does not.
- [ ] 16.3.2. Derive Bumpy Road's per-line complexity signal from the shared
  measure's increments.
  - Requires 16.3.1.
  - Attribute each authored increment to its source line, and re-baseline the
    default threshold against the 15.6.2 corpus.
  - Keep the weighting model tracked by issue 357 out of scope.
  - Success: on the 15.6.2 corpus, every reported bump interval traces to
    increments the shared measure records. The re-baselined default is
    published together with its false-positive review.

### 16.4. Move the brain trust lints onto the shared measure

This step answers whether type-level complexity can reuse the function-level
measure, so that a brain method and a complex function mean the same thing. See
[brain trust lints design](brain-trust-lints-design.md)
§Implementation decisions (6.2.3) and (6.3.1).

- [ ] 16.4.1. Feed `CognitiveComplexityBuilder` from the classifier, and align
  brain trust thresholds with the shared measure's scale.
  - Requires 16.1.1.
  - Derive the builder's expansion flags from `is_generated`, not
    `from_expansion()`, and rename the parameters as the ADR specifies.
  - Re-derive the brain-method complexity threshold on the shared scale, and
    amend the brain trust design sections cited above.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §3.3 and §9.
  - Success: for every function in a shared fixture, the brain-method
    complexity equals `macro_aware_cognitive_complexity`'s score under the
    shared model.

### 16.5. Release the harmonized measure as one breaking change

This step answers whether the harmonized lints behave consistently together and
whether downstream projects can migrate mechanically. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §10.

- [ ] 16.5.1. Add the cross-lint harmonization end-to-end suite.
  - Requires 15.4.2, 15.5.1, 16.2.1, 16.3.2 and 16.4.1.
  - Build one fixture workspace and lint it across three axes:
    - `tracing/log` on and off;
    - the experimental feature on and off;
    - each complexity lint enabled alone, and all enabled together.
  - Success: each lint's diagnostics are byte-identical across the `log` axis,
    and no lint charges a construct that the classifier marks as generated.
- [ ] 16.5.2. Publish the migration guide, changelog entries and breaking
  version bump for the harmonized measure.
  - Requires 16.5.1.
  - Add a `docs/migrations/` guide with before-and-after behaviour,
    configuration renames and threshold re-baselining advice for each lint.
  - Update the affected sections of [users' guide](users-guide.md) and
    [Whitaker Dylint suite design](whitaker-dylint-suite-design.md). Promote
    `macro_aware_cognitive_complexity` to the stable suite if 15.6.2 approved
    it.
  - Success: the migration guide covers every change listed in the 16.1.1
    ADR, and the release notes flag the release as breaking.

### 16.6. Revisit deferred macro-aware extensions

This step collects scope that RFC 0010 explicitly leaves open, so that it does
not creep into the harmonized release. See
[RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §6 and §14.

- [ ] 16.6.1. Decide whether to add a `charged_macros` setting for
  control-flow domain-specific language (DSL) macros, a per-item threshold
  override and broader test-function detection.
  - Requires 16.5.2.
  - Base the decision on false-positive and false-negative reports gathered
    after the harmonized release (RFC 0010 open questions 3, 4 and 5).
  - Success: each option is either implemented with UI coverage or rejected
    in the ADR with its reasoning.
- [ ] 16.6.2. Offer the validated classifier and its evidence upstream to
  Clippy.
  - Requires 15.4.1.
  - See [RFC 0010](rfcs/0010-macro-aware-cognitive-complexity.md) §6.
  - Success: a comment or pull request on `rust-lang/rust-clippy` issue 14417
    links the ADR, the parity suite and the corpus report.

## 17. Validate not, only parse domain-modelling lints

The [Phase 17 DOMAIN roadmap amendment](roadmap-phase-17-domain-amendment.md)
is a normative supplement and governs conflicting Phase 17 DOMAIN task wording.

### 17.1. Family registration and shared evidence model

- [ ] 17.1.1. Reserve the `DOMAIN` rule family, allocate `DOMAIN001` to
  `DOMAIN099` for boundary refinement, `DOMAIN101` to `DOMAIN149` for type
  state spaces, `DOMAIN150` to `DOMAIN179` for local protocol and lifecycle
  surfaces, `DOMAIN180` to `DOMAIN199` for future local domain-modelling rules,
  and `DOMAIN900` to `DOMAIN999` for grouped reports. Integrate the family with
  experimental selector semantics. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Rule-code allocation.
  Requires 3.6.1.
- [ ] 17.1.2. Add `common::domain_model` with compiler-independent boundary,
  witness-disposition, source-use, field-role, constraint, evidence-strength,
  and construction-surface models. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Shared analysis architecture
  and [RFC 0012](rfcs/0012-fragile-types.md) §Shared state-space analysis.
  Requires 1.1.1.
- [ ] 17.1.3. Add resolved definition-path classifiers for `FromStr`,
  `str::parse`, `TryFrom`, `TryInto`, `NonZero*::new`, and configured custom
  refiners, including strict path validation and bounded warnings for malformed
  exclusions. See [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Initial
  refiner vocabulary and §Configuration. Requires 17.1.2.
- [ ] 17.1.4. Add local HIR use summaries and narrow MIR confirmation for
  success-edge reachability, source mutation, alias escape, and retained
  witnesses. Reuse the ownership-shape MIR contracts where they provide the
  required place-equivalence facts. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §HIR prefilter and §MIR
  confirmation. Requires 9.1.4 and 17.1.2.
- [ ] 17.1.5. Define a deterministic finding-record schema for crate-post and
  workspace aggregation, including rule code, item identity, source and witness
  types, normalized constraint or refinement fingerprint, evidence strength,
  and source spans. See [RFC 0011](rfcs/0011-weak-domain-boundaries.md)
  §Crate-post aggregation and [RFC 0012](rfcs/0012-fragile-types.md)
  §Diagnostic precedence and deduplication. Requires 17.1.2.

### 17.2. Weak-representation boundary rules

- [ ] 17.2.1. Create the `discarded_parsed_value` lint crate and implement the
  direct discarded-witness forms for `?` and explicit matches, including
  resolved source and witness types plus inward-use labels. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Proposed rule:
  `discarded_parsed_value`. Requires 2.1.1, 17.1.2, and 17.1.3.
- [ ] 17.2.2. Add mutation, alias, parser-probe, source-retention, raw-boundary,
  test, and generated-code controls to `discarded_parsed_value`. Emit
  machine-applicable suggestions only for proven local binding rewrites. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Use classification,
  §Suggestions, and §Exemptions and false-positive controls. Requires 17.1.4
  and 17.2.1.
- [ ] 17.2.3. Add pure, property, behaviour-driven, UI, and localized smoke
  coverage for `discarded_parsed_value`, including raw and parsed values
  carried together. See [RFC 0011](rfcs/0011-weak-domain-boundaries.md)
  §Testing requirements. Requires 1.2.1, 2.3.4, and 17.2.2.
- [ ] 17.2.4. Create `validation_without_refinement` at `allow`, summarize only
  private same-crate intrinsic validators with no unknown effects, and require
  success-path use of the unchanged weak value. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Proposed rule:
  `validation_without_refinement`. Requires 17.1.4 and 17.2.3.
- [ ] 17.2.5. Implement `repeated_boundary_refinement` as a grouped
  `whitaker check` advisory report using normalized parser and predicate
  fingerprints. See [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Deferred
  report: `repeated_boundary_refinement`. Requires 3.5.2, 17.1.5, and 17.2.4.

### 17.3. Fragile-type and state-space rules

This list is retained for traceability. The normative
[Phase 17 DOMAIN roadmap amendment](roadmap-phase-17-domain-amendment.md)
supersedes conflicting Phase 17.3 wording, including the former
`mutually_exclusive_bool_fields` task. Its expanded catalogue and acceptance
corpus govern current scope; non-conflicting tasks remain applicable.

- [ ] 17.3.1. Implement type-level field-role, truth-table, constraint,
  construction-surface, and evidence-strength collection in
  `common::domain_model::state_space`. Add bounded exhaustive tests for
  presence and boolean constraints up to four fields. See
  [RFC 0012](rfcs/0012-fragile-types.md) §Shared state-space analysis and
  §Testing requirements. Requires 17.1.2.
- [ ] 17.3.2. Create the `manual_tagged_union` lint crate and detect exhaustive
  discriminator-to-payload mappings with rejected mismatches, including common
  metadata outside the candidate payload group. See
  [RFC 0012](rfcs/0012-fragile-types.md) §Proposed rule: `manual_tagged_union`.
  Requires 2.1.1 and 17.3.1.
- [ ] 17.3.3. Create `correlated_optional_fields` at `allow`, recognize
  exactly-one, at-most-one, all-or-none, and implication relationships, and
  require one exhaustive truth table or two independent evidence sites. See
  [RFC 0012](rfcs/0012-fragile-types.md) §Proposed rule:
  `correlated_optional_fields`. Requires 17.3.1 and 17.3.2.
- [ ] 17.3.4. Trial Clippy's `struct_excessive_bools` against the target corpus
  and implement `mutually_exclusive_bool_fields` only for residual cases where
  explicit correlation evidence identifies a state subset that field counts do
  not. See [RFC 0012](rfcs/0012-fragile-types.md) §Proposed rule:
  `mutually_exclusive_bool_fields`. Requires 17.3.1.
- [ ] 17.3.5. Create `bypassable_type_invariant` at `allow`, combining
  intrinsic invariant summaries with effectively visible public fields,
  unchecked constructors, independent setters, mutable inner access, and
  deserialization paths. Add builder, raw-model, FFI, runtime-resource, and
  external-contract suppressions. See [RFC 0012](rfcs/0012-fragile-types.md)
  §Proposed rule: `bypassable_type_invariant`. Requires 17.1.4 and 17.3.1.
- [ ] 17.3.6. Implement `invalid_default` as a report-only symbolic evaluator
  for direct constants and known standard defaults against supported intrinsic
  constraints. Unknown facts must suppress rather than approximate. See
  [RFC 0012](rfcs/0012-fragile-types.md) §Proposed report: `invalid_default`.
  Requires 17.3.1 and 17.3.5.

### 17.4. Integration, documentation, and promotion

- [ ] 17.4.1. Add implemented `DOMAIN` rules to the experimental suite with
  independent feature gates, stable rule metadata, and diagnostic precedence
  that emits one primary finding per correlated field group. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Category, not monolith and
  [RFC 0012](rfcs/0012-fragile-types.md) §Diagnostic precedence and
  deduplication. Requires 17.2.3, 17.3.2, and 17.3.3.
- [ ] 17.4.2. Add English, Welsh, and Gaelic Fluent entries and structured
  diagnostic arguments for every implemented rule. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Diagnostics and localization
  and [RFC 0012](rfcs/0012-fragile-types.md) §Diagnostics and localization.
  Requires 2.3.4 and 17.4.1.
- [ ] 17.4.3. Update the users' and developers' guides with family selection,
  intrinsic-versus-extrinsic validity, source-preserving parsed values,
  builder/raw-type exemptions, suppression guidance, and staged migration
  examples. Requires 17.4.1.
- [ ] 17.4.4. Run the family over representative df12 and third-party corpora,
  classify findings by the RFC exception models, publish the false-positive
  budget, and promote each rule independently. `DOMAIN001`, `DOMAIN101`, and
  `DOMAIN106` should receive the first promotion decisions. `DOMAIN107`,
  `DOMAIN108`, and `DOMAIN151` remain experimental or report-only until their
  proof boundaries are stable. See
  [RFC 0011](rfcs/0011-weak-domain-boundaries.md) §Downstream corpus and
  [RFC 0012](rfcs/0012-fragile-types.md) §Downstream corpus. Requires 17.4.2
  and 17.4.3.
