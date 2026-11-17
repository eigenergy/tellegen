#!/usr/bin/env bash
# Run the generated-grid hand-off browser benchmarks (#261).
#
#   validation/evidence/synth/handoff/run.sh [ahead|serial]
#
# `ahead` (the default) is the hand-off as it is now; `serial` is the hand-off
# before #261. Build first, as README.md beside this file says; it also lists
# the options and the recorded results.
set -euo pipefail
cd "$(dirname "$0")/../../../../apps/web"
TELLEGEN_SYNTH_BENCH=1 \
TELLEGEN_SYNTH_BENCH_PATH="${1:-ahead}" \
TELLEGEN_SYNTH_BENCH_RUNS="${TELLEGEN_SYNTH_BENCH_RUNS:-9}" \
  npx playwright test synth-handoff-bench -g "hand-off browser benchmark"
