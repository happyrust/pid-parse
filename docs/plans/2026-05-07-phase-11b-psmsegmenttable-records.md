# Phase 11b — PSMsegmenttable Conservative Record View

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Prerequisite:** Phase 11a-2 `PSMclustertable` consistency/policy must be green.
**Goal:** 把 `PSMsegmenttable` 从当前的 `flags + probe` 视图推进到保守 record 视图：每个 segment flag 有稳定 index / byte range / owner candidate / confidence / consistency summary。与 Phase 11a 一样，本 Phase 只命名证据支撑的结构字段，不把 `flag` 过早解释成业务 `SegmentKind`。
**Estimate:** 4-6h

---

## 0. Current State

当前代码已经有：

- `PsmSegmentTable`：
  - `size`
  - `count`
  - `flags`
  - `entries`
  - `trailing_bytes`
- `PsmSegmentEntry`：
  - `index`
  - `offset`
  - `flag`
  - `probe`
- `PsmSegmentRecordProbe`：
  - `flag_hex`
  - `neighbor_window_hex`
  - `stream_offset`
  - `owner_cluster_hint`
- `apply_segment_owner_hints`：
  - 只在 `segment_table.entries.len() == cluster_table.entries.len()` 时做 1:1 owner hint。
  - 在当前真实 fixture 中 cluster count 5/6，而 segment count 4，因此 hint 多数不会产生。

Phase 11a evidence 已证明：

- `PSMsegmenttable.count = 4` 在 DWG-0201 / DWG-0202 / 中文样本中稳定。
- `PSMclustertable.count = 5/6`，不能做简单 1:1 cluster→segment 映射。
- 11a 没有证明 `declared_segment_count` 字段，因此 11b 不能依赖它。

---

## 1. Non-Goals

- 不把 `flag == 0x01` 命名为 `Connection` / `Valid` / `Geometric`。
- 不把 segment 绑定到 cluster owner，除非有独立证据。
- 不改 Sheet endpoint / layout / object graph 的消费路径。
- 不升级 `PSMsegmenttable` 到 `FullyDecoded`。
- 不刷新 byte-audit baseline，除非 trace confidence 真正变化。

---

## 2. Target Design

### 2.1 Additive decoded record view

在 `PsmSegmentTable` 上新增 additive 字段：

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub decoded_records: Vec<PsmSegmentRecordDecoded>,
```

建议模型：

```rust
pub struct PsmSegmentRecordDecoded {
    pub index: usize,
    pub stream_offset: usize,
    pub flag: u8,
    pub flag_hex: String,
    pub confidence: String,
    pub field_ranges: Vec<DecodedFieldRange>,
    pub owner_cluster_candidate: Option<String>,
}
```

命名原则：

- `flag` 是原始字段名，可以保留；不要叫 `kind`。
- `owner_cluster_candidate` 只能来自保守规则，不能承诺业务 owner。
- `field_ranges` 复用 Phase 11a 的 `DecodedFieldRange`。

### 2.2 Consistency summary

新增内部 helper：

```rust
pub struct PsmSegmentDecodedConsistency {
    pub declared_count: u32,
    pub flags_len: usize,
    pub entries_len: usize,
    pub decoded_len: usize,
    pub decoded_matches_entries: bool,
    pub offsets_match_flag_positions: bool,
    pub owner_candidates_populated: usize,
    pub status: ConsistencyStatus,
    pub warnings: Vec<String>,
}
```

规则：

- `decoded_len == entries_len == flags_len == count` 才 consistent。
- 每条 decoded record 必须镜像 legacy entry 的 index/offset/flag。
- owner candidate 缺失不是错误，只是 evidence 状态。
- 如果 11a consistency helper 可用，则把它作为 preflight 信息，不作为 hard dependency。

---

## 3. Task Plan

### Task 1 — Red test: decoded records mirror flags

**Files:**

- Modify: `src/parsers/psm_tables.rs`

**Steps:**

1. 新增测试：

   ```rust
   #[test]
   fn segment_table_decoded_records_mirror_flag_entries() {}
   ```

2. 构造 `[magic 'stab'][count=4][01 02 03 04]`。
3. 断言：
   - `decoded_records.len() == 4`
   - decoded index/offset/flag 与 `entries` 平行
   - `flag_hex` 与 probe 一致
   - `field_ranges[0]` 指向 flag byte offset
   - `confidence == "medium"`

**Expected red:** 缺 `decoded_records` / `PsmSegmentRecordDecoded`。

### Task 2 — Add model and parser implementation

**Files:**

- Modify: `src/model.rs`
- Modify: `src/parsers/psm_tables.rs`

**Steps:**

1. 新增 `PsmSegmentRecordDecoded`。
2. 在 `PsmSegmentTable` 新增 `decoded_records`。
3. `parse_psm_segment_table_with_trace` 生成 decoded view。
4. 保留 `flags` / `entries` / `probe`，不破坏旧 API。
5. 对所有测试中的 struct literal 加 `decoded_records: vec![]`。

**Acceptance:**

- Task 1 转绿。
- `cargo test --locked --lib parsers::psm_tables` 通过。

### Task 3 — Real fixture guard

**Files:**

- Modify: `tests/parse_real_files.rs`

**Test:**

```rust
#[test]
fn psm_segment_decoded_records_match_observed_flags() {}
```

For DWG-0201 / DWG-0202:

- `count == 4`
- `flags == [0x01, 0x01, 0x01, 0x01]`
- `decoded_records.len() == entries.len() == flags.len()`
- each decoded `stream_offset == 8 + index`
- owner candidate may be `None`; do not fail on missing owner.

**Acceptance:**

- Test passes locally and soft-skips if fixtures are absent.

### Task 4 — Consistency helper

**Files:**

- Modify: `src/crossref.rs`

**Tests:**

- `psm_segment_decoded_consistency_accepts_parallel_flags`
- `psm_segment_decoded_consistency_warns_on_missing_decoded_records`
- `psm_segment_decoded_consistency_warns_on_offset_mismatch`

**Implementation:**

- Add `psm_segment_decoded_consistency(doc: &PidDocument) -> Option<PsmSegmentDecodedConsistency>`.
- Return warnings, not parser errors.
- Keep helper internal unless there is an existing public crossref DTO pattern.

### Task 5 — Report output

**Files:**

- Modify: `src/inspect/report.rs`

**Steps:**

1. In `--- PSMsegmenttable ---`, print decoded summary per first 20 rows:

   ```text
   decoded: confidence=medium flag=0x01 owner_candidate=-
   ```

2. Keep existing probe output for first 3 rows.
3. Print consistency summary if Task 4 helper is wired into report.

**Acceptance:**

- Report test covers decoded segment summary.

### Task 6 — Coverage policy

**Files:**

- Modify: `src/inspect/coverage.rs`

**Rule:**

`PSMsegmenttable` remains `PartiallyDecoded`:

```text
flags + decoded record candidates; flag semantics pending
```

Add regression test to prevent accidental `FullyDecoded` claim.

### Task 7 — Docs / changelog

**Files:**

- Modify: `CHANGELOG.md`
- Maybe modify: `docs/analysis/2026-05-06-psmclustertable-evidence.md` or create new segment evidence note

**Steps:**

1. Add Chinese Unreleased entry:
   - additive `PsmSegmentTable.decoded_records`
   - no semantic `SegmentKind`
   - no coverage upgrade
   - real fixture guard
2. If creating evidence note:
   - `docs/analysis/2026-05-07-psmsegmenttable-evidence.md`
   - record current 3 fixture facts: `count=4`, flags all `0x01`, no owner proof.

### Task 8 — Verification

Run:

```bash
cargo fmt --all -- --check
cargo test --locked --lib parsers::psm_tables
cargo test --locked --lib inspect::report::tests::report_shows_psm_segment
cargo test --locked --test parse_real_files psm_segment -- --nocapture
cargo test --locked --lib inspect::coverage
git diff --check
```

If Phase 12d schema snapshots are present:

```bash
cargo test --locked --test schema_snapshots
```

Commit suggestion:

```text
feat(parser): add PSM segment decoded candidates
```

---

## 4. Acceptance Criteria

- [ ] `PsmSegmentTable.decoded_records` mirrors legacy flags/entries.
- [ ] No semantic `SegmentKind` is introduced.
- [ ] Real fixture tests lock count/flags/offsets.
- [ ] Crossref consistency summarizes decoded view health.
- [ ] Report output shows decoded segment candidates.
- [ ] Coverage remains `PartiallyDecoded`.
- [ ] Changelog/docs explicitly say owner/kind semantics are still pending.

---

## 5. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| All sampled flags are `0x01` | Cannot infer kind | Keep raw `flag`; no `SegmentKind` |
| Cluster count != segment count | No 1:1 owner mapping | owner candidate optional; missing owner is not warning |
| Public schema churn | Snapshot failure | Refresh only under Phase 12d workflow |
| W1 dirty files still present | Commit contamination | Stage only 11b files and changelog hunk |
| Report noise | CLI clutter | Show first 20 decoded rows, probe only first 3 |

---

## 6. Forward Link

After Phase 11b:

- Phase 11d can inspect `PSMspacemap` / `tseg` against segment offsets.
- Phase 11c Sheet work can use segment decoded records as raw index anchors, but must not treat `flag` as kind until more fixtures prove it.
