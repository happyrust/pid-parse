#!/usr/bin/env bash
# Optional byte-audit baseline runner.
#
# Baseline JSON files (`docs/baselines/*.byte-audit.json`) are commit-safe,
# and real `.pid` fixtures now live under in-repo `test-file/` as well.
# This script runs comparisons when both sides exist and still skips
# gracefully when a baseline has no matching fixture (e.g. partial
# checkouts or extra baselines for not-yet-committed samples).

set -euo pipefail

shopt -s nullglob
baselines=(docs/baselines/*.byte-audit.json)

if (( ${#baselines[@]} == 0 )); then
  echo "byte-audit baselines: no docs/baselines/*.byte-audit.json files found; skipping."
  exit 0
fi

ran=0
skipped=0

for baseline in "${baselines[@]}"; do
  name=$(basename "${baseline}" .byte-audit.json)
  fixture="test-file/${name}.pid"

  if [[ ! -f "${fixture}" ]]; then
    echo "byte-audit baselines: skip '${baseline}' (missing private fixture '${fixture}')."
    skipped=$((skipped + 1))
    continue
  fi

  echo "byte-audit baselines: comparing '${fixture}' against '${baseline}'..."
  cargo run --locked --bin pid_inspect -- "${fixture}" --byte-audit --byte-audit-baseline "${baseline}"
  ran=$((ran + 1))
done

echo "byte-audit baselines: completed comparisons=${ran}, skipped=${skipped}."
