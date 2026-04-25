# Byte Audit Baseline Runner Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an optional baseline runner that compares committed `docs/baselines/*.byte-audit.json` files when matching private fixtures are present, while safely skipping in public CI where those fixtures are absent.

**Architecture:** Keep the runner as a small Bash script under `.github/scripts/`, matching the existing CI helper style. The script scans committed baseline JSON files, derives `test-file/<name>.pid`, invokes `pid_inspect --byte-audit --byte-audit-baseline`, and propagates non-zero comparison exits. If no baselines exist, or if a baseline's private fixture is missing, the script logs a skip rather than failing public CI.

**Tech Stack:** Bash, existing `pid_inspect` CLI, existing GitHub Actions workflow.

---

### Task 1: Confirm Missing Runner

Run:

```bash
bash .github/scripts/check-byte-audit-baselines.sh
```

Expected RED: shell reports the script does not exist.

### Task 2: Add Runner Script

**Files:**
- Create: `.github/scripts/check-byte-audit-baselines.sh`

Behavior:
- `set -euo pipefail`.
- Collect `docs/baselines/*.byte-audit.json`.
- If none exist, print a clear skip message and exit 0.
- For each baseline, derive `test-file/<basename>.pid`.
- If the fixture is missing, print a skip message and continue.
- If fixture exists, run:
  `cargo run --locked --bin pid_inspect -- "$fixture" --byte-audit --byte-audit-baseline "$baseline"`.
- Propagate regression / parse failures from `pid_inspect`.

### Task 3: Wire CI

**Files:**
- Modify: `.github/workflows/ci.yml`

Add a step after the missing-docs ratchet:

```yaml
- name: byte-audit baselines (optional private fixtures)
  run: bash .github/scripts/check-byte-audit-baselines.sh
```

### Task 4: Update Docs

**Files:**
- Modify: `docs/byte-audit-guide.md`
- Modify: `CHANGELOG.md`

Document the runner, skip behavior, and fixture naming convention.

### Task 5: Verify

Run:

```bash
bash .github/scripts/check-byte-audit-baselines.sh
```

Expected in this checkout: skip because no `docs/baselines/*.byte-audit.json` files exist.
