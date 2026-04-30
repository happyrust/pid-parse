# Phase 11a — PSMclustertable Record Field Naming

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Sprint window:** W2, starting after W1 closes Phase 12c / 12d.
**Goal:** 把现有 `PSMclustertable` probe 层推进到可命名的 per-record decoded 层：为每条 cluster record 建立稳定字段、source byte range、confidence 与 cross-check，给 Phase 11b `PSMsegmenttable` 和后续 Sheet / normalized graph 提供可靠外键。
**Estimate:** 6-8h，必要时拆成 2 个 PR（fixture evidence / parser+tests）。

---

## 0. Current State

`PSMclustertable` 当前已经不是空白起点：

- `src/model.rs` 已有 `PsmClusterTable` / `PsmClusterEntry` / `PsmClusterRecordProbe`。
- `src/parsers/psm_tables.rs` 已能扫描 UTF-16LE cluster 名，记录 `record_offset` / `record_len` / `prefix_bytes`，并生成 probe。
- `src/inspect/report.rs` 已在 report 中展示前 3 条 probe 样本。
- `PSMsegmenttable` 也已有 probe 和 conservative `owner_cluster_hint`，但还不能作为语义外键。

本 Phase 不再做"增加 probe"。本 Phase 只做：用至少 2 份真实 fixture 对 probe 字节进行对照，确认哪些 prefix 字段可以命名，并把确认后的字段纳入稳定模型。

---

## 1. Preconditions

进入 Task 1 前必须满足：

```bash
git status
cargo test --locked --workspace --all-targets
cargo test --locked --test schema_snapshots
bash .github/scripts/check-byte-audit-baselines.sh
```

预期：

- W1 的 Phase 12c baseline 与 Phase 12d schema snapshot 已落地并提交。
- `docs/baselines/` 至少包含 DWG-0201 / DWG-0202 / sample-cn-1 三份真实 fixture baseline。
- `test-file/` 本地能访问至少 2 份真实 `.pid` fixture；如果只有 1 份，本 Phase 只能完成 evidence table，不能 claim `FullyDecoded`。
- 工作区干净，避免把 W1 未收口的 docs / baseline 改动混进本 Phase。

如果以上不满足，先回到 `docs/plans/2026-04-30-w1-unblock-and-ship-sprint.md` 完成收口。

---

## 2. Non-Goals

- 不修改 writer / round-trip 行为；cluster stream 仍按现有 raw/pass-through 策略。
- 不把 `PSMsegmenttable` 的 flag 语义命名；那是 Phase 11b。
- 不在 `layout` / `import_view` 中消费新字段；本 Phase 只提供 model + inspect + crossref evidence。
- 不引入新依赖，不引入 snapshot 框架。
- 不为了让 coverage 好看而越过 fixture gate：少于 2 份 fixture 时只允许 `confidence=medium`，不得标 `FullyDecoded`。

---

## 3. Target Design

### 3.1 新增稳定字段模型

在保留 `PsmClusterEntry` 旧字段的前提下新增 decoded 视图：

```rust
pub struct PsmClusterRecordDecoded {
    pub index: usize,
    pub name: String,
    pub record_offset: usize,
    pub record_len: usize,
    pub prefix_len: usize,
    pub candidate_id: Option<u32>,
    pub candidate_kind_tag: Option<u16>,
    pub candidate_flags: Option<u16>,
    pub candidate_segment_count: Option<u32>,
    pub inferred_kind: Option<ClusterKind>,
    pub confidence: DecodeConfidence,
    pub field_ranges: Vec<DecodedFieldRange>,
    pub unknown_prefix_bytes: Vec<u8>,
}
```

命名规则：

- `candidate_*` 只在跨 fixture 证据足够时落库；如果某字段仍是推测，保持 probe 字段，不进 decoded。
- `field_ranges` 必须指回原始 `PSMclustertable` stream offset，用于 byte-audit / inspect 解释。
- `unknown_prefix_bytes` 保留剩余 prefix，避免误称 fully decoded。

实际命名可按代码现有类型调整；核心约束是旧 `entries` 兼容保留，新 decoded 视图 additive。

### 3.2 Confidence policy

| 条件 | confidence | coverage status |
|---|---|---|
| 只有 1 份 fixture 能解释 | `medium` | 保持 `PartiallyDecoded` |
| ≥2 份 fixture 字段位置与值域稳定，且 prefix 无未知高熵字段 | `high` | 可升 `FullyDecoded` |
| 字段位置稳定但语义未确认 | 不进 decoded，留在 probe | 保持 `PartiallyDecoded` |
| 任一 fixture 出现不一致 | `low` 或不生成 decoded | 保持 `PartiallyDecoded` |

---

## 4. Task Plan

### Task 1 — Fixture Evidence Matrix

**Files:**

- Create: `docs/analysis/2026-05-06-psmclustertable-evidence.md`
- Read/verify: `test-file/*.pid`

**Steps:**

1. 对至少 2 份真实 fixture 运行 inspect / byte-audit：

   ```bash
   cargo run --locked --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --byte-audit --json > target/tmp/dwg-0201.audit.json
   cargo run --locked --bin pid_inspect -- test-file/DWG-0202GP06-01.pid --byte-audit --json > target/tmp/dwg-0202.audit.json
   ```

2. 记录每份 fixture 的 `PSMclustertable`：
   - stream size
   - declared `count`
   - `entries.len()`
   - 每条 entry 的 `name` / `record_offset` / `record_len`
   - `prefix_bytes` hex
   - probe 的 `first_u32_le` / `last_u32_le` / `trailer_hex`

3. 写 evidence table，按 record index 横向对齐各 fixture：
   - 哪些字节位置随 index 单调递增
   - 哪些字段与 cluster name / kind 相关
   - 哪些字段与 `PSMsegmenttable.entries.len()` 或 owner hint 相关
   - 哪些字段跨 fixture 不稳定

**Acceptance:**

- evidence 文档列出至少 2 份 fixture；不足 2 份时明确写 `BLOCKED: single-fixture only`。
- 每个拟命名字段都有"offset + byte width + endian + 验证公式"。

### Task 2 — Red Tests for Decoded Records

**Files:**

- Modify: `src/parsers/psm_tables.rs`
- Modify: `tests/parse_real_files.rs`

**Steps:**

1. 在 parser 单元测试里加 synthetic records 测试：
   - 正常 record：decoded 字段与 prefix bytes 一致。
   - short prefix：不 panic，缺字段为 `None`。
   - unknown kind tag：`inferred_kind = None`，保留 raw。
   - prefix 有未解释尾巴：写入 `unknown_prefix_bytes`。

2. 在 real fixture 测试里加结构性断言：
   - `decoded_records.len() == entries.len()`
   - `decoded_records[i].name == entries[i].name`
   - `record_offset` / `record_len` 与旧 entry 完全一致
   - 每个 decoded field range 落在当前 record 内

3. 如果 evidence 已支持强字段命名，再加具体字段断言；否则只加结构性红测。

**Acceptance:**

- 新测试先 fail，失败点是缺 `decoded_records` / 字段未实现，而不是 fixture 缺失或环境错误。

### Task 3 — Add Model Types

**Files:**

- Modify: `src/model.rs`

**Steps:**

1. 增加 decoded record 类型与 field range 类型。
2. 在 `PsmClusterTable` 上新增：

   ```rust
   #[serde(default, skip_serializing_if = "Vec::is_empty")]
   pub decoded_records: Vec<PsmClusterRecordDecoded>,
   ```

3. 所有新 public 类型与字段补 `///`，保持 missing-docs gate 不回退。
4. 保留 `entries` / `probe` 原样，旧 JSON consumer 不 break。

**Acceptance:**

- `cargo build --locked --workspace --all-targets` 通过。
- `cargo rustdoc --lib --locked -- -W missing-docs` warning count 不上升。

### Task 4 — Implement Conservative Decoding

**Files:**

- Modify: `src/parsers/psm_tables.rs`

**Steps:**

1. 新增内部 helper：

   ```rust
   fn decode_cluster_record(entry: &PsmClusterEntry) -> PsmClusterRecordDecoded
   ```

2. 只解析 evidence 已确认的 prefix slice；未确认字节必须保留。
3. 用 `get()` / `checked_add()` 读取字节，避免直接切片 panic。
4. 在 `parse_psm_cluster_table_with_trace` 中同步生成 `decoded_records`。
5. 如果 decoded 字段覆盖 prefix 的一部分，trace confidence 可保持 `Probed`，不要提前升级到 `Decoded`，直到 Task 7 决定 coverage policy。

**Acceptance:**

- Task 2 parser 单测通过。
- 不改变现有 `entries` 内容。
- fuzz/smoke 风格 parser panic-safety 测试继续通过。

### Task 5 — Cross-Reference Checks

**Files:**

- Modify: `src/crossref.rs` or nearest existing crossref/report module
- Modify tests near existing crossref coverage tests

**Steps:**

1. 增加 cluster record consistency check：
   - decoded record count vs declared count
   - decoded name vs `entries` name
   - inferred kind vs existing cluster path/kind if available
   - candidate segment count sum vs `PSMsegmenttable.entries.len()` if evidence supports this field

2. 输出 mismatch 为 warning/evidence，不让 parser hard-fail。
3. 单测 consistent / inconsistent 两条路径。

**Acceptance:**

- `--crossref` 或 report 能看到 cluster record consistency 摘要。
- 不一致时不会丢数据；只降低 confidence 或输出 warning。

### Task 6 — Inspect / JSON Output

**Files:**

- Modify: `src/inspect/report.rs`
- Modify CLI/report tests

**Steps:**

1. 在 `--- PSMclustertable ---` 段中为前 20 条 record 输出 decoded 摘要。
2. 前 3 条保留 probe 行，方便和 decoded 字段对照。
3. 输出示例：

   ```text
   [0] @+0008 len=42 name=PSMcluster0 decoded(confidence=medium)
       id?=0x00000001 kind_tag?=0x0002 flags?=0x0000 unknown_prefix=[...]
       probe: first_u32_le=... trailer=[...]
   ```

4. 如果字段还未达到命名标准，显示为 `candidate_*` 或省略，不输出确定性业务名。

**Acceptance:**

- report 测试断言包含 `decoded(confidence=` 与 `unknown_prefix` / `probe` 关键字。
- 输出对长 fixture 有截断，不刷屏。

### Task 7 — Coverage / Byte-Audit Policy

**Files:**

- Modify: `src/inspect/coverage.rs`
- Modify byte-audit trace tests if confidence upgraded

**Steps:**

1. 根据 Task 1 evidence 决定是否升级 coverage：
   - 若 ≥2 fixture 且 prefix 字段全覆盖：`PSMclustertable` 可从 `PartiallyDecoded` 升 `FullyDecoded`。
   - 若仍有 unknown prefix：保持 `PartiallyDecoded`，note 改成 `names + decoded record candidates; prefix residual remains`。

2. byte-audit trace 中已确认字段可标 `Decoded`；未确认字段继续 `Probed`。
3. 更新 byte-audit baselines（仅当 consumed/decoded confidence 发生预期变化）。

**Acceptance:**

- coverage 文本与 byte-audit JSON 不矛盾。
- baseline 更新只包含本 Phase 引起的预期 delta。

### Task 8 — Docs / Changelog

**Files:**

- Modify: `CHANGELOG.md`
- Modify: `docs/sppid/v0.10.x-status.md` or current status doc if present
- Modify: `docs/byte-audit-guide.md` only if trace confidence policy changed

**Steps:**

1. `[Unreleased]` 增加 Phase 11a 段：
   - 新 decoded record view
   - fixture evidence matrix
   - coverage 是否升级
   - confidence policy
2. 更新 status 文档，把 `PSMclustertable` 从"probe only"调整为实际状态。
3. 如果 baseline 刷新，说明每个 delta 为什么是 improvement。

**Acceptance:**

- 文档不宣称未由 fixture 证明的字段。
- CHANGELOG 明确这是 additive API，旧 `entries` 保留。

### Task 9 — Verification and Commit

Run:

```bash
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo test --locked --test schema_snapshots
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
bash .github/scripts/check-byte-audit-baselines.sh
```

Suggested commits:

```text
docs(analysis): map PSMclustertable record evidence across fixtures
feat(parser): add conservative PSM cluster decoded records
docs(parser): record Phase 11a PSMclustertable field naming
```

If only one fixture is available, stop after the docs/analysis commit and do not merge parser field naming.

---

## 5. Acceptance Criteria

- [ ] Evidence matrix covers at least 2真实 fixture，或明确标记 single-fixture block。
- [ ] `PsmClusterTable` exposes additive decoded record view while preserving `entries` and `probe`.
- [ ] Parser uses checked slicing and does not panic on malformed short prefixes.
- [ ] Real fixture tests verify decoded record structure against existing entries.
- [ ] Cross-reference/report output exposes consistency status and confidence.
- [ ] Coverage status changes only when fixture evidence satisfies the ≥2 fixture rule.
- [ ] Byte-audit baselines are refreshed only for intentional trace-confidence changes.
- [ ] 5 道 pre-commit gate + schema snapshot + byte-audit baseline runner all pass.

---

## 6. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Fixture 不足 2 份 | 不能 high-confidence 命名字段 | 只提交 evidence/probe analysis，不合并 decoded 语义字段 |
| prefix 字段在 DWG / 中文 fixture 间漂移 | 字段命名可能过拟合 | 保持 `candidate_*` 或 probe，coverage 不升级 |
| `schemars` snapshot 因新增 public fields drift | schema snapshot fail | 这是预期 gate；同 PR 刷新 snapshot 并解释 additive API |
| byte-audit baseline delta 过大 | review 难判断 | 先拆 evidence commit，再单独 parser commit，并在 PR 描述列出每条 delta |
| report 输出过长 | CLI 噪声 | 默认只打印前 20 条 decoded record，probe 仍只显示前 3 条 |

---

## 7. Forward Links

- Next phase: `docs/plans/2026-04-21-phase-11b-psmsegmenttable.md` 的新版 W3 plan，应在本 Phase 产出的 cluster decoded 外键稳定后重写。
- Roadmap: `docs/plans/2026-04-29-pid-parse-roadmap.md` 阶段 B。
- Methodology: `docs/plans/2026-04-21-phase-11a-probe-psm-cluster-records-plan.md` 是本 Phase 的 probe 前置依据；本文件取代旧 `2026-04-21-phase-11a-psmclustertable-records.md` 的实现步骤。
