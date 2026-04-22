# Phase 12b-1-minimal — Byte-Audit Scaffolding Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 roadmap Phase 4（`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md`）
里"每个 parser 都要能回答'解释了哪些字节'"的能力奠基。在 **一次 ship**
里只交付最小可行的基础设施 + 一个示范 parser 的 trace 迁移，让后续
parser 迁移走成熟模板。

本 plan 是 Phase 12b 原 plan
(`docs/plans/2026-04-21-phase-12b-byte-audit-framework.md`) 里 **12b-1**
的收紧版 —— 原 12b-1 含 "2-3 个示范 parser + CLI flag"，此 minimal
版再砍一半，只做 "核心类型 + 1 个 parser + 公共 API"，方便一次性 ship。

**Architecture:**
- 新模块 `src/byte_audit/mod.rs`，与现有 `inspect/coverage` 完全正交
  （coverage 是**流级别**状态，byte-audit 是**字节级别**范围）
- 零破坏：旧 parser API 保留为 thin wrapper，不接 trace 的 consumer
  无感知
- 新 pub API 通过 `lib.rs` 导出 `ByteRange` / `TraceConfidence` /
  `ParserTrace` / `ParserTraceBuilder` / `parse_psm_segment_table_with_trace`
- **不** 暴露到 `schema.rs`（先稳定几个 release 再 surface 给外部 JSON
  schema 消费者）

**Tech Stack:** Rust 新模块 + 单测；不依赖任何 fixture；不需要新 crate。

---

## Task 1: ByteRange 类型

**Files:**
- Create: `src/byte_audit/mod.rs`
- Test: 同文件内 `#[cfg(test)]`

**Step 1: 写 ByteRange + 单测**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
         Serialize, Deserialize, JsonSchema)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,  // exclusive
}

impl ByteRange {
    pub fn new(start: u64, end: u64) -> Self { ... }
    pub fn len(&self) -> u64 { self.end.saturating_sub(self.start) }
    pub fn is_empty(&self) -> bool { self.end <= self.start }
    pub fn overlaps(&self, other: &Self) -> bool { ... }
    pub fn contains_offset(&self, offset: u64) -> bool { ... }
}
```

**断言覆盖：**

- `len` / `is_empty` / `overlaps` 基本行为
- `start > end` 时 `len == 0` 且 `is_empty == true`
- 两个相邻非相交 range 不 overlap（`[0..4]` vs `[4..8]`）
- `Ord` 按 `(start, end)` 字典序

**Step 2: Commit**

```bash
git add src/byte_audit/mod.rs
git commit -m "feat(byte_audit): scaffold ByteRange primitive (Phase 12b-1)"
```

## Task 2: TraceConfidence + ParserTrace + ParserTraceBuilder

**Files:**
- Modify: `src/byte_audit/mod.rs`

**Step 1: TraceConfidence 枚举**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
         Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TraceConfidence {
    /// Fully-named field with stable semantics.
    Decoded,
    /// Byte layout isolated but no semantic claim (Phase 11a-/11b-probe
    /// flavoured output).
    Probed,
    /// Bytes passed through unchanged, no interpretation attempted.
    Raw,
}
```

**Step 2: ParserTrace 聚合类型**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParserTrace {
    pub parser_name: String,
    pub stream_path: String,
    pub total_bytes: u64,
    pub consumed_ranges: Vec<ByteRange>,
    pub leftover_ranges: Vec<ByteRange>,
    pub ranges_by_confidence:
        std::collections::BTreeMap<TraceConfidence, Vec<ByteRange>>,
}

impl ParserTrace {
    pub fn consumed_bytes(&self) -> u64 { ... }
    pub fn leftover_bytes(&self) -> u64 { ... }
    pub fn coverage_ratio(&self) -> f32 { ... }
}
```

**Step 3: ParserTraceBuilder**

```rust
#[derive(Debug, Clone)]
pub struct ParserTraceBuilder {
    parser_name: String,
    ranges: Vec<(ByteRange, TraceConfidence)>,
}

impl ParserTraceBuilder {
    pub fn new(parser_name: impl Into<String>) -> Self { ... }
    pub fn consume(&mut self, range: ByteRange, c: TraceConfidence) { ... }
    pub fn build(self, stream_path: impl Into<String>, total_bytes: u64)
        -> ParserTrace
    {
        // 1. 按 start 排序 ranges
        // 2. 合并相邻 / 重叠 consumed_ranges（保留原始 confidence 分组）
        // 3. 计算 leftover_ranges = [0..total_bytes] \ consumed
        // 4. ranges_by_confidence = 按 TraceConfidence 桶装
    }
}
```

**断言覆盖：**

- `consume` 空 range → no-op
- 合并：`[0..4]` + `[4..8]` → `[0..8]`（相邻 same-confidence 合并）
- 不合并：`[0..4] Decoded` + `[4..8] Probed` → 保持两条
- leftover：总 10 字节，consumed `[0..4], [6..8]` → leftover `[4..6], [8..10]`
- `coverage_ratio()` 对 `total_bytes == 0` 时返回 `0.0`，不 panic
- `consumed_bytes + leftover_bytes == total_bytes`（不变式）

**Step 4: Commit**

```bash
git add src/byte_audit/mod.rs
git commit -m "feat(byte_audit): ParserTrace + ParserTraceBuilder (Phase 12b-1)"
```

## Task 3: parse_psm_segment_table_with_trace 示范迁移

**Files:**
- Modify: `src/parsers/psm_tables.rs`
- Modify: `src/lib.rs`（pub use byte_audit）

**Step 1: 新 `_with_trace` 变体**

```rust
pub fn parse_psm_segment_table_with_trace(
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<PsmSegmentTable> {
    let magic = read_u32_le(data, 0)?;
    if magic != STAB_MAGIC {
        return None;
    }
    trace.consume(ByteRange::new(0, 4), TraceConfidence::Decoded);  // magic
    let count = read_u32_le(data, 4)?;
    trace.consume(ByteRange::new(4, 8), TraceConfidence::Decoded);  // count

    let flags_start = 8usize;
    let flags_end = flags_start.checked_add(count as usize)?;
    if flags_end > data.len() {
        return None;
    }
    // 每个 flag 单独标 Probed（因为 flag 语义尚未命名 —— Phase 11b-probe
    // 一致）
    for i in 0..(count as usize) {
        let off = (flags_start + i) as u64;
        trace.consume(ByteRange::new(off, off + 1), TraceConfidence::Probed);
    }
    // 其余 bytes 保持 leftover（trailing_bytes 不 consume）

    // 构造和旧版一致的 PsmSegmentTable
    ...
}
```

**Step 2: 原 API 改为 thin wrapper**

```rust
pub fn parse_psm_segment_table(data: &[u8]) -> Option<PsmSegmentTable> {
    let mut trace = ParserTraceBuilder::new("parse_psm_segment_table");
    parse_psm_segment_table_with_trace(data, &mut trace)
}
```

验证：原有 10+ 个 `parse_psm_segment_table` 单测全部保持绿。

**Step 3: 新增 `_with_trace` 专属单测**

- `trace_coverage_is_complete_for_header_and_flags`：断言 consumed
  应 = `8 + count`；trailing bytes 应进 leftover
- `trace_magic_and_count_are_marked_decoded`：TraceConfidence 分桶正确
- `trace_each_flag_is_marked_probed`：flag 区间的 confidence 正确

**Step 4: lib.rs pub use**

```rust
pub mod byte_audit;
pub use byte_audit::{
    ByteRange, ParserTrace, ParserTraceBuilder, TraceConfidence,
};
```

**Step 5: Commit**

```bash
git add src/parsers/psm_tables.rs src/lib.rs
git commit -m "feat(parser): trace-aware parse_psm_segment_table (Phase 12b-1)"
```

## Task 4: 全量验证

Run:

```bash
cargo build --all-targets
cargo test --all-targets
```

Expected: 全绿；新增测试数 ≥ 12；lib.rs 导出无冲突。

## 设计约束

### 为什么不加 CLI / report / aggregate

- 一次 ship 范围越大，回归面越大
- CLI 层和 ByteAuditReport 聚合是 Phase 12b-2 的主菜
- 先让核心类型稳定、用一个示范 parser 验证模式、留下可扩展的 pub API

### 为什么不改 coverage module

- `inspect/coverage.rs` 是 **流级别**状态机；byte-audit 是 **字节级别**范围
- 两者可共存但不共生：将来 `ParseCoverageStatus::PartiallyDecoded`
  可以在其 note 里引用 `ParserTrace.coverage_ratio`，但那是 12b-2+

### 为什么选 PSMsegmenttable 作为示范 parser

- 刚在 Phase 11b-probe 碰过，代码新鲜度最高
- 字节布局简单（magic + count + N×1B flags），trace 语义清晰
- trailing_bytes 不消费 → 天然检验 leftover 计算

### 不要做的事

- 不要给 ByteAuditReport / StreamAuditSummary 起 stub（等 12b-2）
- 不要 touch `src/bin/pid_inspect.rs`
- 不要修改 `src/schema.rs`（新类型先不 surface 到 JSON schema）
- 不要迁移任何其他 parser（除 `parse_psm_segment_table`）
- 不要 bump 版本号（功能尚未到可 release 的节点）

### SemVer

Patch（新增 pub module + 新增 API，零破坏）。

## 完成定义

- `byte_audit` 模块就绪，含 `ByteRange` / `TraceConfidence` /
  `ParserTrace` / `ParserTraceBuilder`
- `parse_psm_segment_table_with_trace` 可用；旧 API 改 thin wrapper
  且行为 byte-identical
- `lib.rs` 暴露核心类型
- 新增单测数 ≥ 12；既有测试 0 回归
- cargo build / test 全绿

## 后续

- **Phase 12b-1b** 可选加餐：再迁 1-2 个 parser（`parse_psm_cluster_table`
  / `parse_psm_roots`）验证模式复用性
- **Phase 12b-2**：全 15 parser 迁移 + `ByteAuditReport` 聚合入口
- **Phase 12b-3**：`pid_inspect --byte-audit` CLI flag + CI baseline 回归守

## 交叉引用

- 上游：`docs/plans/2026-04-21-phase-12b-byte-audit-framework.md` 的 12b-1
- 平行：Phase 11a-probe / 11b-probe 的 `Probed` 命名沿用
- SPPID roadmap：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 4**
