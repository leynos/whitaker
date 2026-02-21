.PHONY: help all clean test build release lint fmt check-fmt markdownlint nixie publish-check typecheck install-smoke package-lints workflow-test

APP ?= whitaker
CARGO ?= cargo
BUILD_JOBS ?=
CARGO_FLAGS ?= --workspace --all-targets --all-features
TEST_EXCLUDES ?= --exclude rustc_ast --exclude rustc_attr_data_structures --exclude rustc_hir --exclude rustc_lint --exclude rustc_middle --exclude rustc_session --exclude rustc_span --exclude whitaker --exclude function_attrs_follow_docs --exclude module_max_lines --exclude no_expect_outside_tests
TEST_CARGO_FLAGS ?= $(CARGO_FLAGS) $(TEST_EXCLUDES)
RUST_FLAGS ?= -D warnings
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
MDLINT ?= markdownlint-cli2
NIXIE ?= nixie
WHITAKER_REPO ?= $(CURDIR)
WHITAKER_REV ?= HEAD
PUBLISH_PACKAGES ?=
LINT_CRATES ?= bumpy_road_function conditional_max_n_branches function_attrs_follow_docs module_max_lines module_must_have_inner_docs no_expect_outside_tests no_std_fs_operations no_unwrap_or_else_panic whitaker_suite
CARGO_DYLINT_VERSION ?= 5.0.0
DYLINT_LINK_VERSION ?= 5.0.0
WHITAKER_SCRIPT ?= $(HOME)/.local/bin/whitaker

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	@command -v cargo-nextest >/dev/null || { echo "Install cargo-nextest (cargo install cargo-nextest)"; exit 1; }
	@# Prefer dynamic linking during local `cargo test` runs to avoid rustc_private
	@# linkage pitfalls when building cdylib-based lints; `publish-check` omits
	@# this flag to exercise production-like linking behaviour.
	@# Run tests with backup/restore safeguard in a single shell with trap
	@# to ensure cleanup runs even when tests fail.
	@set -eu; \
	WHITAKER_BACKUP=""; \
	HAD_WHITAKER=false; \
	cleanup() { \
		EXIT_CODE=$$?; \
		if [ -n "$$WHITAKER_BACKUP" ] && [ -f "$$WHITAKER_BACKUP" ]; then \
			if [ "$$HAD_WHITAKER" = "true" ]; then \
				if [ ! -f "$(WHITAKER_SCRIPT)" ] || ! diff -q "$(WHITAKER_SCRIPT)" "$$WHITAKER_BACKUP" >/dev/null 2>&1; then \
					echo "ERROR: Tests modified $(WHITAKER_SCRIPT) - restoring backup"; \
					cp "$$WHITAKER_BACKUP" "$(WHITAKER_SCRIPT)"; \
					rm -f "$$WHITAKER_BACKUP"; \
					exit 1; \
				fi; \
			fi; \
			rm -f "$$WHITAKER_BACKUP"; \
		elif [ "$$HAD_WHITAKER" = "false" ] && [ -f "$(WHITAKER_SCRIPT)" ]; then \
			echo "ERROR: Tests created $(WHITAKER_SCRIPT) (file did not exist before tests)"; \
			if [ -n "$${CI:-}" ] || [ -n "$${WHITAKER_TEST_STRICT:-}" ]; then \
				echo "Cleaning up $(WHITAKER_SCRIPT) because strict test mode is enabled (CI/WHITAKER_TEST_STRICT)"; \
				rm -f "$(WHITAKER_SCRIPT)"; \
			else \
				echo "Leaving $(WHITAKER_SCRIPT) in place (not running under CI; set WHITAKER_TEST_STRICT=1 to enforce cleanup)"; \
			fi; \
			exit 1; \
		fi; \
		exit $$EXIT_CODE; \
	}; \
	trap cleanup EXIT; \
	WHITAKER_BACKUP=$$(mktemp "$${TMPDIR:-/tmp}/.whitaker-test-backup-XXXXXX"); \
	if cp "$(WHITAKER_SCRIPT)" "$$WHITAKER_BACKUP" 2>/dev/null; then \
		HAD_WHITAKER=true; \
	else \
		rm -f "$$WHITAKER_BACKUP"; \
		WHITAKER_BACKUP=""; \
	fi; \
	RUSTFLAGS="-C prefer-dynamic -Z force-unstable-if-unmarked $(RUST_FLAGS)" $(CARGO) nextest run $(TEST_CARGO_FLAGS) $(BUILD_JOBS); \
	if [ "$${ACT_WORKFLOW_TESTS:-0}" = "1" ]; then \
		$(MAKE) workflow-test; \
	fi

workflow-test: ## Run opt-in GitHub workflow smoke tests with act + pytest
	@command -v act >/dev/null || { echo "Install act to run workflow tests"; exit 1; }
	@command -v python3 >/dev/null || { echo "python3 is required for workflow tests"; exit 1; }
	@ACT_WORKFLOW_TESTS=1 python3 -m pytest tests/workflows

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

typecheck:
	RUSTFLAGS="-C prefer-dynamic -Z force-unstable-if-unmarked $(RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS)

install-smoke: ## Install whitaker-installer and verify basic functionality
	set -eu; \
	TMP_DIR=$$(mktemp -d); \
	trap 'rm -rf "$$TMP_DIR"' 0 INT TERM HUP; \
	$(CARGO) install --path installer --root "$$TMP_DIR" --locked; \
	export PATH="$$TMP_DIR/bin:$$PATH"; \
	SYSROOT=$$(rustc --print sysroot); \
	HOST_TRIPLE=$$(rustc -vV | awk -F ': ' '/host:/ {print $$2}'); \
	RUSTLIB_DIR="$$SYSROOT/lib/rustlib/$$HOST_TRIPLE/lib"; \
	export LD_LIBRARY_PATH="$$RUSTLIB_DIR:$${LD_LIBRARY_PATH:-}"; \
	command -v whitaker-installer >/dev/null; \
	whitaker-installer --help >/dev/null; \
	whitaker-installer --version >/dev/null

publish-check: ## Build, test, and validate packages before publishing
	@command -v cargo-nextest >/dev/null || { echo "Install cargo-nextest (cargo install cargo-nextest)"; exit 1; }
	PINNED_TOOLCHAIN=$$(awk -F '\"' '/^channel/ {print $$2}' rust-toolchain.toml); \
	TOOLCHAIN="$$PINNED_TOOLCHAIN"; \
	ORIG_DIR="$(CURDIR)"; \
	rustup component add --toolchain "$$TOOLCHAIN" rust-src rustc-dev llvm-tools-preview; \
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) build --workspace --all-features $(BUILD_JOBS); \
	RUSTFLAGS="-Z force-unstable-if-unmarked $(RUST_FLAGS)" $(CARGO) +$$TOOLCHAIN nextest run $(TEST_CARGO_FLAGS) $(BUILD_JOBS); \
	TMP_DIR=$$(mktemp -d); \
	trap 'rm -rf "$$TMP_DIR"' 0 INT TERM HUP; \
	if ! command -v cargo-dylint >/dev/null 2>&1; then \
		$(CARGO) install --locked --version $(CARGO_DYLINT_VERSION) cargo-dylint; \
	fi; \
	if ! command -v dylint-link >/dev/null 2>&1; then \
		$(CARGO) install --locked --version $(DYLINT_LINK_VERSION) dylint-link; \
	fi; \
	TARGET_DIR="$$TMP_DIR/target"; \
	git clone "$(WHITAKER_REPO)" "$$TMP_DIR/whitaker-src"; \
	cd "$$TMP_DIR/whitaker-src" && { \
		CLONE_HEAD=$$(git rev-parse HEAD); \
		TARGET_REV=$${GIT_TAG:-$${WHITAKER_REV:-$$CLONE_HEAD}}; \
		git checkout "$$TARGET_REV"; \
		for lint in $(LINT_CRATES); do \
			CARGO_TARGET_DIR="$$TARGET_DIR" RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) +$$TOOLCHAIN build --release --features dylint-driver -p $$lint; \
			mkdir -p "$$TARGET_DIR/dylint/libraries/$$TOOLCHAIN/release"; \
			cp "$$TARGET_DIR/release/lib$$lint.so" "$$TARGET_DIR/dylint/libraries/$$TOOLCHAIN/release/lib$$lint@$$TOOLCHAIN.so"; \
		done; \
		DYLINT_LIBRARY_PATH="$$TARGET_DIR/dylint/libraries/$$TOOLCHAIN/release" CARGO_TARGET_DIR="$$TARGET_DIR" $(CARGO) +$$TOOLCHAIN dylint list --no-metadata --no-build; \
	}; \
	cd "$$ORIG_DIR"; \
	for crate in $(PUBLISH_PACKAGES); do \
		$(CARGO) package -p $$crate --allow-dirty; \
	done

package-lints: ## Build lint crates and package as .tar.zst archives
	set -eu; \
	TOOLCHAIN=$$(awk -F '"' '/^channel/ {print $$2}' rust-toolchain.toml); \
	HOST_TRIPLE=$$(rustc -vV | awk -F ': ' '/host:/ {print $$2}'); \
	SHA=$$(git rev-parse --short HEAD); \
	DIST_DIR="$(CURDIR)/dist"; \
	mkdir -p "$$DIST_DIR"; \
	for lint in $(LINT_CRATES); do \
		RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) +$$TOOLCHAIN build --release --features dylint-driver -p $$lint; \
	done; \
	$(CARGO) run -p whitaker-installer --bin whitaker-package-lints -- \
		--git-sha "$$SHA" \
		--toolchain "$$TOOLCHAIN" \
		--target "$$HOST_TRIPLE" \
		--output-dir "$$DIST_DIR" \
		--release-dir target/release

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
