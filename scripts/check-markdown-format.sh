#!/usr/bin/env bash
# Verifies that Markdown sources are already in the canonical form `make fmt`
# produces, without modifying any tracked file.
#
# `mdtablefix` owns table padding and paragraph wrapping, while
# `markdownlint-cli2` owns Markdown lint fixes. Neither has a check-only mode,
# so this script applies both formatter stages to staged copies in one batch,
# then compares each result against the corresponding source without modifying
# tracked files.
# `mdtablefix` emits LF, whereas Git can check text out with CRLF on Windows;
# the comparison accepts only either exact line-ending form. Keep the flags in
# step with the `mdtablefix` invocation in `mdformat-all`, which `make fmt`
# runs.
#
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "Usage: $(basename "$0") <file>..." >&2
  exit 64 # EX_USAGE
fi

MDTABLEFIX="${MDTABLEFIX:-mdtablefix}"
MDLINT="${MDLINT:-markdownlint-cli2}"

if ! command -v "$MDTABLEFIX" >/dev/null 2>&1; then
  echo "$(basename "$0"): '$MDTABLEFIX' is not installed or not on PATH." >&2
  exit 127
fi

if ! command -v "$MDLINT" >/dev/null 2>&1; then
  echo "$(basename "$0"): '$MDLINT' is not installed or not on PATH." >&2
  exit 127
fi

staged_directory="$(mktemp -d)"
trap 'rm -rf "$staged_directory"' EXIT

original_files=()
staged_files=()
for file in "$@"; do
  staged_file="$staged_directory/${#original_files[@]}.md"
  cp -- "$file" "$staged_file"
  original_files+=("$file")
  staged_files+=("$staged_file")
done

"$MDTABLEFIX" --in-place --wrap --renumber --breaks --ellipsis --fences \
  "${staged_files[@]}"
"$MDLINT" --fix "${staged_files[@]}"

unformatted=()
for index in "${!original_files[@]}"; do
  original_file="${original_files[$index]}"
  staged_file="${staged_files[$index]}"
  if ! cmp -s "$staged_file" "$original_file" \
    && ! sed $'s/$/\r/' "$staged_file" | cmp -s - "$original_file"; then
    unformatted+=("$original_file")
  fi
done

if [[ ${#unformatted[@]} -gt 0 ]]; then
  echo "The following Markdown files are not formatted; run 'make fmt':" >&2
  printf '  %s\n' "${unformatted[@]}" >&2
  exit 1
fi
