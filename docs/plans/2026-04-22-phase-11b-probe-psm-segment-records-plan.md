# Phase 11b-probe — PSMsegmenttable Per-Segment Probe Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 按 Phase 11a-probe 已验证的模式，给 `PSMsegmenttable` 的每条
`PsmSegmentEntry` 挂上一个 **字节级 probe**（`neighbor window` +
`owner_cluster_hint`），暴露足够的观测数据，但 **不承诺语义命名**。
Roadmap 硬约束：≥ 2 份 fixture 才能 claim `FullyDecoded`；当前样本
`flags = [0x01, 0x01, 0x01, 0x01]` 全 1，单 fixture 下无法区分
`SegmentKind::Connection` / `Geometric` / `Reference`。因此本 Phase
**只做 probe 层**，为将来的 decoded 层预留稳定基础。

**Architecture:**
- 保持 `parse_psm_segment_table` 主入参不变（pure byte → probe）
- 新增 `PsmSegmentRecordProbe`，挂到 `PsmSegmentEntry.probe: Option<_>`
- `owner_cluster_hint` 为 **派生字段**，在 `streams/psm_tables.rs`
  dispatcher 里后处理（可见 `PsmClusterTable` 上下文），保持 parser
  纯函数性
- `inspect/report.rs` 展示前 3 条 entry 的 probe 样本
- fixture 回归验证 probe ↔ flag / stream_offset / window 的字节级一致性

**Tech Stack:** Rust `src/model.rs` + `src/parsers/psm_tables.rs` +
`src/streams/psm_tables.rs` + `src/inspect/report.rs` + 单测 + fixture 断言。

---

## Task 1: 定义 `PsmSegmentRecordProbe` 模型

**Files:**
- Modify: `src/model.rs`
- Test: `src/parsers/psm_tables.rs`

**Step 1: 写失败测试**

在 `parse_psm_segment_table` 的测试模块里新增：

```rust
#[test]
fn segment_table_entries_expose_byte_level_probe() {}
```

至少断言：

- 每条 `PsmSegmentEntry.probe` 是 `Some`
- probe 里暴露：
  - `flag_hex: String`（单字节 hex，两位大写，如 `"01"`）
  - `neighbor_window_hex: String`（当前字节 ±3 字节窗口的 hex，空格分隔；
    边界处自动截断）
  - `stream_offset: usize`（= `flags_start + index`，与 `entry.offset` 一致）
  - `owner_cluster_hint: Option<String>`（parser 阶段总为 `None`；
    dispatcher 才填）

**Step 2: 扩展模型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct PsmSegmentRecordProbe {
    pub flag_hex: String,
    pub neighbor_window_hex: String,
    pub stream_offset: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_cluster_hint: Option<String>,
}
```

并在 `PsmSegmentEntry` 下新增：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub probe: Option<PsmSegmentRecordProbe>,
```

`#[serde(default)]` + `skip_if_none`，旧 JSON 兼容。

**Step 3: 跑编译**

Run: `cargo build --all-targets`

Expected: 编译通过；测试仍 FAIL（probe 未填）。

**Step 4: Commit**

```bash
git add src/model.rs src/parsers/psm_tables.rs
git commit -m "feat(model): add PSM segment record probe type (Phase 11b-probe)"
```

## Task 2: parser 填充 probe（parser 纯函数层）

**Files:**
- Modify: `src/parsers/psm_tables.rs`

**Step 1: 在 `parse_psm_segment_table` 里生成 probe**

在 push `PsmSegmentEntry` 之前构造 probe：

```rust
let probe = build_segment_record_probe(data, flags_start + i, &flag);
```

实现 `fn build_segment_record_probe(stream: &[u8], offset: usize, flag: &u8) -> PsmSegmentRecordProbe`：

- `flag_hex = format!("{flag:02X}")`
- `neighbor_window_hex`：取 `stream[offset.saturating_sub(3) ..= min(offset+3, stream.len()-1)]`
  的 hex，空格分隔；仅当 `stream` 非空
- `stream_offset = offset`
- `owner_cluster_hint = None`（dispatcher 层才填）

**Step 2: 跑测试**

Run: `cargo test --lib parsers::psm_tables::tests::segment_table_entries_expose_byte_level_probe`

Expected: PASS

**Step 3: 增补小测试**

- flag 位于流尾部 → window 向左截断
- flag 位于流起点附近 → window 向右截断
- 1 字节流 → window = "01"（单字节）

**Step 4: Commit**

```bash
git add src/parsers/psm_tables.rs
git commit -m "feat(parser): emit byte-level probe for each PSM segment record (Phase 11b-probe)"
```

## Task 3: dispatcher 填 `owner_cluster_hint` + inspect report

**Files:**
- Modify: `src/streams/psm_tables.rs`
- Modify: `src/inspect/report.rs`

**Step 1: dispatcher 后处理**

在 `streams/psm_tables.rs` 负责把 raw 流解析为 `PsmSegmentTable` 的调用点
之后，访问 `doc.psm_cluster_table` 并给 probe.owner_cluster_hint 填值。

**hint 策略（保守）：**
- 若 `psm_segment_table.flags.len() == psm_cluster_table.entries.len()`
  → 按下标 1:1 分配，第 i 个 segment hint 到第 i 个 cluster 的 `name`
- 否则 → 全部保留 `None`（避免过度 claim）

**Step 2: 写 dispatcher 单测**

测试两条路径：
- `cluster.len == segment.len` → 每条 hint 等于对应 cluster name
- `cluster.len != segment.len` → 所有 hint 均为 None

**Step 3: inspect report 展示**

在 PSMsegmenttable section 下（找 `doc.psm_segment_table` 打印处），为前
3 条 entry 多打一行 probe：

```text
--- PSMsegmenttable (size=12B, count=4) ---
  [0] offset=0008 flag=0x01
      probe: flag=01 window=[.. ..] owner_hint=PSMcluster0
  [1] offset=0009 flag=0x01
      probe: flag=01 window=[.. ..] owner_hint=StyleCluster
  ...
```

**Step 4: 写 inspect 单测**

```rust
#[test]
fn report_shows_psm_segment_record_probe_sample() {}
```

断言：
- 前 3 条 entry 显示 `probe:` 样本
- 样本含 `flag=` / `window=[...]` / `owner_hint=` 关键字

**Step 5: 跑测试**

```bash
cargo test --lib streams::psm_tables
cargo test --lib inspect::report::tests::report_shows_psm_segment_record_probe_sample
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/streams/psm_tables.rs src/inspect/report.rs
git commit -m "feat(report): show PSM segment record probe samples (Phase 11b-probe)"
```

## Task 4: 真实 fixture 回归

**Files:**
- Modify: `tests/parse_real_files.rs`

**Step 1: 写测试**

```rust
#[test]
fn psm_segment_record_probes_align_with_flags() {}
```

断言（对 `DWG-0201GP06-01.pid`）：

- 每条 `PsmSegmentEntry.probe` 是 `Some`
- `probe.flag_hex == format!("{:02X}", entry.flag)`
- `probe.stream_offset == entry.offset`
- `probe.neighbor_window_hex` 拆分后 token 数 ∈ `[1, 7]`
- 若 `psm_cluster_table.entries.len() == flags.len()` → 每条 hint 等于
  对应 cluster name
- 若不等 → 所有 hint 均为 None

**Step 2: 运行**

```bash
cargo test --test parse_real_files psm_segment_record_probes_align_with_flags
```

Expected: PASS（缺 fixture 则 skip）

**Step 3: Commit**

```bash
git add tests/parse_real_files.rs
git commit -m "test(parser): verify PSM segment record probes against fixture (Phase 11b-probe)"
```

## Task 5: 全量验证

Run:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Expected: 全绿 + fmt 干净。

若有 fmt 增量则单独一次 `chore(fmt): ...` 提交。

## 设计约束

### 为什么只做 probe 不做 decoded 层

- roadmap 硬约束 = ≥ 2 fixture 才能 claim FullyDecoded
- 单 fixture 的 `flag=[0x01, 0x01, 0x01, 0x01]` 全 1，无法区分 kind
- probe 层提供可观察数据，让后续 fixture 到位后能快速确认命名
- 与 Phase 11a-probe 的节奏保持一致

### hint 为什么采用 "1:1 顺序"

- SmartPlant 样本里 `flags.len() == cluster_count == 4`，1:1 是最自然猜测
- 但 **不承诺** `segment_id -> cluster_id` 就是 1:1；`owner_cluster_hint`
  字段名明确是 hint，decoded 层可以覆盖或修正
- 若不相等，**不提供 hint** 比乱猜更安全

### 不要做的事

- 不要给 probe 字段起语义化名字（如 `segment_kind` / `role` / `target_id`）
- 不要修改 `PsmSegmentEntry` 已有字段
- 不要扩展 `PsmSegmentTable` 顶层字段
- 不要在 layout / crossref / import_view 里消费 probe
- 不要升级 `psm_segment_table` 的 coverage 分类（保持 `PartiallyDecoded`）
- 不要触碰 writer 路径

### SemVer

Patch（新增 Option 字段，默认 None；旧 JSON 兼容）。

## 完成定义

- `PsmSegmentEntry.probe: Option<PsmSegmentRecordProbe>` 就绪并填充
- dispatcher 在 `cluster.len == segment.len` 时填 `owner_cluster_hint`
- report 展示前 3 条 probe 样本
- fixture 回归断言 probe ↔ flag / offset / window / hint 一致性
- 全量测试持续绿；clippy / fmt 双零

## 后续

当第 2 份 fixture 到位时，按 `docs/plans/2026-04-21-phase-11b-psmsegmenttable.md`
在 probe 基础上：

1. 命名 `inferred_kind`（Connection / Geometric / Reference / Unknown）
2. 把 `owner_cluster_hint` 升级为 `owner_cluster_id`（基于 Phase 11a-decoded
   的 `declared_segment_count`）
3. 引入 `CrossReferenceGraph::SegmentReconciliation` 三向对账
4. coverage 升级到 `FullyDecoded`（confidence=high）

## 交叉引用

- 前置：Phase 11a-probe（PSMclustertable 字节级 probe，已于 `fc36a59` ship）
- 平行对标：`docs/plans/2026-04-21-phase-11a-probe-psm-cluster-records-plan.md`
- 完整 Phase 11b plan：`docs/plans/2026-04-21-phase-11b-psmsegmenttable.md`
  （下一步 decoded 层引用）
- SPPID 总 roadmap：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 2.3**
