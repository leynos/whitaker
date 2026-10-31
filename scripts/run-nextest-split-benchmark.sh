#!/usr/bin/env bash
# Run one half of the issue-513 benchmark on a quiet CI runner.
#
# Usage: run-nextest-split-benchmark.sh filtered|full REPETITIONS OUTPUT_DIR
#
# `filtered` runs both capacity-one groups in one Nextest invocation, five
# times per configuration, alternating the serial control with the delivered
# split. `full` measures warm `make test` and `make test` after the same
# one-line lint-crate edit for both configurations, then five clean sequential
# post-split `make test` runs.
#
# Every measurement keeps its own configuration copy, log, JUnit report, meta
# record, and summary. A failed repetition is recorded and the run continues;
# the script's exit status counts the failures so the job is still red.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
readonly REPO_ROOT
readonly MODE="${1:?usage: $0 filtered|full REPETITIONS OUTPUT_DIR}"
readonly REPETITIONS="${2:-5}"
readonly OUT="${3:?usage: $0 filtered|full REPETITIONS OUTPUT_DIR}"
readonly PROFILE=default
readonly CONFIG="$REPO_ROOT/.config/nextest.toml"
readonly EDITED_SOURCE="$REPO_ROOT/crates/bumpy_road_function/src/lib.rs"
readonly BENCHMARK="$SCRIPT_DIR/nextest_split_benchmark.py"

failures=0

benchmark() {
  python3 "$BENCHMARK" "$@"
}

write_meta() {
  local path=$1 name=$2 status=$3 started=$4 ended=$5
  python3 - "$path" "$name" "$status" "$started" "$ended" <<'PY'
import json, pathlib, sys

path, name, status, started, ended = sys.argv[1:]
pathlib.Path(path).write_text(
    json.dumps(
        {
            "name": name,
            "exit_status": int(status),
            "started_unix": float(started),
            "ended_unix": float(ended),
            "wall_seconds": round(float(ended) - float(started), 3),
        },
        indent=2,
        sort_keys=True,
    )
    + "\n"
)
PY
}

record_host() {
  {
    uname -a
    printf 'cpus: %s\n' "$(nproc)"
    free -m
    df -m /
    rustc -Vv
    cargo -V
    cargo nextest --version
    git -C "$REPO_ROOT" rev-parse HEAD
    git -C "$REPO_ROOT" status --porcelain
  } >"$OUT/host.txt" 2>&1 || true
  if command -v sccache >/dev/null; then
    sccache --show-stats >"$OUT/sccache-before.txt" 2>&1 || true
  fi
}

# prepare_config MODE LABEL writes a configuration copy and prints its path.
prepare_config() {
  local mode=$1 label=$2
  local args=(
    prepare
    --config "$CONFIG"
    --output "$OUT/configs/$label.toml"
    --junit "$OUT/junit/$label.xml"
    --profile "$PROFILE"
  )
  if [ "$mode" = control ]; then
    args+=(--serial-control)
  fi
  benchmark "${args[@]}"
  printf '%s\n' "$OUT/configs/$label.toml"
}

# run_measured NAME COMMAND... records wall time, log and meta for one run.
run_measured() {
  local name=$1
  shift
  local started ended status
  mkdir -p "$OUT/logs" "$OUT/meta" "$OUT/summaries"
  printf '%s\n' "$*" >>"$OUT/commands.txt"
  started=$(date +%s.%N)
  set +e
  "$@" 2>&1 | tee "$OUT/logs/$name.log"
  status=${PIPESTATUS[0]}
  set -e
  ended=$(date +%s.%N)
  write_meta "$OUT/meta/$name.json" "$name" "$status" "$started" "$ended"
  if [ "$status" -ne 0 ]; then
    failures=$((failures + 1))
  fi
  return 0
}

summarize_run() {
  local name=$1
  benchmark summarize \
    --junit "$OUT/junit/$name.xml" \
    --membership "$OUT/membership.json" \
    --log "$OUT/logs/$name.log" \
    --meta "$OUT/meta/$name.json" \
    --output "$OUT/summaries/$name.json" || true
}

# nextest_run MODE LABEL [EXTRA...] runs `make test` with a generated config.
nextest_run() {
  local mode=$1 label=$2
  shift 2
  local config
  config=$(prepare_config "$mode" "$label")
  run_measured "$label" make -C "$REPO_ROOT" test \
    TEST_RUNNER="nextest run --config-file $config $*"
  if [ -f "$OUT/membership.json" ]; then
    summarize_run "$label"
  fi
}

write_membership() {
  local union trybuild ui
  union=$(benchmark union-filter --config "$CONFIG" --profile "$PROFILE")
  trybuild=$(benchmark union-filter --config "$CONFIG" --profile "$PROFILE" \
    --group serial-trybuild-contracts)
  ui=$(benchmark union-filter --config "$CONFIG" --profile "$PROFILE" \
    --group serial-dylint-ui)

  local split_config control_config
  split_config=$(prepare_config split membership)
  control_config=$(prepare_config control membership)

  run_measured membership-trybuild make -C "$REPO_ROOT" test \
    TEST_RUNNER="nextest list --config-file $split_config -E '$trybuild' --message-format json"
  run_measured membership-ui make -C "$REPO_ROOT" test \
    TEST_RUNNER="nextest list --config-file $split_config -E '$ui' --message-format json"
  run_measured membership-control make -C "$REPO_ROOT" test \
    TEST_RUNNER="nextest list --config-file $control_config -E '$union' --message-format json"

  # The reader accepts the JSON line inside the log, which `make test` follows
  # with the dylint_testing library tests.
  # Seven compile contracts must be under measurement: six standard ones and
  # the nested-rstest-helper contract.
  benchmark membership \
    --group "serial-trybuild-contracts=$OUT/logs/membership-trybuild.log" \
    --group "serial-dylint-ui=$OUT/logs/membership-ui.log" \
    --control "$OUT/logs/membership-control.log" \
    --expect-count serial-trybuild-contracts=7 \
    --output "$OUT/membership.json"
}

warm_up_filtered() {
  local union
  union=$(benchmark union-filter --config "$CONFIG" --profile "$PROFILE")
  nextest_run split warmup-split -E "'$union'" --no-fail-fast
  nextest_run control warmup-control -E "'$union'" --no-fail-fast
}

run_filtered() {
  local union rep
  union=$(benchmark union-filter --config "$CONFIG" --profile "$PROFILE")
  mkdir -p "$OUT/logs" "$OUT/meta" "$OUT/summaries"
  write_membership
  warm_up_filtered
  for rep in $(seq 1 "$REPETITIONS"); do
    nextest_run control "control-$rep" -E "'$union'" --no-fail-fast
    nextest_run split "split-$rep" -E "'$union'" --no-fail-fast
  done
}

# edit_lint_crate appends one comment line so Cargo rebuilds that lint crate.
edit_lint_crate() {
  printf '\n// benchmark rebuild marker %s\n' "$(date +%s)" >>"$EDITED_SOURCE"
}

restore_lint_crate() {
  git -C "$REPO_ROOT" checkout -- "$EDITED_SOURCE"
}

run_full() {
  trap restore_lint_crate EXIT
  write_membership
  nextest_run split warmup-split
  nextest_run split warm-split
  nextest_run control warm-control

  # Each edit appends a distinct marker line, so the lint crate really
  # rebuilds. The two configurations are measured in both orders: whichever
  # runs second inherits a warmer cache, and one order alone would bake that
  # bias into the comparison.
  edit_lint_crate
  nextest_run split edited-split
  restore_lint_crate

  edit_lint_crate
  nextest_run control edited-control
  restore_lint_crate

  edit_lint_crate
  nextest_run control edited-control-repeat
  restore_lint_crate

  edit_lint_crate
  nextest_run split edited-split-repeat
  restore_lint_crate

  local rep
  for rep in $(seq 1 "$REPETITIONS"); do
    nextest_run split "clean-$rep"
  done
}

mkdir -p "$OUT/configs" "$OUT/junit" "$OUT/logs" "$OUT/meta" "$OUT/summaries"
: >"$OUT/commands.txt"
record_host

case "$MODE" in
  filtered) run_filtered ;;
  full) run_full ;;
  *)
    printf 'unknown mode %s\n' "$MODE" >&2
    exit 2
    ;;
esac

if command -v sccache >/dev/null; then
  sccache --show-stats >"$OUT/sccache-after.txt" 2>&1 || true
fi
printf 'failed repetitions: %s\n' "$failures"
exit "$failures"