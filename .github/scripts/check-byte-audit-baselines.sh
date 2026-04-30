#!/usr/bin/env bash
# Optional byte-audit baseline runner.
#
# Baseline JSON files (`docs/baselines/*.byte-audit.json`) are commit-safe,
# and real `.pid` fixtures now live under in-repo `test-file/` as well.
# This script runs comparisons when both sides exist and still skips
# gracefully when a baseline has no matching fixture (e.g. partial
# checkouts or extra baselines for not-yet-committed samples).
#
# Fixture path resolution (Phase 12c, 2026-04-29):
#   1. If `docs/baselines/<slug>.fixture.txt` exists, the first non-empty
#      trimmed line is treated as the fixture path (relative to the repo
#      root). This sidecar lets baseline filenames stay ASCII while the
#      underlying fixture path can contain non-ASCII characters (e.g.
#      Chinese filenames), avoiding cross-platform / CI shell escaping
#      issues.
#   2. Otherwise the legacy convention `test-file/<slug>.pid` is used.

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
  sidecar="docs/baselines/${name}.fixture.txt"

  if [[ -f "${sidecar}" ]]; then
    fixture=$(head -n 1 "${sidecar}" | tr -d '\r' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
    if [[ -z "${fixture}" ]]; then
      echo "byte-audit baselines: skip '${baseline}' (sidecar '${sidecar}' is empty)."
      skipped=$((skipped + 1))
      continue
    fi
  else
    fixture="test-file/${name}.pid"
  fi

  if [[ ! -f "${fixture}" ]]; then
    echo "byte-audit baselines: skip '${baseline}' (missing fixture '${fixture}')."
    skipped=$((skipped + 1))
    continue
  fi

  echo "byte-audit baselines: comparing '${fixture}' against '${baseline}'..."
  cargo run --locked --bin pid_inspect -- "${fixture}" --byte-audit --byte-audit-baseline "${baseline}"
  ran=$((ran + 1))
done

echo "byte-audit baselines: completed comparisons=${ran}, skipped=${skipped}."
