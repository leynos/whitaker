"""Shell stubs that stand in for Cargo and the tools the Makefile invokes.

The lock-mode contract tests run real `make` recipes against these stubs, so a
recipe's Cargo arguments and environment can be recorded without resolving a
dependency graph or touching the network.
"""

from __future__ import annotations

from pathlib import Path


def _write_stub(directory: Path, name: str, body: str) -> Path:
    """Write an executable shell stub and return its path."""
    path = directory / name
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(0o755)
    return path


def _write_cargo_stub(directory: Path) -> Path:
    """Write a Cargo stand-in that records arguments and selected environment."""
    return _write_stub(
        directory,
        "cargo",
        '''echo "$@" >> "$CARGO_LOCKED_LOG"
printf '%s\t%s\t%s\t%s\t%s\t%s\n' "${CARGO_NET_OFFLINE-<unset>}" "${CARGO-<unset>}" \
    "${CARGO_REAL-<unset>}" "${DYLINT_DRIVER_PATH-<unset>}" "$*" "${RUSTUP_TOOLCHAIN-<unset>}" \
    >> "$CARGO_LOCKED_ENV_LOG"
case "$1" in
metadata) echo '{"packages":[{"name":"whitaker-installer","version":"0.2.5"}]}' ;;
build)
    target=""
    for argument in "$@"; do
        if [ "$previous" = "--target" ]; then target="$argument"; fi
        previous="$argument"
    done
    mkdir -p "target/$target/release"
    : > "target/$target/release/whitaker-installer"
    cat > "target/$target/release/whitaker-package-installer" <<'EOF'
#!/bin/sh
for argument in "$@"; do
    if [ "$previous" = "--output-dir" ]; then output_dir="$argument"; fi
    previous="$argument"
done
touch "$output_dir/whitaker-installer.tgz"
EOF
    chmod 755 "target/$target/release/whitaker-package-installer"
    ;;
esac''',
    )


def _write_msrv_check_cargo_stub(directory: Path) -> Path:
    """Write a Cargo stand-in that packages and installs a minimal crate archive."""
    return _write_stub(
        directory,
        "cargo",
        '''echo "$@" >> "$CARGO_LOCKED_LOG"
if [ "$1" = "+1.85.0" ]; then shift; fi
case "$1" in
package)
    package_source="$CARGO_TARGET_DIR/package-source/whitaker-installer-0.2.5"
    mkdir -p "$package_source" "$CARGO_TARGET_DIR/package"
    printf '[package]\nname = "whitaker-installer"\nversion = "0.2.5"\n' \
        > "$package_source/Cargo.toml"
    tar -czf "$CARGO_TARGET_DIR/package/whitaker-installer-0.2.5.crate" \\
        -C "${package_source%/*}" "${package_source##*/}"
    ;;
install)
    root=""
    path=""
    previous=""
    for argument in "$@"; do
        if [ "$previous" = "--root" ]; then root="$argument"; fi
        if [ "$previous" = "--path" ]; then path="$argument"; fi
        previous="$argument"
    done
    test -f "$path/Cargo.toml"
    grep -qx 'name = "whitaker-installer"' "$path/Cargo.toml"
    mkdir -p "$root/bin"
    printf '%s\\n' "$path" > "$root/.installed-from-packaged-crate"
    printf '#!/bin/sh\\ntest -f "$(dirname "$0")/../.installed-from-packaged-crate"\\n' \
        > "$root/bin/whitaker-installer"
    chmod 755 "$root/bin/whitaker-installer"
    ;;
esac''',
    )


def _write_tool_stubs(stub_dir: Path) -> None:
    """Write stand-ins for the non-Cargo tools `test`/`publish-check` invoke."""
    _write_stub(
        stub_dir,
        "cargo-nextest",
        'echo "$@" >> "$CARGO_LOCKED_LOG"\n'
        'printf \'%s\\t%s\\t%s\\t%s\\t%s\\t%s\\n\' "${CARGO_NET_OFFLINE-<unset>}" '
        '"${CARGO-<unset>}" "${CARGO_REAL-<unset>}" '
        '"${DYLINT_DRIVER_PATH-<unset>}" "$*" "${RUSTUP_TOOLCHAIN-<unset>}" '
        '>> "$CARGO_LOCKED_ENV_LOG"',
    )
    _write_stub(stub_dir, "rustup", "exit 0")
    _write_stub(
        stub_dir,
        "git",
        # `git clone <repo> <dest>` must create <dest>; `git rev-parse` prints a
        # placeholder SHA; other subcommands (checkout, ...) are no-ops.
        'case "$1" in\n'
        '    clone) mkdir -p "$3" ;;\n'
        "    rev-parse) echo 0000000000000000000000000000000000000000 ;;\n"
        "esac",
    )


def _write_publish_check_cargo_stub(directory: Path) -> Path:
    """Cargo stand-in for `publish-check`: records args and fakes per-lint libs.

    A per-lint `cargo build ... -p <lint>` is expected to produce
    `$CARGO_TARGET_DIR/release/lib<lint>.so`, which the recipe then copies, so
    the stub materializes that file when it sees such a build.
    """
    return _write_stub(
        directory,
        "cargo",
        'echo "$@" >> "$CARGO_LOCKED_LOG"\n'
        'lint=""\n'
        'previous=""\n'
        'for argument in "$@"; do\n'
        '    if [ "$previous" = "-p" ]; then lint="$argument"; fi\n'
        '    previous="$argument"\n'
        "done\n"
        'case " $* " in\n'
        '    *" build "*)\n'
        '        if [ -n "$lint" ] && [ -n "${CARGO_TARGET_DIR:-}" ]; then\n'
        '            mkdir -p "$CARGO_TARGET_DIR/release"\n'
        '            : > "$CARGO_TARGET_DIR/release/lib$lint.so"\n'
        "        fi\n"
        "        ;;\n"
        "esac",
    )

