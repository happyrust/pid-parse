# Byte Audit Baseline Comparison Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a small library-level comparator for two `ByteAuditReport` values so future fixture baselines can detect parser coverage regressions deterministically.

**Architecture:** Keep baseline comparison independent of CLI and filesystem concerns. A future CI script can deserialize two JSON reports and call `compare_byte_audit_reports`; this change only defines the comparison model and tests it with synthetic `ByteAuditReport` values.

**Tech Stack:** Rust 2021, `serde`, `schemars`, existing `ByteAuditReport` / `StreamAuditSummary`.

---

### Task 1: Write Comparator Tests First

**Files:**
- Create: `src/byte_audit/compare.rs`
- Modify: `src/byte_audit/mod.rs`

Tests should cover:
- overall coverage ratio decrease is a regression
- per-stream `consumed_bytes` decrease is a regression
- a stream moving from traced to unregistered is a regression
- new traced streams are reported as improvements

Expected RED: `compare_byte_audit_reports` and the comparison types do not exist.

### Task 2: Implement Minimal Comparator

**Files:**
- Create: `src/byte_audit/compare.rs`
- Modify: `src/byte_audit/mod.rs`

Public API:

```rust
pub fn compare_byte_audit_reports(
    baseline: &ByteAuditReport,
    current: &ByteAuditReport,
) -> ByteAuditComparison
```

Model:
- `ByteAuditComparison`
- `ByteAuditRegression`
- `ByteAuditRegressionKind`
- `ByteAuditImprovement`
- `ByteAuditImprovementKind`

Rules:
- `current.overall_coverage_ratio < baseline.overall_coverage_ratio` => regression
- existing stream consumed bytes decreased => regression
- existing stream had parser and now has no parser => regression
- existing stream had no parser and now has parser => improvement
- new stream with parser => improvement

### Task 3: Export and Document

**Files:**
- Modify: `src/byte_audit/mod.rs`
- Modify: `docs/byte-audit-guide.md`
- Modify: `CHANGELOG.md`

Export comparison types from `pid_parse::byte_audit`.

### Task 4: Verify

Run:

```bash
cargo test --lib byte_audit -- --nocapture
cargo fmt --all
```

Expected: byte-audit unit tests pass and formatting is clean.
