# 开发计划：Phase 12b — consumed/leftover 字节验证框架

> 起稿：2026-04-21
> 目标版本：v0.14.0（minor bump；若在 Phase 12a major bump 之后则为 v1.1.0）
> 前置：Phase 12a ship（normalized + provenance 基础设施，非强制但强推荐）
> 估计工时：12-18 hr（可分 3 轮 ship）
> 所属 roadmap：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 4**

## 动机

roadmap Phase 4 原文：

> 避免"看起来解析对了，但实际上只覆盖了部分字节"的假象。
> 为 parser 增加 consumed-range / leftover-range 报告。
> 每种结构建立：单元测试 + fixture 测试 + 真实文件 golden 测试 + 跨流一致性检查。
> 增加关键一致性断言：
> - record count 与实际遍历数量一致
> - 交叉引用无不可解释悬空
> - stream size 与已消费区间一致
> - 未解析区间明确可见

当前 (v0.7.0) 的 coverage 体系（Phase 10a-10f）提供了**流级别**的覆盖状态与字节总量，但**流内部**的 consumed / leftover 仍不可见。这意味着：

- 一个"看起来 FullyDecoded 的流"可能只用了 header 10 字节，后 10KB body 完全 untouched — 静态 coverage 看不出来
- 字节级保真回归（如 Writer passthrough）只能靠整流 diff，没有"哪段被哪个 parser 消费"的溯源

Phase 12b 把每个 parser 改造为 **byte-range-aware**，让每条字节都有明确的消费状态，回归守防止隐式退化。

## 非目标

- **不改** 现有 parser 的语义输出（只加 consumed-range 报告）
- **不做** parser fuzz 或 coverage-guided testing
- **不生成** 针对未消费字节的自动解释（那是未来 parser 工作的输入）
- **不接** Writer 层（Writer 走 passthrough + delta，本来就字节级；byte-audit 只管 reader 侧）

## 核心设计

### 1. ParserTrace 类型

```rust
// src/byte_audit/mod.rs（新模块）

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,  // exclusive
}

impl ByteRange {
    pub fn len(&self) -> u64 { self.end - self.start }
    pub fn overlaps(&self, other: &ByteRange) -> bool { self.start < other.end && other.start < self.end }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParserTrace {
    pub stream_path: String,
    pub total_bytes: u64,
    pub consumed_ranges: Vec<ByteRange>,      // 已解释
    pub leftover_ranges: Vec<ByteRange>,      // 未解释（= total - consumed，已去除 overlap）
    pub confidence_by_range: BTreeMap<ByteRange, Confidence>,
    pub parser_name: String,                   // "parse_psm_cluster_table" 等
}

impl ParserTrace {
    pub fn consumed_bytes(&self) -> u64 { self.consumed_ranges.iter().map(|r| r.len()).sum() }
    pub fn leftover_bytes(&self) -> u64 { self.leftover_ranges.iter().map(|r| r.len()).sum() }
    pub fn coverage_ratio(&self) -> f32 { self.consumed_bytes() as f32 / self.total_bytes as f32 }
}
```

### 2. ByteAuditReport（顶层聚合）

```rust
pub struct ByteAuditReport {
    pub traces: Vec<ParserTrace>,
    pub total_file_bytes: u64,
    pub overall_consumed: u64,
    pub overall_leftover: u64,
    pub per_stream_summary: BTreeMap<String, StreamAuditSummary>,
}

pub struct StreamAuditSummary {
    pub path: String,
    pub total_bytes: u64,
    pub consumed_bytes: u64,
    pub leftover_bytes: u64,
    pub coverage_ratio: f32,
    pub parsers_involved: Vec<String>,  // 一个流可能有多个 parser 协作
    pub leftover_hex_preview: Option<String>,  // 前 64B leftover 的 hex dump
}
```

### 3. Parser 改造策略

**不改既有 API，加一个 side-channel**。

选项 A（侵入式）：每个 parser 返回 `Result<(T, ParserTrace), PidError>` — API 大破坏

选项 B（side-channel，**推荐**）：parser 接收 `&mut ParserTraceBuilder` 参数，按字节推进：

```rust
// 旧
pub fn parse_psm_cluster_table(bytes: &[u8]) -> Option<PsmClusterTable> { ... }

// 新（back-compat wrapper）
pub fn parse_psm_cluster_table(bytes: &[u8]) -> Option<PsmClusterTable> {
    let mut trace = ParserTraceBuilder::new();
    parse_psm_cluster_table_with_trace(bytes, &mut trace)
}

pub fn parse_psm_cluster_table_with_trace(
    bytes: &[u8],
    trace: &mut ParserTraceBuilder,
) -> Option<PsmClusterTable> {
    trace.consume(0..8, Confidence::Decoded);  // header
    // ... record-by-record 解析，每读一段调 trace.consume(range, conf)
}

pub struct ParserTraceBuilder {
    pub parser_name: String,
    pub ranges: Vec<(ByteRange, Confidence)>,
}

impl ParserTraceBuilder {
    pub fn consume(&mut self, r: impl Into<ByteRange>, c: Confidence) { ... }
    pub fn build(self, stream_path: &str, total: u64) -> ParserTrace { ... }
}
```

这样旧 consumer 调 `parse_psm_cluster_table(bytes)` 无感；想拿 trace 的调 `parse_psm_cluster_table_with_trace`。

### 4. byte-audit 汇聚入口

```rust
// src/byte_audit/mod.rs
pub fn byte_audit_report(pkg: &PidPackage) -> ByteAuditReport {
    let mut traces: Vec<ParserTrace> = Vec::new();
    for (path, stream) in &pkg.streams {
        // 调各 parser 的 _with_trace 版本
        if path == "/PSMclustertable" {
            let mut b = ParserTraceBuilder::new("parse_psm_cluster_table");
            let _ = parse_psm_cluster_table_with_trace(&stream.data, &mut b);
            traces.push(b.build(path, stream.data.len() as u64));
        }
        // ... 每种已知 parser 一次
    }
    aggregate_traces(traces, pkg)
}
```

### 5. 一致性断言库

```rust
// src/byte_audit/assertions.rs
pub fn assert_record_count_matches(trace: &ParserTrace, declared: u32, actual: u32) -> Result<(), AuditError> { ... }
pub fn assert_stream_fully_consumed(trace: &ParserTrace) -> Result<(), AuditError> { ... }
pub fn assert_coverage_ratio_ge(trace: &ParserTrace, threshold: f32) -> Result<(), AuditError> { ... }
pub fn assert_leftover_is_hex_pattern(trace: &ParserTrace, pattern: &[u8]) -> Result<(), AuditError> { ... }
```

### 6. CLI `--byte-audit`

```bash
pid_inspect file.pid --byte-audit
```

输出：

```
=== Byte Audit Report ===

Total file bytes: 196 608
Overall consumed: 187 234 (95.2%)
Overall leftover:   9 374 ( 4.8%)

--- Per-Stream ---
  [100%] /PSMroots (278 B)               parse_psm_roots
  [ 87%] /PSMclustertable (265 B)         parse_psm_cluster_table
         leftover: @0F8..110 (24 B)  hex: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
  [100%] /DocVersion3 (192 B)              parse_doc_version3
  [ 42%] /Sheet6 (29594 B)                 parse_sheet_body (Phase 11c)
         leftover: @1A24..7258 (20540 B)  hex: ...
  [ 0%] /DocVersion2 (48 B)                unregistered parser
         leftover: @0000..0030 (48 B)  hex: 34 00 01 00 ... (full)
  ...

--- Coverage Trajectory ---
  Phase 10f baseline:   N/A
  Phase 11a baseline:   N/A
  Current (Phase 12b):  95.2%

--- Assertions ---
  [PASS] record counts consistent across parsers
  [PASS] sheet decoded_bytes == sum of SheetRecord consumed ranges
  [WARN] /PSMclustertable trailing 24B uninterpreted
```

### 7. CI 回归守

```yaml
# .github/workflows/ci.yml 新 job
- name: Byte audit regression
  run: |
    cargo run --bin pid_inspect -- fixture.pid --byte-audit --json > audit-current.json
    ./scripts/compare-audit.sh audit-baseline.json audit-current.json
    # 脚本断言：overall_coverage 不能降，且每个已知流的 consumed_bytes 不能降
```

## 范围（预估）

| 文件 | 改动 | 行数 |
|---|---|---|
| `src/byte_audit/mod.rs` 新模块 | ParserTrace / ByteAuditReport / ParserTraceBuilder | +300 |
| `src/byte_audit/assertions.rs` | 断言 helpers | +150 |
| 每个 parser 文件（~15 个）| 添加 `_with_trace` 变体 | +30 × 15 = +450 |
| `src/bin/pid_inspect.rs` | `--byte-audit` flag | +80 |
| `tests/byte_audit.rs` 新测试文件 | 覆盖率 trajectory 回归守 | +200 |
| `CHANGELOG.md` | `[0.14.0]` | +120 |
| `docs/byte-audit-guide.md` 新文档 | 使用指南 | +150 |
| `.github/workflows/ci.yml` | CI job | +30 |
| **本 plan** | | +本文件 |

~1500 行，分 3 轮 ship。

## 实施阶段

### Phase 12b-1：基础设施（~4-5 hr）

- `byte_audit` 模块 + 核心类型
- `ParserTraceBuilder` 测试
- 1-2 个 "示范 parser" 迁移（比如 PSMclustertable / DocVersion3）
- Ship `v0.14.0` (minor)：新模块 + 新 CLI flag，但只覆盖 2-3 个 parser

### Phase 12b-2：全 parser 迁移（~5-7 hr）

- 所有 ~15 个 parser 加 `_with_trace` 变体
- CLI 输出完整 per-stream 覆盖
- Ship `v0.14.1` (patch)：内部扩展

### Phase 12b-3：CI 回归守 + 文档（~3-5 hr）

- `.github/workflows/ci.yml` byte-audit job
- baseline JSON 入 repo
- `docs/byte-audit-guide.md`
- Ship `v0.14.2` (patch)

## 预计工时

- 12b-1: 4-5 hr
- 12b-2: 5-7 hr
- 12b-3: 3-5 hr（含 CI）
- 文档分散: 1 hr
- **合计 13-18 hr**（区间 12-18 hr）

## 验证清单

### Phase 12b-1

- [ ] ParserTrace / ByteRange 单测
- [ ] 2 个示范 parser 的 `_with_trace` 变体工作
- [ ] `pid_inspect --byte-audit` 输出部分流的覆盖

### Phase 12b-2

- [ ] 所有 ~15 parser 有 `_with_trace` 变体
- [ ] 未迁移 parser（比如 Phase 11c 的新加）被明确标为 "unregistered"
- [ ] CLI 输出所有已知流的 consumed / leftover

### Phase 12b-3

- [ ] CI 有 byte-audit job
- [ ] baseline JSON 提交到 repo
- [ ] coverage_ratio 回归阈值（比如不能下降 > 1%）
- [ ] `docs/byte-audit-guide.md` 完整

## 风险

| 风险 | 缓解 |
|---|---|
| 15 个 parser 迁移工作量大 | 拆到 3 轮 ship；12b-1 先做基础设施 + 示范，验证模式成熟后再扩 |
| `_with_trace` 变体和原版 drift | 原版改为调 `_with_trace` + 丢弃 trace 的 thin wrapper；两者永远同步 |
| CI baseline 要更新时争议 | baseline 变化需要 PR review；自动更新脚本但需人工审核 |
| Phase 11c Sheet parser 未迁移 → 覆盖率虚低 | 明确标为 "unregistered"，不影响已覆盖 parser 的回归守 |
| 跨 parser 协作的流（比如 DynamicAttributes）算哪个 parser | `StreamAuditSummary.parsers_involved: Vec<String>` 记录多个参与者 |

## SemVer 判定

- 新模块 + 新 CLI flag：minor（0.13 → 0.14）
- 后续 parser 迁移：patch（内部改善）
- CI 回归守：不影响 API，patch
- 总体：**minor bump + 2 patch bumps**

## Next 候选（12b 完成后）

roadmap Phase 5 验收：

- 所有顶层流 FullyDecoded + 动态探针
- `unknown_streams` 仅样本特异
- 2-3 个代表性样本 leftover < 5%
- Byte-audit baseline coverage > 95%

达成后项目进入"稳定迭代"模式，主要是：
- 多 fixture 扩展
- Writer 层的细化 feature
- 消费者 binding（WASM / Python / Node.js）

## 交叉引用

- 上游总 roadmap：`docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` 阶段 D
- SPPID 战略：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` **Phase 4**
- 前置 Phase：12a（推荐但非强制）
- 配套 Phase：所有 Phase 11 parser 的迁移 pass
