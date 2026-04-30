# Phase 11a-2 — PSMclustertable Consistency and Coverage Policy

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Upstream commit:** `d90662b feat(parser): add PSM cluster decoded candidates`
**Goal:** 把 Phase 11a 第一轮产出的 `PsmClusterTable.decoded_records` 从"可见候选字段"推进到"有 crossref consistency、coverage policy、schema/changelog 工作流"的稳定工程化状态，同时继续保持 conservative：不把证据不足字段命名为 `segment_count`，不把 `PSMclustertable` 提前标成 `FullyDecoded`。
**Estimate:** 3-5h

---

## 0. Current State

已完成：

- `PsmClusterTable.decoded_records` additive API。
- `PsmClusterRecordDecoded` / `DecodedFieldRange`。
- parser 产出候选字段：
  - `name_bytes_with_nul`
  - `candidate_ordinal`
  - `candidate_non_sheet_marker`
  - `candidate_non_sheet_payload_index`
- `pid_inspect` 文本 report 显示 decoded candidate summary。
- synthetic parser test、report test、真实 fixture guard 已通过。
- `docs/analysis/2026-05-06-psmclustertable-evidence.md` 记录三份 fixture evidence。

仍缺：

- `crossref` 里没有一致性摘要，用户需要手工对照。
- coverage / byte-audit policy 还没有明确代码化：当前应保持 `PartiallyDecoded`。
- 新 public schema 字段可能影响后续 schema snapshot gate，需在计划里明确处理。
- 中文 changelog 已有第一轮记录，但 11a-2 落地后需要追加 consistency/policy 段。

---

## 1. Preconditions

执行前确认：

```bash
git status --short
git log --oneline -5
cargo test --locked --lib parsers::psm_tables
cargo test --locked --lib inspect::report::tests::report_shows_psm_cluster
cargo test --locked --test parse_real_files psm_cluster -- --nocapture
```

预期：

- HEAD 包含 `d90662b` 或其后续 commit。
- 上述目标测试仍绿。
- 工作区可能仍有 W1 baseline/runner/docs 未提交变更；本 Phase 只能触碰 11a-2 相关文件，提交时继续拆分。

---

## 2. Non-Goals

- 不新增更多候选字段，除非 evidence 文档已证明。
- 不改 `PsmClusterRecordDecoded` 字段名。
- 不把 `PSMsegmenttable` 做语义化解码。
- 不升级 `PSMclustertable` 到 `FullyDecoded`。
- 不刷新 byte-audit baselines，除非 trace confidence 真的变化。
- 不提交 W1 baseline/runner/docs 未收口内容。

---

## 3. Target Design

### 3.1 Crossref consistency model

新增一个派生摘要，建议放在 `src/crossref.rs` 的 cluster coverage 附近：

```rust
pub struct PsmClusterDecodedConsistency {
    pub declared_count: u32,
    pub entries_len: usize,
    pub decoded_len: usize,
    pub names_match_entries: bool,
    pub record_ranges_match_entries: bool,
    pub ordinals_monotonic_for_decoded: bool,
    pub sheet_marker_matches_sheet_names: bool,
    pub payload_index_only_on_non_sheet_candidates: bool,
    pub status: ConsistencyStatus,
    pub warnings: Vec<String>,
}
```

`ConsistencyStatus` 可复用现有风格；若没有合适类型，先用 string enum：

```rust
#[serde(rename_all = "snake_case")]
pub enum ConsistencyStatus {
    Consistent,
    Warning,
    MissingDecodedRecords,
}
```

原则：

- consistency 只描述结构是否自洽，不做业务语义承诺。
- mismatch 进入 warnings，不让 parser hard-fail。
- report/crossref 输出要让用户知道为什么仍是 `PartiallyDecoded`。

### 3.2 Coverage policy

保持：

```text
PSMclustertable = PartiallyDecoded
note = names + decoded record candidates; SmartPlant field semantics pending
```

升级条件写进代码注释或测试名：

- ≥2 fixture evidence 只是允许 candidate view，不足以 FullyDecoded。
- FullyDecoded 需要：
  1. 所有 prefix bytes 都有稳定字段解释。
  2. `candidate_*` 字段重命名为业务语义字段。
  3. crossref consistency 在 DWG-0201 / DWG-0202 / 中文样本全绿。
  4. byte-audit confidence 能从 `Probed` 升到 `Decoded`。

---

## 4. Task Plan

### Task 1 — Red test: consistency summary happy path

**Files:**

- Modify: `src/crossref.rs`

**Steps:**

1. 写单元测试：

   ```rust
   #[test]
   fn psm_cluster_decoded_consistency_accepts_parallel_candidate_view() {}
   ```

2. 构造 `PidDocument`：
   - `PsmClusterTable.count = 2`
   - `entries = [PSMcluster0, Sheet6]`
   - `decoded_records` 与 `entries` 平行
   - `PSMcluster0.marker = 1 payload_index = Some(0)`
   - `Sheet6.marker = 0 payload_index = None`

3. 断言：
   - `decoded_len == entries_len`
   - `names_match_entries`
   - `record_ranges_match_entries`
   - `sheet_marker_matches_sheet_names`
   - `payload_index_only_on_non_sheet_candidates`
   - `status == Consistent`

**Expected red:** 缺函数/类型或字段。

### Task 2 — Implement consistency summary

**Files:**

- Modify: `src/crossref.rs`

**Steps:**

1. 新增 `psm_cluster_decoded_consistency(doc: &PidDocument) -> Option<PsmClusterDecodedConsistency>`。
2. 只在 `doc.psm_cluster_table.is_some()` 时返回 `Some`。
3. `decoded_records.is_empty()` 时：
   - status = `MissingDecodedRecords`
   - warnings 包含 `"decoded_records is empty"`
4. 检查规则：
   - `decoded_records.len() == entries.len()`
   - `decoded.name == entry.name`
   - `decoded.record_offset == entry.record_offset`
   - `decoded.record_len == entry.record_len`
   - decoded ordinals 在 `Some` 值范围内非递减，且首条可为 0
   - name 以 `Sheet` 开头时 marker 必须 `Some(0)`
   - marker `Some(0)` 时 payload index 必须 `None`
5. 任一失败：status = `Warning`，追加具体 warning。

**Acceptance:**

- Task 1 测试转绿。
- 无 parser 行为变化。

### Task 3 — Red tests for warning paths

**Files:**

- Modify: `src/crossref.rs`

**Tests:**

- `psm_cluster_decoded_consistency_warns_on_name_mismatch`
- `psm_cluster_decoded_consistency_warns_on_sheet_payload`
- `psm_cluster_decoded_consistency_reports_missing_decoded_records`

**Acceptance:**

- 三条测试先 red，再用 Task 2 helper 最小补齐。

### Task 4 — Wire into crossref/report output

**Files:**

- Modify: `src/inspect/report.rs` or existing crossref print path in `src/bin/pid_inspect.rs`

**Steps:**

1. 在文本 report 的 `PSMclustertable` 段末尾输出：

   ```text
   decoded consistency: consistent
   ```

   或 warning：

   ```text
   decoded consistency: warning
     - decoded record #2 name mismatch: ...
   ```

2. 输出只放摘要，避免重复每条 decoded record。
3. 若已有 `--crossref` 专门输出路径，优先接入那里；普通 report 可只输出一行。

**Acceptance:**

- 新 report 单测覆盖 consistent 与 warning 摘要。

### Task 5 — Coverage policy test

**Files:**

- Modify: `src/inspect/coverage.rs`

**Steps:**

1. 加测试：

   ```rust
   #[test]
   fn psm_cluster_table_remains_partial_with_candidate_decoded_records() {}
   ```

2. 构造含 `decoded_records` 的 doc，断言 coverage status 仍为 `PartiallyDecoded`，note 包含 `decoded record candidates` 或等价文案。
3. 如现有 coverage note 仍是旧文案，更新为 conservative 描述。

**Acceptance:**

- 明确防止未来误把 candidate view 当 FullyDecoded。

### Task 6 — Schema / changelog / docs

**Files:**

- Modify: `CHANGELOG.md`
- Maybe modify: `docs/analysis/2026-05-06-psmclustertable-evidence.md`
- Maybe modify: schema snapshots if Phase 12d already landed

**Steps:**

1. `CHANGELOG.md [Unreleased]` 在 Phase 11a 段补 11a-2 子 bullet：
   - crossref consistency
   - coverage remains partial
   - no byte-audit baseline refresh
2. evidence doc 增加一节 "Consistency policy result"。
3. 如果 `tests/schema_snapshots.rs` 已存在且失败：
   - 运行 `cargo test --locked --test schema_snapshots`
   - 若只是 additive schema field，按 snapshot workflow 刷新并说明原因。

**Acceptance:**

- 文档仍用中文描述用户可见变化。
- 不夸大 FullyDecoded。

### Task 7 — Verification and commit

Run:

```bash
cargo fmt --all -- --check
cargo test --locked --lib parsers::psm_tables
cargo test --locked --lib inspect::report::tests::report_shows_psm_cluster
cargo test --locked --lib inspect::coverage
cargo test --locked --test parse_real_files psm_cluster -- --nocapture
git diff --check
```

If time allows:

```bash
cargo test --locked --workspace --all-targets
```

Suggested commit:

```text
feat(parser): add PSM cluster decoded consistency checks
```

Commit scope must exclude W1 baseline/runner/docs leftovers unless the user explicitly asks to ship W1 too.

---

## 5. Acceptance Criteria

- [ ] `psm_cluster_decoded_consistency` summarizes decoded candidate self-consistency.
- [ ] Warning paths are covered by unit tests.
- [ ] Text report or crossref output shows consistency status.
- [ ] Coverage stays `PartiallyDecoded` with an explicit candidate-view note.
- [ ] Real fixture tests still pass.
- [ ] Changelog/docs explain that 11a-2 is consistency/policy, not semantic FullyDecoded.
- [ ] No byte-audit baseline refresh unless trace confidence changes.

---

## 6. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Consistency API becomes public too early | More schema churn | Keep helper internal unless downstream needs it |
| Coverage accidentally upgrades | Overclaims parser maturity | Add explicit coverage test that stays partial |
| W1 dirty files mix into commit | Review noise / wrong release story | Stage only 11a-2 files; use cached diff check |
| Sheet marker heuristic is wrong | False warning on new fixture | Treat as warning only, not parser failure |
| Schema snapshot fails | CI red after public field changes | Follow Phase 12d snapshot workflow if present |

---

## 7. Forward Link

After this plan lands, the next safe step is Phase 11b planning:

- Use `PsmClusterDecodedConsistency` as a preflight check.
- Do not derive `PSMsegmenttable` owner semantics from cluster records until 11a consistency is green on at least DWG-0201 / DWG-0202 / 中文样本.
