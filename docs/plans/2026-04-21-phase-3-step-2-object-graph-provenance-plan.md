# Phase 3 Step 2 Object Graph Provenance Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 Phase 3 Step 1（cluster/sheet/symbol/attribute-class provenance）之上，把 provenance 链接向下游延伸到 `ObjectGraph`，让 `PidObject` / `PidRelationship` 与 `SheetEndpointRecord` / `DynamicAttributesBlob.attribute_records` 之间有**结构化的来源关系**，为 Phase 12a 的统一 NormalizedObject 模型打基础。

**Architecture:** 保持 `crossref::build_graph` 纯派生语义；不改动 `PidObject` / `PidRelationship` / `SheetEndpointRecord` 等原始模型的公有字段。所有 provenance 信息作为新的 `CrossReferenceGraph` 子字段存在：

- `relationship_endpoint_links: Vec<RelationshipEndpointLink>` — 由每条 `PidRelationship` 经 `field_x` 映射到 `SheetEndpointRecord`，兼带源/目标 `drawing_id` 是否已解析的状态
- `object_sources: Vec<ObjectSourceRef>` — 由每条 `PidObject.drawing_id` 映射到其 DA `attribute_records` 里的原始 provenance（class_name / record_index / drawing_ids / model_ids）

保留已有 `EndpointResolutionStats` 摘要字段。增加：

- `relationship_endpoint_coverage: EndpointLinkCoverage`
- `object_source_coverage: ObjectSourceCoverage`

以匹配 Step 1 的「summary + records」双轨风格。

**Tech Stack:** Rust、`src/model.rs` / `src/crossref.rs` / `src/inspect/report.rs`、单元测试 + `tests/parse_real_files.rs`

---

### Task 1: 定义 relationship endpoint provenance 模型

**Files:**
- Modify: `src/model.rs`
- Test: `src/crossref.rs`

**Step 1: 写失败测试**

在 `src/crossref.rs` 的测试模块里新增目标测试：

```rust
#[test]
fn relationship_endpoint_links_record_sheet_provenance() {}
```

至少断言：

- 每个 `PidRelationship` 有对应的 `RelationshipEndpointLink`
- link 里能取到：
  - `relationship_guid`
  - `relationship_record_id`
  - `rel_field_x`
  - `sheet_path`
  - `sheet_offset`
  - 源/目标端点的 `field_x`
  - 源/目标端点是否已解析为 `drawing_id`
- 对于 Sheet 里缺失对应 endpoint 的 relationship，link 应标记为 `missing_sheet_record = true`

**Step 2: 跑测试确认失败**

Run: `cargo test crossref::tests::relationship_endpoint_links_ -- --nocapture`

Expected: FAIL，因为 `CrossReferenceGraph` 还没有 `relationship_endpoint_links`。

**Step 3: 扩展模型**

在 `src/model.rs` 中新增：

```rust
pub struct RelationshipEndpointLink {
    pub relationship_guid: String,
    pub relationship_record_id: Option<u32>,
    pub rel_field_x: Option<u32>,
    pub source_field_x: Option<u32>,
    pub target_field_x: Option<u32>,
    pub source_drawing_id: Option<String>,
    pub target_drawing_id: Option<String>,
    pub sheet_path: Option<String>,
    pub sheet_offset: Option<usize>,
    pub missing_sheet_record: bool,
}

pub struct EndpointLinkCoverage {
    pub total: usize,
    pub linked: usize,
    pub missing_field_x: usize,
    pub missing_sheet_record: usize,
    pub fully_resolved: usize,
    pub partially_resolved: usize,
}
```

然后在 `CrossReferenceGraph` 中新增：

- `relationship_endpoint_links: Vec<RelationshipEndpointLink>`
- `relationship_endpoint_coverage: EndpointLinkCoverage`

两字段都用 `#[serde(default, skip_serializing_if = "...")]` 以保向后兼容。

**Step 4: 跑编译验证**

Run: `cargo build --all-targets`

Expected: 编译通过；测试仍 FAIL（逻辑未填充）。

**Step 5: Commit**

```bash
git add src/model.rs
git commit -m "feat(model): add relationship endpoint provenance types"
```

### Task 2: 在 `crossref.rs` 中构造 relationship_endpoint_links

**Files:**
- Modify: `src/crossref.rs`
- Test: `src/crossref.rs`

**Step 1: 实现 build 函数**

新增 `fn build_relationship_endpoint_links(doc: &PidDocument) -> (Vec<RelationshipEndpointLink>, EndpointLinkCoverage)`：

- 扫描 `doc.object_graph.as_ref().map(|g| &g.relationships)`（若 `object_graph` 不在则返回空）
- 把所有 Sheet 的 `endpoint_records` 压平为 `BTreeMap<rel_field_x, &SheetEndpointRecord>`（同一 rel_field_x 命中多条时保留第一条并记录 `duplicate_sheet_record = true`（可不加、后续再补）
- 对每条 relationship：
  - 若 `field_x` 为 `None`：link 里的 sheet 字段留 `None`，计入 `missing_field_x`
  - 若在 map 里没命中：`missing_sheet_record = true`，计入 `missing_sheet_record`
  - 否则填入 `sheet_path` / `sheet_offset` / `source_field_x` / `target_field_x`
  - `source_drawing_id` / `target_drawing_id` 直接复用 relationship 已有字段

- coverage 规则：
  - `total = relationships.len()`
  - `linked = 命中 sheet_record 的条数`
  - `fully_resolved = 两端 drawing_id 都 Some`
  - `partially_resolved = 有一端 Some`

**Step 2: 串进入口 `build_graph`**

在 `build_graph` 里把返回的 links / coverage 塞进 `CrossReferenceGraph`。

**Step 3: 跑测试**

Run: `cargo test crossref::tests::relationship_endpoint_links_ -- --nocapture`

Expected: PASS

**Step 4: 增补小场景单测**

覆盖：

- 没有 object_graph 时，`relationship_endpoint_links` 返回空、coverage 全零
- `field_x = None` 的 relationship 正确落到 `missing_field_x` 桶
- Sheet 里没有对应 record 的 relationship 正确落到 `missing_sheet_record` 桶

**Step 5: Commit**

```bash
git add src/crossref.rs
git commit -m "feat(crossref): link relationships to sheet endpoint provenance"
```

### Task 3: object ↔ DA record 反向链

**Files:**
- Modify: `src/model.rs`
- Modify: `src/crossref.rs`
- Test: `src/crossref.rs`

**Step 1: 写失败测试**

```rust
#[test]
fn object_sources_record_da_provenance() {}
```

至少断言：

- 每个 `PidObject` 对应一条 `ObjectSourceRef`
- `ObjectSourceRef` 里包含：
  - `drawing_id`
  - `class_name`
  - `attribute_record_index`（DA `attribute_records` 的下标）
  - `confidence`
  - `has_trailer_record_id` (bool)

**Step 2: 扩展模型**

```rust
pub struct ObjectSourceRef {
    pub drawing_id: String,
    pub class_name: String,
    pub attribute_record_index: usize,
    pub confidence: String,
    pub has_trailer_record_id: bool,
}

pub struct ObjectSourceCoverage {
    pub total_objects: usize,
    pub linked: usize,
    pub missing_da_record: usize,
}
```

并在 `CrossReferenceGraph` 中：

- `object_sources: Vec<ObjectSourceRef>`
- `object_source_coverage: ObjectSourceCoverage`

**Step 3: 实现 `build_object_sources`**

- 只要 `doc.dynamic_attributes` 和 `doc.object_graph` 都存在
- 扫 DA `attribute_records`，按 `DrawingID` 字段聚合为 `BTreeMap<drawing_id, (record_index, class_name, confidence, has_trailer_record_id)>`
  - `has_trailer_record_id = da.record_trailers.get(record_index).is_some() && record_trailer.record_id.is_some()`（若 `record_trailers` 是 `Vec<Option<Trailer>>`，按实际形状调整）
- 遍历 `object_graph.objects`，命中就生成 `ObjectSourceRef`，否则 `missing_da_record += 1`

**Step 4: 跑测试**

Run: `cargo test crossref::tests::object_sources_ -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/model.rs src/crossref.rs
git commit -m "feat(crossref): derive object sources from DA attribute records"
```

### Task 4: inspect report 展示 provenance 样例

**Files:**
- Modify: `src/inspect/report.rs`
- Test: `src/inspect/report.rs`

**Step 1: 写失败测试**

新增 2 个测试：

```rust
#[test]
fn report_shows_relationship_endpoint_links() {}

#[test]
fn report_shows_object_source_links() {}
```

断言输出里含：

- `Relationship endpoint refs:` 小节
- 某条 `Sheet path` + `field_x` 样例
- `Object source refs:` 小节
- 某条 `class_name` + `rec#`

**Step 2: 最小实现**

在 `Cross Reference` section 末尾追加：

```text
  Relationship endpoint refs:
    total: N, linked: N, missing field_x: N, missing sheet record: N
    rel <guid> rel_fx=<fx> sheet=<path>@<off> src=<id_or_->-> tgt=<id_or_->
  Object source refs:
    total: N, linked: N, missing DA record: N
    <drawing_id> class=<class_name> rec#<idx> conf=<confidence>
```

同样只展示前 3 条样例。

**Step 3: 跑测试 / 更新已有 `generate_report` smoke test 使其仍通过**

Run: `cargo test inspect::report -- --nocapture`

Expected: PASS。

**Step 4: Commit**

```bash
git add src/inspect/report.rs
git commit -m "feat(report): surface relationship & object provenance links"
```

### Task 5: 真实样本回归

**Files:**
- Modify: `tests/parse_real_files.rs`

**Step 1: 新测试**

```rust
#[test]
fn relationship_endpoint_links_match_sheet_records() {}

#[test]
fn object_sources_align_with_attribute_records() {}
```

断言：

- `relationship_endpoint_links.len() == object_graph.relationships.len()`
- 所有 `missing_field_x` + `missing_sheet_record` + `linked` = `total`
- 至少存在一条 `sheet_offset.is_some()` 的 link（fixture 里已知有端点）
- 每条 `object_sources.drawing_id` 在 DA `attribute_records` 里能找到同名 DrawingID
- `object_source_coverage.total_objects == object_graph.objects.len()`

**Step 2: 运行并修漂移**

Run: `cargo test relationship_endpoint_links_match_sheet_records object_sources_align_with_attribute_records -- --nocapture`

Expected: PASS（若 fixture 缺失则 skip，与项目惯例一致）。

**Step 3: Commit**

```bash
git add tests/parse_real_files.rs
git commit -m "test(crossref): verify relationship/object provenance against fixture"
```

### Task 6: 全量验证 + 清理

**Files:**
- No code changes expected

**Step 1: 定向 unit 测试**

Run: `cargo test crossref::tests::relationship_endpoint_links_ crossref::tests::object_sources_ -- --nocapture`

Expected: PASS

**Step 2: report 定向测试**

Run: `cargo test inspect::report::tests::report_shows -- --nocapture`

Expected: PASS

**Step 3: 真实样本**

Run: `cargo test --test parse_real_files`

Expected: PASS

**Step 4: 全量 + 卫生**

Run:

```bash
cargo test --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: 全绿。

**Step 5: Commit**

```bash
git add -A
git commit -m "chore(crossref): validate phase 3 step 2 provenance"
```

## 设计约束

### 为什么选 relationship / object 而不是直接 Phase 12a

当前 `ObjectGraph` 的 `PidObject.field_x` + `PidRelationship.field_x` 已经给了我们一条"字段到 DA/Sheet 的潜在桥梁"，但没有显式表达。我们只缺一个派生层把它暴露出来。这是成本最低、信息增益最高的一步。

全面的 Phase 12a 规范化（NormalizedObject / NormalizedRelationship / 统一 Provenance）需要整份设计稿与 API 评审，本步刻意**不启动**，避免陷入大重构。

### 这一步不要做的事

- 不要引入 `NormalizedObject` / `NormalizedRelationship`
- 不要修改 `PidObject` / `PidRelationship` / `SheetEndpointRecord` 的公有字段
- 不要让 `layout` / `import_view` 依赖新 provenance 字段
- 不要在本轮处理 Dynamic Attributes 里的非 relationship attribute 到 SheetEndpoint 的解码回流（目前 fixture 行为不稳定）

### SemVer

纯 additive（新字段 + `#[serde(default, skip_serializing_if)]`）→ minor bump 候选（由 release 时统一决定）。

## 完成定义

本步完成后，应达到：

- `CrossReferenceGraph` 含 `relationship_endpoint_links` / `relationship_endpoint_coverage` / `object_sources` / `object_source_coverage`
- `inspect report` 在 Cross Reference section 能显示端点 & 对象 provenance 样例
- 真实样本回归：`relationship_endpoint_links.len() == relationships.len()`、`object_sources.len() == objects.len()`
- 全量 332+ 测试持续绿

后续：Phase 3 Step 3 可考虑把上述 links 汇总成一条「端点解析通路一致性」诊断（cluster → sheet → endpoint record → DA record → object），为 Phase 12a 设计稿采样。
