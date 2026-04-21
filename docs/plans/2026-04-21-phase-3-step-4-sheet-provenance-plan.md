# Phase 3 Step 4 Sheet Provenance Aggregation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 Phase 3 Step 1–3 建立的 provenance 结构之上，新增**以 SheetStream 为主轴**的聚合视图。过去所有 provenance 都是"以 relationship / object / cluster 为主轴"的正向视图；Step 4 补一条反向汇总，让每条 `SheetStream` 一眼看到：
- 它在 PSMclustertable 里是否被声明 / 匹配
- 它里面有多少条 `SheetEndpointRecord`
- 有多少条 relationship link 指向它
- 有多少 relationship link 经它完整追到 source/target object

方便识别"孤岛 Sheet"（有 endpoint records 但没被 PSM 声明）以及"空壳 Sheet"（被声明但无 endpoint records）这类漂移。

**Architecture:** 仍保持 `crossref` 纯派生语义，不改动前 3 步的数据结构。新字段都挂在 `CrossReferenceGraph` 上：

- `sheet_provenance: Vec<SheetProvenanceRef>`
- `sheet_provenance_coverage: SheetProvenanceCoverage`

数据源全是已构造好的子图（`ClusterCoverage.matches_detailed`、`relationship_endpoint_links`、`object_sources`、`doc.sheet_streams`），一次 O(N) 即可。

**Tech Stack:** Rust、现有 `model.rs` / `crossref.rs` / `inspect/report.rs`、真实样本测试、单元测试。

---

### Task 1: 定义 SheetProvenance 模型

**Files:**
- Modify: `src/model.rs`
- Test: `src/crossref.rs`

**Step 1: 写失败测试**

```rust
#[test]
fn sheet_provenance_aggregates_endpoint_and_relationship_counts() {}
```

至少断言：

- 每条 `SheetStream` 对应一条 `SheetProvenanceRef`
- Ref 暴露：`sheet_path`、`endpoint_record_count`、`declared_in_psm`、`matched_declared_index`、`linked_relationship_count`、`fully_traced_relationship_count`
- `SheetProvenanceCoverage` 暴露：`total_sheets`、`declared_sheets`、`orphan_sheets`、`sheets_with_endpoint_records`、`empty_declared_sheets`

**Step 2: 扩展 model**

```rust
pub struct SheetProvenanceRef {
    pub sheet_path: String,
    pub endpoint_record_count: usize,
    pub declared_in_psm: bool,
    pub matched_declared_index: Option<usize>,
    pub linked_relationship_count: usize,
    pub fully_traced_relationship_count: usize,
}

pub struct SheetProvenanceCoverage {
    pub total_sheets: usize,
    pub declared_sheets: usize,
    pub orphan_sheets: usize,
    pub sheets_with_endpoint_records: usize,
    pub empty_declared_sheets: usize,
}
```

在 `CrossReferenceGraph` 下新增（都 `#[serde(default)]`）。

**Step 3: 编译 / 测试预期**

Run: `cargo build --all-targets`

Expected: 编译通过；测试 FAIL。

**Step 4: Commit**

```bash
git add src/model.rs
git commit -m "feat(model): add sheet provenance aggregation types"
```

### Task 2: `build_sheet_provenance` 聚合

**Files:**
- Modify: `src/crossref.rs`

**Step 1: 实现**

新增 `fn build_sheet_provenance(doc: &PidDocument, graph: &CrossReferenceGraph) -> (Vec<SheetProvenanceRef>, SheetProvenanceCoverage)`：

- 预先按 sheet_path 索引 `relationship_endpoint_links`
- 预先按 sheet name 索引 `cluster_coverage.matches_detailed`（仅对 source_kind = SheetStream 的 match）
- 遍历 `doc.sheet_streams`，每条产出 SheetProvenanceRef
- coverage 自然统计

规则：

- `linked_relationship_count` = 指向本 sheet_path 的 relationship_endpoint_links 数
- `fully_traced_relationship_count` = 其中同时在 `source_drawing_id` / `target_drawing_id` 都被 `object_sources` 索引解析的条数
- `matched_declared_index` = `cluster_coverage.matches_detailed[i].declared_index`，当 `found_entries[match.found_index].path == sheet_path`
- `declared_in_psm = matched_declared_index.is_some()`
- `orphan_sheets = total_sheets - declared_sheets`
- `empty_declared_sheets` = declared but `endpoint_record_count == 0`

**Step 2: 串进 build_graph**

在 `build_graph` 末尾（Step 3 的 provenance_chain 之后）调用 `build_sheet_provenance`，赋值到新字段。

**Step 3: 跑测试**

Run: `cargo test crossref::tests::sheet_provenance -- --nocapture`

Expected: PASS

**Step 4: 增补小场景测试**

- 没有 sheet_streams → 空 vec，coverage 全零
- 有 declared 但 endpoint_records = 0 → empty_declared_sheets 命中
- 有 sheet_streams 但无 PSM → orphan_sheets 等于 total

**Step 5: Commit**

```bash
git add src/crossref.rs
git commit -m "feat(crossref): aggregate sheet-level provenance"
```

### Task 3: report 展示 sheet provenance

**Files:**
- Modify: `src/inspect/report.rs`
- Test: `src/inspect/report.rs`

**Step 1: 写失败测试**

```rust
#[test]
fn report_shows_sheet_provenance_summary() {}
```

断言输出含：

- `Sheet provenance:` 小节
- `total=N declared=N orphan=N with_endpoints=N empty_declared=N`
- `Sheet provenance refs:`
- 某条 `/Sheet6` 样例

**Step 2: 实现**

在 `Cross Reference` section `Provenance chain:` 之后追加：

```text
  Sheet provenance: total=N declared=N orphan=N with_endpoints=N empty_declared=N
  Sheet provenance refs:
    /Sheet6 endpoint_records=N declared=true match_index=N relationships=N fully_traced=N
```

仅前 5 条样本。

**Step 3: 跑测试**

Run: `cargo test inspect::report::tests::report_shows_sheet_provenance -- --nocapture`

Expected: PASS

**Step 4: Commit**

```bash
git add src/inspect/report.rs
git commit -m "feat(report): surface sheet provenance aggregation"
```

### Task 4: 真实样本回归

**Files:**
- Modify: `tests/parse_real_files.rs`

**Step 1: 写测试**

```rust
#[test]
fn sheet_provenance_matches_sheet_streams() {}
```

断言：

- `sheet_provenance.len() == doc.sheet_streams.len()`
- 每条 ref 的 `sheet_path` 与 `doc.sheet_streams[i].path` 对齐
- `endpoint_record_count == sheet_streams[i].endpoint_records.len()`
- `linked_relationship_count == cross.relationship_endpoint_links.iter().filter(|l| l.sheet_path.as_deref() == Some(path)).count()`
- coverage.total_sheets == doc.sheet_streams.len()

**Step 2: 运行**

Run: `cargo test sheet_provenance_matches_sheet_streams -- --nocapture`

Expected: PASS（缺 fixture 则 skip）

**Step 3: Commit**

```bash
git add tests/parse_real_files.rs
git commit -m "test(crossref): verify sheet provenance aggregation against fixture"
```

### Task 5: 全量验证

**Files:**
- No code changes expected

**Step 1: 定向单测**

Run: `cargo test crossref::tests::sheet_provenance -- --nocapture`

Expected: PASS

**Step 2: report 定向**

Run: `cargo test inspect::report::tests::report_shows_sheet_provenance -- --nocapture`

Expected: PASS

**Step 3: 全量**

Run:

```bash
cargo test --all-targets
cargo fmt --check
```

Expected: 全绿。

**Step 4: Commit（若有增量）**

```bash
git add -A
git commit -m "chore(crossref): validate sheet provenance phase 3 step 4"
```

## 设计约束

### 为什么现在做 sheet 聚合

Step 1–3 的 provenance 都是以 relationship / object / cluster 为主轴。Sheet 作为端点记录的物理载体，缺一条专门视角：
- 某条 Sheet 可能没有被 PSM 声明但仍持有端点数据（隐藏流）
- 某条 Sheet 被声明但内部 0 端点（空壳）
- 某条 Sheet 被声明、有端点，但没任何 relationship link 指向它（冗余声明）

这条视图能在一行 report 里直接看到 fixture-drift 的物理分布。

### 这一步不要做的事

- 不要修改前 3 步数据结构
- 不要新增解析逻辑（不碰 `/Sheet*` 二进制）
- 不要让 `layout` / `import_view` 依赖新字段
- 不要试图重绘 Step 3 的 provenance chain；Step 4 只补 Sheet 维度的聚合

### SemVer

纯 additive → minor bump 候选。

## 完成定义

本步完成后，应达到：

- `CrossReferenceGraph` 含 `sheet_provenance` + `sheet_provenance_coverage`
- `inspect report` 在 Cross Reference section 展示 sheet provenance 总览与样例
- 真实样本断言 `sheet_provenance.len() == doc.sheet_streams.len()` 与计数对齐
- 全量测试持续绿

后续：如 fixture 稳定，可在 Phase 11c 把 Sheet 几何解码结果回注到 `SheetProvenanceRef`（e.g. `graphic_count`, `labeled_text_count`）。
