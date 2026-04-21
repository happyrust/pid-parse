# Phase 3 Step 3 Provenance Chain Diagnostic Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 Phase 3 Step 1 / Step 2 已经落地的分散 provenance 记录之上，新增一条「端到端链路一致性」诊断：给定 `CrossReferenceGraph`，对每条 `PidRelationship` 沿着 `cluster → sheet → endpoint_record → DA record (source/target) → PidObject` 这条链逐跳检查，报告哪些链是完整的、哪些在哪一跳断裂。

**Architecture:** 保持 `crossref` 纯派生语义。仅新增一个小型诊断层，不改动 Step 1 / Step 2 的 provenance 数据结构。在 `CrossReferenceGraph` 下增加：

- `provenance_chain_coverage: ProvenanceChainCoverage` —— 逐跳通过数统计
- `provenance_chain_breaks: Vec<ProvenanceChainBreak>` —— 前若干条断裂样例，便于 debug

这是**读不写**的视图——链断裂不会触发异常，只记录。上层 CLI（`pid_inspect`）把 coverage 单行显示 + 若干 break 样例。

**Tech Stack:** Rust、现有 `model.rs` / `crossref.rs` / `inspect/report.rs`、真实样本测试、单元测试。

---

### Task 1: 定义 ProvenanceChain 模型

**Files:**
- Modify: `src/model.rs`
- Test: `src/crossref.rs`

**Step 1: 写失败测试**

在 `src/crossref.rs` 测试模块里新增：

```rust
#[test]
fn provenance_chain_coverage_counts_each_hop() {}

#[test]
fn provenance_chain_breaks_point_at_first_failed_hop() {}
```

至少断言：

- `ProvenanceChainCoverage` 暴露 `total_relationships`、`has_field_x`、`sheet_linked`、`source_object_linked`、`target_object_linked`、`fully_traced`
- `ProvenanceChainBreak` 暴露 `relationship_guid`、`stage`（枚举）、`reason`（短描述）
- `stage` 取值至少包含 `MissingFieldX`、`MissingSheetRecord`、`SourceObjectUnlinked`、`TargetObjectUnlinked`

**Step 2: 扩展 model**

```rust
pub struct ProvenanceChainCoverage {
    pub total_relationships: usize,
    pub has_field_x: usize,
    pub sheet_linked: usize,
    pub source_object_linked: usize,
    pub target_object_linked: usize,
    pub fully_traced: usize,
}

pub enum ProvenanceChainStage {
    MissingFieldX,
    MissingSheetRecord,
    SourceObjectUnlinked,
    TargetObjectUnlinked,
}

pub struct ProvenanceChainBreak {
    pub relationship_guid: String,
    pub stage: ProvenanceChainStage,
    pub reason: String,
}
```

并在 `CrossReferenceGraph` 中新增两字段（都 `#[serde(default)]`）：

- `provenance_chain_coverage: ProvenanceChainCoverage`
- `provenance_chain_breaks: Vec<ProvenanceChainBreak>`

**Step 3: 跑测试确认编译 FAIL → 模型就位后仍 FAIL（逻辑未填）**

Run: `cargo build --all-targets`

Expected: 编译通过；测试 FAIL（coverage 全零）。

**Step 4: Commit**

```bash
git add src/model.rs
git commit -m "feat(model): add provenance chain diagnostic types"
```

### Task 2: `build_provenance_chain` 遍历 + 分类

**Files:**
- Modify: `src/crossref.rs`

**Step 1: 实现 build 函数**

新增 `fn build_provenance_chain(graph: &CrossReferenceGraph) -> (ProvenanceChainCoverage, Vec<ProvenanceChainBreak>)`。

输入只读已经构造好的 `CrossReferenceGraph`（含 `relationship_endpoint_links` + `object_sources`）。输出一对摘要 + 前 10 条断裂样例。

实现要点：

- 构造 `object_source_index: BTreeMap<&str, &ObjectSourceRef>` on `drawing_id`
- 遍历 `relationship_endpoint_links`，逐跳统计：
  - `rel_field_x.is_some()` → `has_field_x += 1`，否则记 `MissingFieldX` 断裂
  - `sheet_path.is_some()` → `sheet_linked += 1`，否则记 `MissingSheetRecord`
  - 解析 `source_drawing_id` 到 `object_sources` → `source_object_linked += 1`，否则记 `SourceObjectUnlinked`
  - 同理 target
- 若 4 跳都通过 → `fully_traced += 1`
- 断裂记录按 first-fail stage 归类；只保留前 10 条

**Step 2: 串进 `build_graph`**

```rust
let (provenance_chain_coverage, provenance_chain_breaks) =
    build_provenance_chain_after_all_other_sections_ready();
```

注意：本函数依赖 `relationship_endpoint_links` 与 `object_sources`，所以要**在这两者产出后**再调用。最简单就是构造 `CrossReferenceGraph` 时先塞进去，然后再调用一个修正函数填充新字段。

**Step 3: 跑测试**

Run: `cargo test crossref::tests::provenance_chain_ -- --nocapture`

Expected: PASS

**Step 4: 增补小场景单测**

覆盖：

- 无 object_graph → coverage 全零、breaks 空
- 所有 relationship 都 fully_traced → breaks 空
- 混合场景（含 4 种 stage 各一条）→ breaks 里按顺序出现

**Step 5: Commit**

```bash
git add src/crossref.rs
git commit -m "feat(crossref): derive provenance chain diagnostic"
```

### Task 3: inspect report 展示链诊断

**Files:**
- Modify: `src/inspect/report.rs`
- Test: `src/inspect/report.rs`

**Step 1: 写失败测试**

新增：

```rust
#[test]
fn report_shows_provenance_chain_summary_and_breaks() {}
```

断言输出含：

- `Provenance chain:` 小节
- `total=N has_field_x=N sheet_linked=N source_object_linked=N target_object_linked=N fully_traced=N`
- `Provenance chain breaks:`
- 某条断裂含 stage 名字

**Step 2: 最小实现**

在 `Cross Reference` section 末尾（`Object sources:` 之后）追加：

```text
  Provenance chain: total=N has_field_x=N sheet_linked=N source_object_linked=N target_object_linked=N fully_traced=N
  Provenance chain breaks:
    <guid> stage=<Stage> reason=<reason>
```

仅显示前 5 条 breaks。

**Step 3: 跑测试**

Run: `cargo test inspect::report -- --nocapture`

Expected: PASS

**Step 4: Commit**

```bash
git add src/inspect/report.rs
git commit -m "feat(report): surface provenance chain diagnostic"
```

### Task 4: 真实样本回归

**Files:**
- Modify: `tests/parse_real_files.rs`

**Step 1: 新测试**

```rust
#[test]
fn provenance_chain_matches_relationship_and_object_counts() {}
```

断言：

- `provenance_chain_coverage.total_relationships == object_graph.relationships.len()`
- `has_field_x == Σ(rel.field_x.is_some())`
- `sheet_linked == Σ(link.sheet_path.is_some())`
- `fully_traced ≤ min(sheet_linked, source_object_linked, target_object_linked)`
- `provenance_chain_breaks.len() ≤ 10`
- 每条 break 的 `relationship_guid` 在 `relationship_endpoint_links` 中存在

**Step 2: 运行并修漂移**

Run: `cargo test provenance_chain_matches_relationship_and_object_counts -- --nocapture`

Expected: PASS（缺 fixture 则 skip）

**Step 3: Commit**

```bash
git add tests/parse_real_files.rs
git commit -m "test(crossref): verify provenance chain against fixture"
```

### Task 5: 全量验证

**Files:**
- No code changes expected

**Step 1: 定向 unit**

Run: `cargo test crossref::tests::provenance_chain_ -- --nocapture`

Expected: PASS

**Step 2: report**

Run: `cargo test inspect::report -- --nocapture`

Expected: PASS

**Step 3: 真实样本**

Run: `cargo test --test parse_real_files`

Expected: PASS

**Step 4: 全量 + 卫生**

Run:

```bash
cargo test --all-targets
cargo fmt --check
```

若 clippy 可用：

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: 全绿。

**Step 5: Commit（仅当有 fmt/fix 增量时）**

```bash
git add -A
git commit -m "chore(crossref): validate phase 3 step 3 provenance chain"
```

## 设计约束

### 为什么现在做诊断而不是下一步直接进 Phase 12a

Step 1 + 2 产出的数据已经足够把一条 relationship 的完整来源链画出来。与其直接跳到 Phase 12a 的大重构（NormalizedObject / NormalizedRelationship 统一模型），不如先把「链是不是完整的」可观测出来，这样：

1. 真实样本回归可以持续守住 `fully_traced` 比例，防止下游变更引入漂移。
2. 如果观察到特定 stage 的断裂比例高，Phase 12a 的模型重构就能带着问题去做（而不是凭空设计）。
3. 代价很低：无新解析、无模型重构，只是在已有派生数据上加一个"读出断裂"的视图。

### 这一步不要做的事

- 不要修改 Step 1 / Step 2 的 provenance 数据结构
- 不要引入 NormalizedObject 层
- 不要在本轮自动修复断裂（只观测，不治理）
- 不要让 `layout` / `import_view` 依赖新字段

### SemVer

纯 additive → minor bump 候选，由 release 时统一决定。

## 完成定义

本步完成后，应达到：

- `CrossReferenceGraph` 含 `provenance_chain_coverage` + `provenance_chain_breaks`
- `inspect report` 在 Cross Reference section 能显示链总览 + 断裂样例
- 真实样本断言 `total_relationships` 与 object graph 对齐，`fully_traced` 不退化
- 全量测试持续绿

后续：Phase 3 Step 4 可考虑把"clusters → found_entries → sheet endpoint_records"这条补完，即 sheet stream 到 cluster path 的反向诊断（对发现缺失的 Sheet* 流有价值）。再之后才考虑正式进入 Phase 12a。
