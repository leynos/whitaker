#!/usr/bin/env bash
# record-sccache-effectiveness.sh — Publish the compiler-cache evidence a
# reviewer needs, and fail loudly when sccache wrapped nothing.
#
# Usage:
#   scripts/record-sccache-effectiveness.sh
#
# Writes `sccache-stats.txt` and `sccache-stats.json` to the working
# directory and appends the human-readable block to the job summary, headed
# by `SETUP_RUST_CACHE_BACKEND`, the backend `setup-rust` selected. `Cache
# location` reads `ghac` for Ubicloud's proxy and GitHub's own service alike,
# so the statistics cannot be interpreted without it.
#
# Zero compile requests means `RUSTC_WRAPPER` never reached the compiler
# invocations, so the job paid the setup and teardown cost of a compiler
# cache that cached nothing. That is a broken integration, not a clean
# zero-miss run, and it is reported as a warning so the condition is visible
# in the run evidence rather than silently passing as "no misses".
set -euo pipefail

sccache --show-stats | tee sccache-stats.txt
sccache --show-stats --stats-format json >sccache-stats.json

summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}
{
    printf '### sccache\n\n'
    # shellcheck disable=SC2016  # The backticks are Markdown, not a subshell.
    printf -- '- Compiler cache backend: `%s`\n\n```text\n' \
        "${SETUP_RUST_CACHE_BACKEND:-unset}"
    cat sccache-stats.txt
    printf '```\n'
} >>"${summary}"

requests=$(awk '/^Compile requests[[:space:]]/ {print $3; exit}' sccache-stats.txt)
if [[ "${requests:-0}" == "0" ]]; then
    echo "::warning::sccache recorded zero compile requests;" \
        "RUSTC_WRAPPER did not wrap this job's compiler invocations."
fi
