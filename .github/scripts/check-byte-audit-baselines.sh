#!/usr/bin/env bash
# Optional byte-audit baseline runner.
#
# Baseline JSON files are commit-safe, but real `.pid` fixtures often live
# under gitignored `test-file/`. This script therefore runs comparisons when
# both sides exist and skips gracefully in public CI when private fixtures are
# absent.

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
