#!/usr/bin/env bash
# Ratchet gate for `rustdoc -W missing-docs` warnings.
#
# Runs `cargo rustdoc --lib --locked -- -W missing-docs` and counts the
# number of `missing documentation for …` diagnostics. The count is
# compared against the integer stored in
# `.github/missing-docs-baseline.txt`:
#
#   current >  baseline → fail (someone added an undocumented pub item)
#   current == baseline → pass (no net change)
#   current <  baseline → fail, but with an instruction to lower the
#                         baseline file (ratchet-down is deliberate so
#                         the progress stays visible in git history)
#
# Locally: `.github/scripts/check-missing-docs.sh`.
# CI: wired into `.github/workflows/ci.yml` after the clippy step.

set -euo pipefail

BASELINE_FILE=".github/missing-docs-baseline.txt"

if [[ ! -f "${BASELINE_FILE}" ]]; then
  echo "error: baseline file '${BASELINE_FILE}' not found" >&2
  exit 2
fi

baseline=$(tr -d '[:space:]' < "${BASELINE_FILE}")
if ! [[ "${baseline}" =~ ^[0-9]+$ ]]; then
  echo "error: baseline file '${BASELINE_FILE}' must contain a single integer; got '${baseline}'" >&2
  exit 2
fi

echo "missing_docs ratchet: running cargo rustdoc --lib --locked -- -W missing-docs ..."
# We tee the output so CI logs still show where the docs are missing if
# the check fails; the count is derived from the full stream.
rustdoc_output=$(cargo rustdoc --lib --locked -- -W missing-docs 2>&1 || true)
current=$(printf '%s\n' "${rustdoc_output}" | grep -c 'missing documentation for' || true)

echo "missing_docs ratchet: current=${current}, baseline=${baseline}"

if (( current > baseline )); then
  echo ""
  echo "FAIL: missing_docs warnings increased from ${baseline} to ${current} (+$((current - baseline)))."
  echo ""
  echo "Document the new \`pub\` items you added, or — if the change is"
  echo "intentional — explicitly bump the baseline in"
  echo "'${BASELINE_FILE}' as part of this PR so the regression is"
  echo "visible in git history."
  echo ""
  echo "New / relevant warnings (first 40):"
  printf '%s\n' "${rustdoc_output}" \
    | grep -E 'warning: missing documentation for|--> ' \
    | head -n 40
  exit 1
fi

if (( current < baseline )); then
  echo ""
  echo "FAIL: missing_docs warnings went down from ${baseline} to ${current} (-$((baseline - current)))."
  echo ""
  echo "Lovely — someone documented more public items! Please also update"
  echo "'${BASELINE_FILE}' to ${current} so the ratchet locks in the"
  echo "improvement."
  exit 1
fi

echo "missing_docs ratchet: OK (count matches baseline)."
