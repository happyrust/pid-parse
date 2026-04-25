# Byte Audit Docs and Baseline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Document the new `pid_inspect --byte-audit` workflow and define the baseline strategy to use once real `.pid` fixtures are available.

**Architecture:** No parser behavior changes. Add user-facing docs that explain text vs JSON output, how traced and unregistered streams should be interpreted, and how future fixture-backed baselines should compare `overall_coverage_ratio` and per-stream `consumed_bytes`.

**Tech Stack:** Markdown docs, existing `pid_inspect` CLI, existing `ByteAuditReport` JSON schema.

---

### Task 1: Add Byte Audit Guide

**Files:**
- Create: `docs/byte-audit-guide.md`

Cover:
- When to use `--byte-audit`
- Text output fields
- JSON output fields
- How to interpret `unregistered_paths`
- Recommended baseline comparison rules
- Current limitation: fixture-backed baseline is blocked when `test-file/*.pid` is absent

### Task 2: Update README Usage

**Files:**
- Modify: `README.md`

Add commands:

```bash
cargo run --bin pid_inspect -- drawing.pid --byte-audit
cargo run --bin pid_inspect -- drawing.pid --byte-audit --json > audit.json
```

Keep the README concise and link to `docs/byte-audit-guide.md`.

### Task 3: Verify

Run:

```bash
cargo test --test inspect_cli -- --nocapture
cargo fmt --all
```

Expected: CLI tests remain green; docs are static Markdown and require no generated artifacts.
