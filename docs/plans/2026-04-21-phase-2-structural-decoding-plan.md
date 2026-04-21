# Phase 2 Structural Decoding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `DocVersion3`、`PSMclustertable`、`PSMsegmenttable` 从当前“可用但部分语义化”的状态推进为更稳定、可验证、可扩展的结构化解码层。

**Architecture:** 保持现有 `streams/* -> parsers/* -> model.rs -> inspect/report` 分层不变，在 parser/model 层补充更细粒度的结构字段与审计信息，再由 `inspect` 消费这些新字段输出更可信的结构摘要。实现顺序遵循“先最稳的 `DocVersion3`，再 `PSMclustertable`，最后 `PSMsegmenttable`”，避免多流耦合一起增长。

**Tech Stack:** Rust、现有 `pid_parse` parser/model/report 架构、真实 `.pid` fixture、单元测试、集成测试

---

### Task 1: 收紧 `DocVersion3` 的结构模型与解析约束

**Files:**
- Modify: `src/model.rs`
- Modify: `src/parsers/doc_version.rs`
- Modify: `src/inspect/report.rs`
- Test: `src/parsers/doc_version.rs`
- Test: `src/inspect/report.rs`

**Step 1: 写失败测试，锁定更强结构**

在 `src/parsers/doc_version.rs` 新增测试，至少覆盖：

- 每条 record 固定 48 字节
- `product`、`version`、`operation`、`timestamp` 都来自固定槽位
- 非法 record 不应 silently 混入结果
- 末尾残余字节会被记录

示例目标：

```rust
#[test]
fn doc_version3_records_expose_record_offset_and_trailing_bytes() {}

#[test]
fn doc_version3_rejects_record_with_empty_product() {}
```

**Step 2: 跑测试确认失败**

Run: `cargo test doc_version -- --nocapture`

Expected: FAIL，因为 offset / trailing bytes / stronger validation 还不存在。

**Step 3: 扩展模型**

在 `src/model.rs` 中调整：

- `VersionHistory` 增加：
  - `record_size: usize`
  - `trailing_bytes: usize`
- `VersionRecord` 增加：
  - `offset: usize`
  - 可选：`raw_operation: String`（如果你想区分规范化 operation 与原始值）

保持现有 JSON 兼容风格，新增字段不应破坏旧逻辑。

**Step 4: 实现最小 parser 升级**

在 `src/parsers/doc_version.rs` 中：

- 明确按 48-byte record 切分
- 要求 `product` 非空、`version` 非空时才接收记录
- 记录每条 `offset`
- 解析停止后，把剩余未消费字节记入 `trailing_bytes`
- `record_size` 固定写 48

不要过早引入日期解析或版本号数值化，先保持字符串层稳定。

**Step 5: 更新 report**

在 `src/inspect/report.rs` 的 Version History 段补充：

- `record_size`
- `trailing_bytes`（仅当 > 0）

**Step 6: 跑测试**

Run: `cargo test doc_version inspect::report -- --nocapture`

Expected: PASS

**Step 7: Commit**

```bash
git add src/model.rs src/parsers/doc_version.rs src/inspect/report.rs
git commit -m "feat(docversion3): add structural offsets and trailing byte audit"
```

### Task 2: 为 `PSMclustertable` 建立 record 级结构而不是仅提取名称

**Files:**
- Modify: `src/model.rs`
- Modify: `src/parsers/psm_tables.rs`
- Modify: `src/inspect/report.rs`
- Test: `src/parsers/psm_tables.rs`
- Test: `tests/parse_real_files.rs`

**Step 1: 写失败测试**

在 `src/parsers/psm_tables.rs` 增加测试，至少锁定：

- 每个 cluster entry 不只有 `name`
- 还要暴露：
  - `record_offset`
  - `name_offset`
  - `raw_record_len` 或 `record_len`
  - 可能的 header bytes / prefix bytes
- parser 对短 UTF-16 名称、噪音前缀、尾部残余的行为稳定

示例：

```rust
#[test]
fn cluster_table_entry_records_offsets_and_prefix_bytes() {}

#[test]
fn cluster_table_reports_trailing_or_unparsed_bytes() {}
```

**Step 2: 跑测试确认失败**

Run: `cargo test psm_tables -- --nocapture`

Expected: FAIL，因为当前 `PsmClusterEntry` 只有 `name` 和 `name_offset`。

**Step 3: 扩展模型**

在 `src/model.rs` 中调整：

- `PsmClusterTable` 增加：
  - `trailing_bytes: usize`
  - 可选：`unparsed_regions: Vec<ByteSpan>`（如果你想一步到位）
- `PsmClusterEntry` 增加：
  - `record_offset: usize`
  - `record_len: usize`
  - `prefix_hex: String` 或 `prefix_bytes: Vec<u8>`
  - 可选：`declared_index: Option<u32>`
  - 可选：`flags: Option<u32>`

初版允许字段仍命名为 audit 风格，不必假装已完全知道语义。

**Step 4: 升级 parser**

不要继续只做“全流扫描 UTF-16 名称”。改成：

- 仍从 `count` 开始
- 以“发现一个 UTF-16 name”为 anchor
- 向前回溯有限窗口，截出一个 record slice
- 把这段 slice 的：
  - 起始 offset
  - 长度
  - name offset
  - prefix bytes
 记录进 entry
- parser 最后统计未覆盖区，生成 `trailing_bytes`

如果还不能稳定恢复 `declared_index`/`flags`，就先不命名，只保留 `prefix_hex`。

**Step 5: 更新 report**

在 `src/inspect/report.rs` 中的 `PSMclustertable` section 输出：

- declared count
- extracted entries
- trailing bytes
- 每项显示：
  - `record_offset`
  - `name_offset`
  - `record_len`
  - `name`

**Step 6: 增加真实样本断言**

在 `tests/parse_real_files.rs` 增加至少一个断言：

- `doc.psm_cluster_table` 存在
- 每个 entry `record_len > 0`
- entry 名称与现有 cluster/sheet 名称集合存在合理交集

**Step 7: 跑测试**

Run: `cargo test psm_tables parse_real_files -- --nocapture`

Expected: PASS

**Step 8: Commit**

```bash
git add src/model.rs src/parsers/psm_tables.rs src/inspect/report.rs tests/parse_real_files.rs
git commit -m "feat(psm): add structural records for cluster table"
```

### Task 3: 为 `PSMsegmenttable` 从 flag-array 升级到 segment-entry 模型

**Files:**
- Modify: `src/model.rs`
- Modify: `src/parsers/psm_tables.rs`
- Modify: `src/inspect/report.rs`
- Test: `src/parsers/psm_tables.rs`
- Test: `tests/parse_real_files.rs`

**Step 1: 写失败测试**

为 `PSMsegmenttable` 增加测试，锁定：

- 每个 segment 不只是一个裸 `u8`
- 需要 entry 级结构：
  - `index`
  - `flag`
  - `offset`
- 若 `count` 与实际 payload 长度不符，返回 `None` 或显式拒绝

示例：

```rust
#[test]
fn segment_table_exposes_indexed_entries() {}
```

**Step 2: 跑测试确认失败**

Run: `cargo test psm_tables -- --nocapture`

Expected: FAIL，因为当前模型只有 `flags: Vec<u8>`。

**Step 3: 扩展模型**

在 `src/model.rs` 中：

- 新增 `PsmSegmentEntry`
- `PsmSegmentTable` 改为：
  - `entries: Vec<PsmSegmentEntry>`
  - 可保留 `flags: Vec<u8>` 作为兼容审计字段，或删除并同步调用方
  - 建议新增 `trailing_bytes: usize`

`PsmSegmentEntry` 最小字段：

```rust
pub struct PsmSegmentEntry {
    pub index: usize,
    pub offset: usize,
    pub flag: u8,
}
```

**Step 4: 升级 parser**

在 `parse_psm_segment_table` 中：

- 按 `[u32 magic][u32 count][u8 * count][optional trailing]` 解析
- 生成 `entries`
- 若尾部存在额外字节，记录 `trailing_bytes`

初版不推断 flag 语义，只把它们结构化。

**Step 5: 更新 report**

`src/inspect/report.rs` 里输出：

- count
- trailing bytes
- 样例 entries（前 N 个）

不要只打印 `flags: [0x01, ...]`

**Step 6: 更新真实样本测试**

在 `tests/parse_real_files.rs` 增加断言：

- `entries.len() == count as usize`
- 所有 `offset` 单调递增

**Step 7: 跑测试**

Run: `cargo test psm_tables parse_real_files -- --nocapture`

Expected: PASS

**Step 8: Commit**

```bash
git add src/model.rs src/parsers/psm_tables.rs src/inspect/report.rs tests/parse_real_files.rs
git commit -m "feat(psm): add indexed segment table entries"
```

### Task 4: 用交叉验证把 Phase 2 三条流串起来

**Files:**
- Modify: `tests/parse_real_files.rs`
- Optional Modify: `src/crossref.rs`（仅当你决定加入轻量交叉验证摘要）

**Step 1: 写失败测试**

增加跨流一致性断言：

- `DocVersion3.records.len()` 与 `DocVersion2.records.len()` 应相等（真实样本上）
- `PSMclustertable.entries` 与 `cluster_coverage.declared` 基本一致
- `PSMsegmenttable.count` 与 `entries.len()` 一致

**Step 2: 跑测试确认失败或部分缺口**

Run: `cargo test parse_real_files -- --nocapture`

Expected: 至少有一部分断言还没法表达或输出不足。

**Step 3: 做最小补充**

只补必要字段，不要在此任务中引入 Phase 3 的统一语义层。

**Step 4: 跑测试**

Run: `cargo test parse_real_files -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add tests/parse_real_files.rs src/crossref.rs
git commit -m "test(phase2): add cross-stream structural consistency checks"
```

### Task 5: 全量验证

**Files:**
- No code changes expected

**Step 1: 运行测试**

Run: `cargo test --all-targets`

Expected: PASS

**Step 2: 运行格式检查**

Run: `cargo fmt --all -- --check`

Expected: PASS

**Step 3: 运行 clippy**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: PASS

**Step 4: 如环境缺少 fmt/clippy**

记录事实，不编造通过；至少保证 `cargo test --all-targets` 通过。

**Step 5: Commit**

```bash
git add -A
git commit -m "chore(phase2): validate structural decoding upgrades"
```

## 实施备注

### 为什么先做 `DocVersion3`

它是三者里结构最稳定、样本最容易验证、与 `DocVersion2` 有天然交叉校验的一条流，最适合作为“Phase 2 的定锚任务”。

### 为什么 `PSMclustertable` 先于 `PSMsegmenttable`

`PSMclustertable` 直接连接 `cluster_coverage`、`doc.clusters`、`Sheet*` 这些更大的语义面；它的收益更高。`PSMsegmenttable` 当前 payload 很小，结构化收益主要是为后续 provenance 和 layout 关联打地基。

### 本阶段不要做的事

- 不要在这一阶段引入完整 consumed-range 框架
- 不要直接改 `layout` 去消费 segment provenance
- 不要把 `PSMsegmenttable.flag` 过早命名为“已知语义”
- 不要把 `DocVersion3.timestamp` 强行解析成时间对象

## 完成定义

此计划完成后，应达到：

- `DocVersion3` 具备 record 级 offset / trailing 审计信息
- `PSMclustertable` 不再只是“名称扫描器”，而是具备 record 级结构
- `PSMsegmenttable` 不再只是 `Vec<u8>`，而是 entry 化结构
- `inspect/report` 能输出更可信的结构摘要
- 真实样本测试能跨流验证这些结构没有自相矛盾
