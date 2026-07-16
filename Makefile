.PHONY: help all clean test coverage build release lint fmt check-fmt markdownlint nixie publish-check typecheck install-smoke release-installer-dry-run package-lints workflow-test workflow-test-deps test-workflow-contracts verus kani verus-clone-detector kani-clone-detector spelling spelling-config spelling-config-write spelling-phrase-check spelling-helper-test

# Appended only on targets that invoke binaries commonly installed under these
# prefixes (cargo/bun/user-local), so the default recipe environment stays
# aligned with the caller's PATH.
TOOL_PATH_SUFFIX = $$HOME/.cargo/bin:$$HOME/.bun/bin:$$HOME/.local/bin

APP ?= whitaker-installer
PATH := $(HOME)/.cargo/bin:$(HOME)/.bun/bin:$(PATH)
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(shell [ -x "$(HOME)/.cargo/bin/cargo" ] && echo "$(HOME)/.cargo/bin/cargo"))
BUILD_JOBS ?=
CARGO_FLAGS ?= --workspace --all-targets --all-features
TEST_EXCLUDES ?= --exclude rustc_ast --exclude rustc_attr_data_structures --exclude rustc_hir --exclude rustc_lint --exclude rustc_middle --exclude rustc_session --exclude rustc_span --exclude whitaker --exclude function_attrs_follow_docs --exclude module_max_lines --exclude no_expect_outside_tests
TEST_CARGO_FLAGS ?= $(CARGO_FLAGS) $(TEST_EXCLUDES)
NEXTEST_PROFILE ?=
# The cargo test driver. `test` runs `cargo nextest run`; `coverage`
# overrides this with `cargo llvm-cov nextest ...` so instrumentation runs
# over the exact same crate subset (TEST_CARGO_FLAGS) and RUSTFLAGS.
TEST_RUNNER ?= nextest run
COVERAGE_OUTPUT ?= lcov.info
RUST_FLAGS ?= -D warnings
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(HOME)/.bun/bin/markdownlint-cli2)
NIXIE ?= nixie
WHITAKER_REPO ?= $(CURDIR)
WHITAKER_REV ?= HEAD
PUBLISH_PACKAGES ?=
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
RUFF_VERSION ?= 0.15.12
PATHSPEC_VERSION ?= 1.1.1
TYPOS_VERSION ?= 1.48.0
TYPOS_CONFIG_BUILDER_COMMIT := b604f198797fdd36a567dd0f8f07b13f9539b241
TYPOS_CONFIG_BUILDER_SOURCE := git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_COMMIT)
TYPOS_CONFIG_BUILDER := $(UV_ENV) $(UV) tool run --python 3.14 \
	--from "$(TYPOS_CONFIG_BUILDER_SOURCE)" typos-config-builder
SPELLING_PY_SRCS := \
	scripts/typos_rollout_check.py scripts/tests/test_typos_rollout_check.py
SPELLING_PY_TESTS := scripts/tests/test_typos_rollout_check.py
SPELLING_PY_ENV := PYTHONDONTWRITEBYTECODE=1
SPELLING_COVERAGE_FILE ?= /tmp/whitaker-spelling-helper.coverage
SPELLING_COVERAGE_ARGS := --cov=typos_rollout_check --cov-fail-under=90
SPELLING_HELPER_PYTEST = PYTHONPATH=scripts $(SPELLING_PY_ENV) \
	COVERAGE_FILE=$(SPELLING_COVERAGE_FILE) $(UV_ENV) $(UV) run --no-project \
	--python 3.14 --with pathspec==$(PATHSPEC_VERSION) --with pytest==9.0.2 \
	--with pytest-cov==7.0.0 python -m pytest
WORKFLOW_TEST_VENV ?= .venv
LINT_CRATES ?= bumpy_road_function conditional_max_n_branches function_attrs_follow_docs module_max_lines module_must_have_inner_docs no_expect_outside_tests test_must_not_have_example no_std_fs_operations no_unwrap_or_else_panic whitaker_suite
CARGO_DYLINT_VERSION ?= 5.0.0
DYLINT_LINK_VERSION ?= 5.0.0
WHITAKER_SCRIPT ?= $(HOME)/.local/bin/whitaker

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artefacts
	$(CARGO) clean
	rm -rf .uv-cache .uv-tools

test: ## Run tests with warnings treated as errors
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; command -v cargo-nextest >/dev/null || { echo "Install cargo-nextest (cargo install cargo-nextest)"; exit 1; }
	@# Prefer dynamic linking during local `cargo test` runs to avoid rustc_private
	@# linkage pitfalls when building cdylib-based lints; `publish-check` omits
	@# this flag to exercise production-like linking behaviour.
	@# Run tests with backup/restore safeguard in a single shell with trap
	@# to ensure cleanup runs even when tests fail.
	@set -eu; \
	export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; \
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
	RUSTFLAGS="-C prefer-dynamic -Z force-unstable-if-unmarked $(RUST_FLAGS)" $(CARGO) $(TEST_RUNNER) $(TEST_CARGO_FLAGS) $(BUILD_JOBS) $(if $(NEXTEST_PROFILE),--profile $(NEXTEST_PROFILE)); \
	if [ "$${ACT_WORKFLOW_TESTS:-0}" = "1" ]; then \
		$(MAKE) workflow-test; \
	fi

coverage: ## Generate LCOV coverage over the CI-tested crate subset
	@# Reuse the `test` recipe verbatim (same TEST_CARGO_FLAGS excludes,
	@# same prefer-dynamic RUSTFLAGS, same WHITAKER_SCRIPT safeguard) but
	@# swap the driver to `cargo llvm-cov nextest`. This keeps the
	@# instrumented run in lockstep with the plain test run: the 11
	@# CI-excluded crates (rustc_* proxy shims, the whitaker root, and the
	@# three lint crates whose dylint UI tests are excluded) stay excluded,
	@# so coverage never attempts a bare `--workspace` build the suite
	@# cannot support.
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; command -v cargo-llvm-cov >/dev/null || { echo "Install cargo-llvm-cov (cargo install cargo-llvm-cov)"; exit 1; }
	@$(MAKE) test TEST_RUNNER="llvm-cov nextest --lcov --output-path $(COVERAGE_OUTPUT)"

workflow-test: workflow-test-deps ## Run opt-in GitHub workflow smoke tests with act + pytest
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; command -v act >/dev/null || { echo "Install act to run workflow tests"; exit 1; }
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; command -v $(UV) >/dev/null || { echo "uv is required for workflow tests"; exit 1; }
	@test -x "$(WORKFLOW_TEST_VENV)/bin/python" || { \
		echo "workflow-test virtualenv is missing or invalid:"; \
		echo "  expected: $(WORKFLOW_TEST_VENV)/bin/python"; \
		echo "Run 'make workflow-test-deps' to create or refresh the virtualenv."; \
		exit 1; \
	}
	@ACT_WORKFLOW_TESTS=1 $(WORKFLOW_TEST_VENV)/bin/python -m pytest tests/workflows

test-workflow-contracts: ## Validate the mutation-testing caller contract
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; command -v $(UV) >/dev/null || { echo "uv is required for workflow contract tests"; exit 1; }
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; $(UV) run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

workflow-test-deps: ## Install Python dependencies for workflow tests
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; command -v $(UV) >/dev/null || { echo "uv is required for workflow tests"; exit 1; }
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; $(UV) venv --allow-existing $(WORKFLOW_TEST_VENV)
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; $(UV) pip install --python $(WORKFLOW_TEST_VENV)/bin/python -r tests/workflows/requirements.txt

target/%/$(APP): ## Build binary in debug or release mode
	manifest=$$(grep -l whitaker-installer */Cargo.toml crates/*/Cargo.toml); \
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP) --manifest-path "$$manifest"

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(CARGO) clippy $(CARGO_FLAGS) -- $(RUST_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: spelling ## Lint Markdown files and enforce spelling
	export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; $(MDLINT) '**/*.md' '!**/.uv-cache/**' '!**/.uv-tools/**'

spelling: spelling-phrase-check ## Enforce en-GB-oxendict in tracked text
	@git ls-files -z | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude --hidden

spelling-phrase-check: spelling-config ## Reject prohibited spelling phrases
	@PYTHONPATH=scripts $(SPELLING_PY_ENV) $(UV_ENV) $(UV) run --no-project --python 3.14 \
		scripts/typos_rollout_check.py --repository .

spelling-config: spelling-helper-test ## Verify generated spelling configuration
	@git ls-files --error-unmatch typos.toml >/dev/null
	@$(TYPOS_CONFIG_BUILDER) --repository . --check

spelling-config-write: spelling-helper-test ## Generate spelling configuration
	@$(TYPOS_CONFIG_BUILDER) --repository .

spelling-helper-test: ## Validate the shared spelling-policy integration
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SPELLING_PY_SRCS)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SPELLING_PY_SRCS)
	@$(SPELLING_HELPER_PYTEST) $(SPELLING_PY_TESTS) -c /dev/null \
		--rootdir=. -p no:cacheprovider $(SPELLING_COVERAGE_ARGS)

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; $(NIXIE) --no-sandbox

typecheck:
	RUSTFLAGS="-C prefer-dynamic -Z force-unstable-if-unmarked $(RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS)

verus: ## Run the pinned Verus proof sidecar
	./scripts/run-verus.sh

verus-clone-detector: ## Run clone-detector Verus proofs
	./scripts/run-verus.sh clone-detector

kani: ## Run practical Kani sidecar harnesses
	./scripts/run-kani.sh

kani-clone-detector: ## Run clone-detector Kani harnesses
	./scripts/run-kani.sh clone-detector

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

release-installer-dry-run: ## Build and package the host-platform installer archive
	set -eu; \
	PYTHON=$$(command -v python3 || command -v python || true); \
	if [ -z "$$PYTHON" ]; then \
		echo "Install python3 or python to run release-installer-dry-run"; \
		exit 1; \
	fi; \
	[ -n "$(CARGO)" ] || { echo "Install cargo to run release-installer-dry-run"; exit 1; }; \
	for tool in awk jq mktemp rustc; do \
		command -v "$$tool" >/dev/null || { echo "Install $$tool to run release-installer-dry-run"; exit 1; }; \
	done; \
	TMP_DIR=$$(mktemp -d); \
	trap 'rm -rf "$$TMP_DIR"' 0 INT TERM HUP; \
	HOST_TRIPLE=$$(rustc -vV | awk -F ': ' '/host:/ {print $$2}'); \
	VERSION=$$($(CARGO) metadata --manifest-path installer/Cargo.toml --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "whitaker-installer") | .version'); \
	if [ -z "$$VERSION" ]; then \
		echo "Failed to extract whitaker-installer version from Cargo metadata"; \
		exit 1; \
	fi; \
	$(CARGO) build $(BUILD_JOBS) -p whitaker-installer --release --target "$$HOST_TRIPLE"; \
	$(CARGO) build $(BUILD_JOBS) --release -p whitaker-installer --bin whitaker-package-installer --target "$$HOST_TRIPLE"; \
	DIST_DIR="$$TMP_DIR/dist"; \
	mkdir -p "$$DIST_DIR"; \
	case "$$HOST_TRIPLE" in \
		*windows*) \
			INSTALLER_BIN="target/$$HOST_TRIPLE/release/whitaker-installer.exe"; \
			PACKAGER="./target/$$HOST_TRIPLE/release/whitaker-package-installer.exe"; \
			ARCHIVE_GLOB="$$DIST_DIR/*.zip"; \
			;; \
		*) \
			INSTALLER_BIN="target/$$HOST_TRIPLE/release/whitaker-installer"; \
			PACKAGER="./target/$$HOST_TRIPLE/release/whitaker-package-installer"; \
			ARCHIVE_GLOB="$$DIST_DIR/*.tgz"; \
			;; \
	esac; \
	"$$PACKAGER" \
		--crate-version "$$VERSION" \
		--target "$$HOST_TRIPLE" \
		--binary-path "$$INSTALLER_BIN" \
		--output-dir "$$DIST_DIR"; \
	"$$PYTHON" scripts/generate_checksums.py "$$DIST_DIR"; \
	found_archive=false; \
	for archive in $$ARCHIVE_GLOB; do \
		if [ -f "$$archive" ]; then \
			found_archive=true; \
			break; \
		fi; \
	done; \
	if [ "$$found_archive" != "true" ]; then \
		echo "Expected installer archive matching $$ARCHIVE_GLOB"; \
		exit 1; \
	fi; \
	found_checksum=false; \
	for checksum in "$$DIST_DIR"/*.sha256; do \
		if [ -f "$$checksum" ]; then \
			found_checksum=true; \
			break; \
		fi; \
	done; \
	if [ "$$found_checksum" != "true" ]; then \
		echo "Expected installer checksum in $$DIST_DIR"; \
		exit 1; \
	fi

publish-check: ## Build, test, and validate packages before publishing
	@export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; command -v cargo-nextest >/dev/null || { echo "Install cargo-nextest (cargo install cargo-nextest)"; exit 1; }
	export PATH="$$PATH:$(TOOL_PATH_SUFFIX)"; \
	PINNED_TOOLCHAIN=$$(awk -F '\"' '/^channel/ {print $$2}' rust-toolchain.toml); \
	TOOLCHAIN="$$PINNED_TOOLCHAIN"; \
	ORIG_DIR="$(CURDIR)"; \
	rustup component add --toolchain "$$TOOLCHAIN" rust-src rustc-dev llvm-tools-preview; \
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) build --workspace --all-features $(BUILD_JOBS); \
	RUSTFLAGS="-Z force-unstable-if-unmarked $(RUST_FLAGS)" $(CARGO) +$$TOOLCHAIN nextest run --profile ci $(TEST_CARGO_FLAGS) $(BUILD_JOBS); \
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
