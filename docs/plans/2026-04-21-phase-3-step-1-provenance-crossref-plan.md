# Phase 3 Step 1 Provenance-Aware Crossref Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 `cross_reference` 增加第一层 provenance-aware 规范化能力，让 `PSMclustertable`、`clusters`、`sheet_streams` 之间的匹配不再只是名字集合，而是带来源与匹配关系的结构化图。

**Architecture:** 保持现有 `CrossReferenceGraph` 顶层入口不变，在 `ClusterCoverage` 下补充 entry 级 provenance 结构，先覆盖 cluster/sheet 这条最稳定的数据链。实现上以“新增字段、不破坏旧摘要字段”为原则：保留 `declared/found/matched/...` 这些轻量统计，新增详细 records 供后续 `inspect/report/layout` 消费。

**Tech Stack:** Rust、现有 `model.rs` / `crossref.rs` / `inspect/report.rs`、真实样本测试、单元测试

---

### Task 1: 为 `ClusterCoverage` 增加 entry 级 provenance 模型

**Files:**
- Modify: `src/model.rs`
- Test: `src/crossref.rs`

**Step 1: 写失败测试**

在 `src/crossref.rs` 的测试模块里新增目标测试：

```rust
#[test]
fn cluster_coverage_records_declared_entry_provenance() {}

#[test]
fn cluster_coverage_records_found_entry_provenance() {}
```

至少断言：

- declared side 能保留 `PSMclustertable` entry 的：
  - `name`
  - `record_offset`
  - `name_offset`
  - `record_len`
- found side 能区分来源：
  - cluster stream
  - sheet stream
- matched relation 能指出一条 declared name 对应哪条 found source

**Step 2: 跑测试确认失败**

Run: `cargo test crossref::tests::cluster_coverage_ -- --nocapture`

Expected: FAIL，因为 `ClusterCoverage` 还没有 provenance records。

**Step 3: 扩展模型**

在 `src/model.rs` 中新增最小结构：

```rust
pub enum ClusterCoverageSourceKind {
    PsmCluster,
    SheetStream,
}

pub struct DeclaredClusterRef {
    pub name: String,
    pub record_offset: usize,
    pub name_offset: usize,
    pub record_len: usize,
}

pub struct FoundClusterRef {
    pub name: String,
    pub source_kind: ClusterCoverageSourceKind,
    pub path: String,
}

pub struct ClusterCoverageMatch {
    pub name: String,
    pub declared_index: usize,
    pub found_index: usize,
}
```

然后在 `ClusterCoverage` 下新增：

- `declared_entries: Vec<DeclaredClusterRef>`
- `found_entries: Vec<FoundClusterRef>`
- `matches_detailed: Vec<ClusterCoverageMatch>`

保留旧字段：

- `declared`
- `found`
- `matched`
- `declared_missing`
- `found_extra`

这样旧调用方不需要同步大改。

**Step 4: 跑编译验证**

Run: `cargo test crossref::tests::cluster_coverage_matches_declared_and_found -- --nocapture`

Expected: 先编译失败或测试失败，再进入下一步实现。

**Step 5: Commit**

```bash
git add src/model.rs
git commit -m "feat(crossref): add provenance models for cluster coverage"
```

### Task 2: 在 `crossref.rs` 中填充 declared/found/match provenance

**Files:**
- Modify: `src/crossref.rs`
- Test: `src/crossref.rs`

**Step 1: 实现 declared side**

在 `build_cluster_coverage(doc)` 中：

- 从 `doc.psm_cluster_table.entries` 构造 `declared_entries`
- 顺序保持与 `PSMclustertable` 原始顺序一致
- 同时继续生成旧的 `declared: Vec<String>`

declared entry 映射规则：

- `name <- PsmClusterEntry.name`
- `record_offset <- PsmClusterEntry.record_offset`
- `name_offset <- PsmClusterEntry.name_offset`
- `record_len <- PsmClusterEntry.record_len`

**Step 2: 实现 found side**

构造 `found_entries` 时不要只用 `BTreeSet<String>`。
改为：

- `doc.clusters` -> `FoundClusterRef { source_kind: PsmCluster, path: c.path }`
- `doc.sheet_streams` -> `FoundClusterRef { source_kind: SheetStream, path: s.path }`

然后为了兼容旧字段：

- `found: Vec<String>` 仍从 `found_entries.name` 去重得到

如果同名条目可能同时出现在 cluster 与 sheet 中，初版可保留第一条，或显式允许多条 found_entries；但 `found` 旧摘要字段仍保持唯一名字集合。

**Step 3: 实现 match relation**

新增 `matches_detailed`：

- 遍历 `declared_entries`
- 按 `name` 在 `found_entries` 中找第一条同名项
- 记录：
  - `name`
  - declared index
  - found index

同时保留旧 `matched: Vec<String>`

**Step 4: 跑测试**

Run: `cargo test crossref::tests::cluster_coverage_ -- --nocapture`

Expected: PASS

**Step 5: 增补回归断言**

更新已有测试 `cluster_coverage_matches_declared_and_found()`：

- `declared_entries.len()` 正确
- `found_entries` 有正确 source kind
- `matches_detailed.len()` 正确

**Step 6: Commit**

```bash
git add src/crossref.rs
git commit -m "feat(crossref): derive provenance-aware cluster coverage records"
```

### Task 3: 在 inspect report 中暴露 provenance-aware cluster coverage

**Files:**
- Modify: `src/inspect/report.rs`
- Test: `src/inspect/report.rs`

**Step 1: 写失败测试**

新增报告测试，断言 coverage/crossref section 里能看见 provenance 信息，例如：

- declared cluster count
- matched detailed count
- found source kind
- path

示例：

```rust
assert!(report.contains("declared entries:"));
assert!(report.contains("SheetStream"));
assert!(report.contains("/Sheet6"));
```

**Step 2: 跑测试确认失败**

Run: `cargo test inspect::report -- --nocapture`

Expected: FAIL，因为报告目前只有名字集合。

**Step 3: 最小实现**

在 `Cross Reference` section 中补一小段：

```text
  Cluster refs:
    declared entries: N
    found entries: N
    matched detailed: N
```

并在 sample lines 中输出前几条 provenance，例如：

```text
    decl PSMcluster0 @+0008 len=24
    found Sheet6 [SheetStream] /Sheet6
    match Sheet6 decl#3 -> found#1
```

要求：

- 不移除旧的 summary lines
- provenance 展示只做前几条 sample，避免报告过长

**Step 4: 跑测试**

Run: `cargo test inspect::report -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/inspect/report.rs
git commit -m "feat(report): show cluster provenance details in crossref"
```

### Task 4: 为真实样本增加 provenance consistency 检查

**Files:**
- Modify: `tests/parse_real_files.rs`

**Step 1: 写失败测试**

新增一个真实样本测试，例如：

```rust
#[test]
fn cluster_coverage_provenance_matches_psm_cluster_table_offsets() {}
```

断言：

- `cross_reference.cluster_coverage.declared_entries.len() == psm_cluster_table.entries.len()`
- 每个 declared entry 的：
  - `name`
  - `record_offset`
  - `name_offset`
  - `record_len`
    与 `psm_cluster_table.entries` 对应项完全一致
- `matches_detailed.len() == matched.len()`

**Step 2: 跑测试确认失败**

Run: `cargo test cluster_coverage_provenance_matches_psm_cluster_table_offsets -- --nocapture`

Expected: FAIL 直到 provenance 导出接好。

**Step 3: 最小修正**

只修 crossref 导出，不要在这个任务扩散到 layout/import_view。

**Step 4: 跑测试**

Run: `cargo test cluster_coverage_provenance_matches_psm_cluster_table_offsets -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add tests/parse_real_files.rs
git commit -m "test(crossref): verify cluster provenance against parsed PSM table"
```

### Task 5: 全量验证

**Files:**
- No code changes expected

**Step 1: 运行定向测试**

Run: `cargo test crossref::tests::cluster_coverage_ -- --nocapture`

Expected: PASS

**Step 2: 运行 report 测试**

Run: `cargo test inspect::report -- --nocapture`

Expected: PASS

**Step 3: 运行真实样本相关测试**

Run: `cargo test cluster_coverage_ -- --nocapture`

Expected: PASS（若 fixture 缺失，则应像项目现有风格一样安全 skip）

**Step 4: 运行全量测试**

Run: `cargo test --all-targets`

Expected: PASS；若仍有与 fixture 路径/环境相关的既有失败，明确记录，不把它归咎于本次变更。

**Step 5: Commit**

```bash
git add -A
git commit -m "chore(crossref): validate provenance-aware cluster coverage"
```

## 设计约束

### 为什么先只做 cluster/sheet provenance

这是当前最稳的一条链：

- `PSMclustertable` 已有 record 级结构
- `cluster_coverage` 已经存在
- `clusters` 与 `sheet_streams` 已是现成来源

它非常适合做 Phase 3 的第一块基石。

### 这一步不要做的事

- 不要在本步引入 object / relationship / endpoint 的统一规范化图
- 不要重构 `CrossReferenceGraph` 顶层结构
- 不要移除旧 `declared/found/matched` 摘要字段
- 不要让 `layout` / `import_view` 依赖新 provenance 字段

## 完成定义

本步完成后，应达到：

- `ClusterCoverage` 同时具备 summary + provenance records
- `cross_reference` 不再只保存 cluster 名称集合
- `inspect/report` 能展示 cluster 来源与匹配样例
- 真实样本测试能验证 provenance 没有漂移
