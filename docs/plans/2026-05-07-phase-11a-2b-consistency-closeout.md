# Phase 11a-2b — PSMclustertable Consistency Closeout

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Upstream commit:** `409965e feat(parser): add PSM cluster consistency checks`
**Goal:** 收口 Phase 11a-2 剩余工作：补齐 warning-path tests、把 consistency summary 接入文本 report / crossref 可见面、用 coverage policy test 明确 `PSMclustertable` 仍保持 `PartiallyDecoded`，最后更新中文 changelog 并提交。
**Estimate:** 2-3h

---

## 0. Current State

已完成：

- `PsmClusterDecodedConsistencyStatus`
- `PsmClusterDecodedConsistency`
- `psm_cluster_decoded_consistency(doc)`
- happy-path 单测：
  - `psm_cluster_decoded_consistency_accepts_parallel_candidate_view`
- 11a-2 与 11b 计划文件已提交并推送。

仍缺：

- warning paths 测试。
- report/crossref 输出。
- coverage policy test。
- 11a-2b changelog。

---

## 1. Preconditions

```bash
git log --oneline -3
git status --short
cargo test --locked --lib crossref::tests::psm_cluster_decoded_consistency_accepts_parallel_candidate_view
cargo fmt --all -- --check
```

Expected:

- `409965e` 已在 HEAD 或祖先提交里。
- 仍可能存在 W1 未提交改动；本 plan 继续只暂存 11a-2b 相关文件。

---

## 2. Task Plan

### Task 1 — Warning path tests

**Files:**

- Modify: `src/crossref.rs`

Add three tests:

```rust
#[test]
fn psm_cluster_decoded_consistency_warns_on_name_mismatch() {}

#[test]
fn psm_cluster_decoded_consistency_warns_on_sheet_payload() {}

#[test]
fn psm_cluster_decoded_consistency_reports_missing_decoded_records() {}
```

Assertions:

- name mismatch → `status == Warning`, warnings mention names.
- `Sheet*` row with `candidate_non_sheet_payload_index = Some(_)` → `status == Warning`, warnings mention payload.
- empty decoded records → `status == MissingDecodedRecords`, warnings mention `decoded_records is empty`.

Acceptance:

- Tests fail first if helper misses warning detail, then pass with minimal code changes.

### Task 2 — Report visibility

**Files:**

- Modify: `src/inspect/report.rs`

Add output near the `PSMclustertable` section footer:

```text
  decoded consistency: consistent
```

For warnings:

```text
  decoded consistency: warning
    - decoded record names do not mirror legacy entries
```

Tests:

```rust
#[test]
fn report_shows_psm_cluster_decoded_consistency_summary() {}
```

Acceptance:

- Normal report shows one summary line.
- Warnings are visible but do not spam per-record dumps.

### Task 3 — Coverage policy test

**Files:**

- Modify: `src/inspect/coverage.rs`

Add test:

```rust
#[test]
fn psm_cluster_table_remains_partial_with_candidate_decoded_records() {}
```

Expected:

- `PSMclustertable` remains `ParseCoverageStatus::PartiallyDecoded`.
- note contains "decoded record candidates" or equivalent conservative phrase.

If current coverage note is too old, update only the note text, not the status.

### Task 4 — Docs and changelog

**Files:**

- Modify: `CHANGELOG.md`
- Maybe modify: `docs/plans/2026-05-06-phase-11a-2-psmclustertable-consistency.md`

Changelog bullet:

- warning path coverage
- report consistency summary
- coverage remains `PartiallyDecoded`

Do not claim:

- `FullyDecoded`
- semantic `cluster_id`
- `declared_segment_count`

### Task 5 — Verification

Run:

```bash
cargo fmt --all -- --check
cargo test --locked --lib crossref::tests::psm_cluster_decoded_consistency
cargo test --locked --lib inspect::report::tests::report_shows_psm_cluster
cargo test --locked --lib inspect::coverage
cargo test --locked --test parse_real_files psm_cluster -- --nocapture
git diff --check
```

If time allows:

```bash
cargo test --locked --workspace --all-targets
```

Commit suggestion:

```text
feat(parser): surface PSM cluster consistency warnings
```

---

## 3. Acceptance Criteria

- [ ] Consistency warning paths covered.
- [ ] Report exposes consistency status.
- [ ] Coverage status remains `PartiallyDecoded` by test.
- [ ] Changelog states this is conservative engineering guardrail work.
- [ ] Staged diff excludes W1 baseline/runner/docs leftovers.

---

## 4. Forward Link

After 11a-2b lands, Phase 11b can begin with `docs/plans/2026-05-07-phase-11b-psmsegmenttable-records.md`.
