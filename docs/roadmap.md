# Development Roadmap

- [ ] Phase 0 — Repository scaffolding
  - [ ] Initialise the Cargo workspace with workspace members and rust-toolchain
        pin.
  - [ ] Add baseline project metadata (`README`, `LICENSE`, contributing guide,
        CODEOWNERS).
  - [ ] Establish Makefile or justfile targets for fmt, lint, test, and UI test
        orchestration.

- [ ] Phase 1 — Common infrastructure
  - [x] Implement the `common` crate helpers for attributes, context detection,
        spans, and diagnostics.
  - [x] Wire shared configuration loading with
        `dylint_linting::config_or_default` and serde structs.
  - [x] Integrate `dylint_testing` harness boilerplate for UI tests across all
        lint crates.

- [ ] Phase 2 — Core lint delivery
  - [x] Establish the lint crate template with shared dependencies and UI test
        harness boilerplate.
  - [x] Implement `function_attrs_follow_docs` with targeted UI scenarios.
  - [x] Implement `no_expect_outside_tests` with context-aware diagnostics.
  - [ ] Implement `public_fn_must_have_docs` using effective visibility data.
  - [ ] Implement `test_must_not_have_example` covering code-fence heuristics.
  - [ ] Implement `module_max_lines` with configurable thresholds.
  - [ ] Implement `conditional_max_two_branches` for complex predicates.
  - [ ] Implement `no_unwrap_or_else_panic` with optional `clippy` helpers.

- [ ] Phase 2a — Localisation enablement
  - [ ] Add `fluent-templates` and `once_cell` to the workspace dependencies
        and expose a shared `common::i18n` loader.
  - [ ] Create the `locales/` resource tree with an `en-GB` fallback and a
        secondary language sample covering every lint slug.
  - [ ] Refactor lint diagnostics to source primary messages, notes, and help
        text from Fluent bundles with structured arguments.
  - [ ] Allow locale selection via `DYLINT_LOCALE` and `dylint.toml`, and add UI
        smoke tests that run under at least one non-English locale.

- [ ] Phase 3 — Aggregated packaging and installer
  - [ ] Assemble the `suite` cdylib using constituent features and combined lint
        pass wiring.
  - [ ] Implement the installer CLI that builds, links, and stages all lint
        libraries.
  - [ ] Provide consumer guidance and workspace metadata examples in
        documentation.

- [ ] Phase 4 — Quality gates and automation
  - [ ] Configure CI workflows for fmt, clippy, dylint runs, and per-crate UI
        tests on a multi-OS matrix.
  - [ ] Add markdownlint, nixie, and other doc/tooling checks to the pipeline.
  - [ ] Enforce lint-level deny rules and fail builds on warnings across the
        workspace.

- [ ] Phase 5 — Experimental Bumpy Road lint
  - [ ] Implement the per-line complexity signal builder and smoothing window
        logic.
  - [ ] Detect bump intervals, surface diagnostics with labelled spans, and add
        configuration options.
  - [ ] Ship UI coverage for positive and negative Bumpy Road scenarios and gate
        behind a feature flag.
