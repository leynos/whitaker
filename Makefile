.PHONY: help all clean test build release lint fmt check-fmt markdownlint nixie publish-check

APP ?= whitaker
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
MDLINT ?= markdownlint
NIXIE ?= nixie
PUBLISH_CHECK_FLAGS ?= --access public # Flags passed to Lading publish; override via env or caller.
LADING ?= uvx --from git+https://github.com/leynos/lading lading

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-Z force-unstable-if-unmarked -D warnings" $(CARGO) test --all-targets --no-default-features $(BUILD_JOBS)

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)

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
