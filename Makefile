.PHONY: help all clean test build release lint fmt check-fmt markdownlint nixie publish-check

APP ?= whitaker
CARGO ?= cargo
BUILD_JOBS ?=
CARGO_FLAGS ?= --workspace --all-targets --all-features
TEST_EXCLUDES ?= --exclude rustc_ast --exclude rustc_attr_data_structures --exclude rustc_hir --exclude rustc_lint --exclude rustc_middle --exclude rustc_session --exclude rustc_span --exclude whitaker --exclude function_attrs_follow_docs --exclude module_max_lines --exclude no_expect_outside_tests
TEST_CARGO_FLAGS ?= --workspace --all-targets --all-features $(TEST_EXCLUDES)
RUST_FLAGS ?= -D warnings
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
MDLINT ?= markdownlint
NIXIE ?= nixie
PUBLISH_CHECK_FLAGS ?= # Flags passed to Lading publish; override via env or caller.
LADING ?= uvx --from git+https://github.com/leynos/lading lading

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-Z force-unstable-if-unmarked $(RUST_FLAGS)" $(CARGO) test $(TEST_CARGO_FLAGS) $(BUILD_JOBS)

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(CARGO) clippy $(CARGO_FLAGS) -- $(RUST_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	$(MDLINT) '**/*.md'

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

publish-check: ## Run Lading publish pre-flight checks (override flags via PUBLISH_CHECK_FLAGS)
	$(LADING) publish $(PUBLISH_CHECK_FLAGS) --workspace-root $(CURDIR)

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
