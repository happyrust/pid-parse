# SPPID Coverage Inventory Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 `pid-parse` 增加一套可落地的 SPPID 解析覆盖清单/报告机制，让工程能明确区分“已识别、部分解析、完全解析、未知”。

**Architecture:** 在 `inspect` 层新增 coverage 生成模块，基于 `PidDocument`、顶层已知流常量、现有 parser/model 状态生成结构化 `CoverageReport`。先把 coverage 作为只读诊断面接入 `pid_inspect` 文本报告，不改动底层 parser 行为；后续再让它驱动更深层 parser 优先级和回归验证。

**Tech Stack:** Rust、现有 `pid_parse` 模型层、`inspect` 报告层、`pid_inspect` CLI、项目内单元测试

---

### Task 1: 定义 coverage 模型

**Files:**
- Modify: `src/model.rs`
- Test: `src/inspect/mod.rs`（后续分类测试会依赖这些类型）

**Step 1: 写出要新增的模型**

在 `src/model.rs` 中新增以下类型，放在 `UnknownStream` 附近，避免与 parser 结构体分散：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ParseCoverageStatus {
    FullyDecoded,
    PartiallyDecoded,
    IdentifiedOnly,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CoverageNodeKind {
    TopLevelStream,
    TopLevelStorage,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoverageEntry {
    pub name: String,
    pub kind: CoverageNodeKind,
    pub status: ParseCoverageStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct CoverageReport {
    pub entries: Vec<CoverageEntry>,
}
```

**Step 2: 运行格式/编译验证**

Run: `cargo test --lib model -- --nocapture`

Expected: 如果命令过滤不到测试，至少应编译通过；若失败，通常是 derive/import 缺失。

**Step 3: 最小修正 imports**

如果 `Serialize` / `Deserialize` / `JsonSchema` 作用域已在 `model.rs` 顶部可用，则不额外改；若编译报错，仅补最小 import。

**Step 4: 再跑验证**

Run: `cargo test --lib model -- --nocapture`

Expected: 编译通过。

**Step 5: Commit**

```bash
git add src/model.rs
git commit -m "feat(inspect): add coverage report model types"
```

### Task 2: 在 inspect 层实现 coverage 分类逻辑

**Files:**
- Create: `src/inspect/coverage.rs`
- Modify: `src/inspect/mod.rs`
- Test: `src/inspect/mod.rs`

**Step 1: 写失败测试，锁定分类行为**

在 `src/inspect/mod.rs` 的测试模块里补 3 个测试：

```rust
#[test]
fn coverage_marks_known_top_level_streams_with_expected_status() {}

#[test]
fn coverage_marks_known_storage_prefixes_as_identified() {}

#[test]
fn coverage_marks_unknown_top_level_entries_as_unknown() {}
```

测试场景至少覆盖：

- `DocVersion3` -> `FullyDecoded`
- `PSMsegmenttable` -> `PartiallyDecoded`
- `AppObject` -> `FullyDecoded`
- `Sheet1/Foo` -> 顶层 `Sheet1` 应被归类为 `IdentifiedOnly`
- `TaggedTxtData/Drawing` -> 顶层 `TaggedTxtData` 应被归类为 `IdentifiedOnly`
- `GhostStream` -> `Unknown`

测试里直接构造 `PidDocument { streams: ... , ..Default::default() }`。

**Step 2: 跑测试确认失败**

Run: `cargo test inspect:: -- --nocapture`

Expected: FAIL，提示 `coverage_report` 或相关 API 不存在。

**Step 3: 实现最小逻辑**

新建 `src/inspect/coverage.rs`，提供以下 API：

```rust
pub fn coverage_report(doc: &PidDocument) -> CoverageReport
pub fn top_level_coverage_entries(doc: &PidDocument) -> Vec<CoverageEntry>
```

实现规则：

- 遍历 `doc.streams`
- 将 `"/Foo"` 与 `"Foo/Bar"` 统一归一到顶层名 `Foo`
- 顶层名去重
- 命中 `KNOWN_TOP_LEVEL_STREAM_NAMES` 时，按硬编码映射给状态：
  - `\u{5}SummaryInformation` -> `FullyDecoded`
  - `\u{5}DocumentSummaryInformation` -> `FullyDecoded`
  - `PSMroots` -> `FullyDecoded`
  - `PSMclustertable` -> `PartiallyDecoded`
  - `PSMsegmenttable` -> `PartiallyDecoded`
  - `DocVersion2` -> `FullyDecoded`
  - `DocVersion3` -> `FullyDecoded`
  - `AppObject` -> `FullyDecoded`
  - `JTaggedTxtStgList` -> `FullyDecoded`
  - `PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` / `Unclustered Dynamic Attributes` -> `PartiallyDecoded`
- 命中 `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 时，返回 `IdentifiedOnly`
- 其余返回 `Unknown`

同时填充 `parser`、`document_field`、`note`，先用静态字符串，不做动态推断。

**Step 4: 在 `inspect/mod.rs` 暴露模块**

加：

```rust
pub mod coverage;
```

并在 `mod.rs` 中保留 `unidentified_top_level_streams`，不要删；Phase 1 目标是增强，不是替换。

**Step 5: 跑测试**

Run: `cargo test inspect:: -- --nocapture`

Expected: 新增测试 PASS；若旧测试依赖 `KNOWN_TOP_LEVEL_STREAM_NAMES.to_vec()` 等写法，按当前代码风格修到通过。

**Step 6: Commit**

```bash
git add src/inspect/mod.rs src/inspect/coverage.rs
git commit -m "feat(inspect): classify top-level coverage states"
```

### Task 3: 把 coverage 接入文本报告

**Files:**
- Modify: `src/inspect/report.rs`
- Test: `src/inspect/report.rs` 或 `src/inspect/mod.rs`

**Step 1: 写失败测试**

新增一个报告测试，至少断言：

- 报告包含 `--- Coverage ---`
- 包含 `Fully decoded`
- 包含 `Partially decoded`
- 包含 `Unknown`

示例断言：

```rust
let report = generate_report(&doc);
assert!(report.contains("--- Coverage ---"));
assert!(report.contains("DocVersion3"));
assert!(report.contains("PSMsegmenttable"));
assert!(report.contains("GhostStream"));
```

**Step 2: 跑测试确认失败**

Run: `cargo test inspect::report -- --nocapture`

Expected: FAIL，因为报告尚未输出 coverage section。

**Step 3: 最小实现**

在 `generate_report` 中、`Top-level Unidentified Streams` 之前插入：

- 调用 `crate::inspect::coverage::coverage_report(doc)`
- 输出 section：

```text
--- Coverage ---
  Fully decoded: N
  Partially decoded: N
  Identified only: N
  Unknown: N
```

然后按状态分组或逐项列出，例如：

```text
  [FULL] DocVersion3 -> version_history
  [PART] PSMsegmenttable -> psm_segment_table
  [ID]   Sheet1
  [UNK]  GhostStream
```

要求：

- 输出稳定排序（按 `name` 排）
- 不删现有 `Top-level Unidentified Streams`，两者可以并存

**Step 4: 跑测试**

Run: `cargo test inspect::report -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/inspect/report.rs
git commit -m "feat(report): add coverage summary section"
```

### Task 4: 为 CLI 增加单独 coverage 开关

**Files:**
- Modify: `src/bin/pid_inspect.rs`
- Test: `tests/` 下新增 CLI 测试，或补已有 CLI 测试文件

**Step 1: 写失败测试**

如果项目已有 `pid_inspect` CLI 测试文件，优先复用；否则创建最小 CLI 测试，断言：

- `pid_inspect sample.pid --coverage`
- 输出中包含 `--- Coverage ---`
- 不要求 JSON，仅文本模式可用

如果当前没有便捷 CLI fixture，可先只测参数分支函数或抽取布尔判断函数。

**Step 2: 跑测试确认失败**

Run: `cargo test --all-targets coverage -- --nocapture`

Expected: FAIL，因为 `--coverage` 尚未支持。

**Step 3: 实现最小支持**

在 `src/bin/pid_inspect.rs` 中：

- usage 文案增加 `--coverage`
- 增加 `let coverage = args.iter().any(|a| a == "--coverage");`
- 行为规则：
  - 若 `--coverage` 单独使用，则打印 coverage section（可只打印 coverage，不打印整份 report）
  - 若无其他 probe/graph/crossref 标志，则仍可继续打印完整 report；为了最小改动，推荐：
    - `--coverage` 时打印完整 report，因为 coverage 已嵌入 report
  - 更好一点：如果 `--coverage` 且没有其他开关，直接打印 coverage-only 文本函数

YAGNI 推荐：先只把它作为一个显式开关，走现有 report 输出。

**Step 4: 跑测试**

Run: `cargo test --all-targets coverage -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add src/bin/pid_inspect.rs tests
git commit -m "feat(cli): expose coverage inspection flag"
```

### Task 5: 增加真实样本/构造样本回归测试

**Files:**
- Modify: `src/inspect/mod.rs`
- Modify/Create: 现有 inspect/report 相关测试文件

**Step 1: 写回归测试**

补充两类测试：

1. 构造样本：
   - 混合已知流、storage、未知流
   - 断言统计数准确
2. 空文档样本：
   - `PidDocument::default()`
   - coverage 为空或仅反映“无顶层项”，行为要固定

示例断言：

```rust
assert_eq!(full_count, 3);
assert_eq!(partial_count, 2);
assert_eq!(unknown_count, 1);
```

**Step 2: 跑测试**

Run: `cargo test inspect:: -- --nocapture`

Expected: PASS

**Step 3: Commit**

```bash
git add src/inspect/mod.rs src/inspect/report.rs
git commit -m "test(inspect): lock coverage inventory behavior"
```

### Task 6: 全量验证

**Files:**
- No code changes expected

**Step 1: 运行格式检查**

Run: `cargo fmt --all -- --check`

Expected: PASS

**Step 2: 运行单测/集成测试**

Run: `cargo test --all-targets`

Expected: PASS

**Step 3: 运行 clippy**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: PASS

**Step 4: 如果失败，最小修复后重跑**

常见修复点：

- 排序/clone 冗余
- 枚举 match 未覆盖
- 测试中 vec 构造风格
- 未使用字段或 helper

**Step 5: Commit**

```bash
git add -A
git commit -m "chore(inspect): validate coverage inventory implementation"
```

## 额外说明

### 初版状态映射建议

第一版不要追求“动态智能判断解析程度”，而是采用“已知顶层名 -> 明确状态”的静态映射。这样：

- 实现简单
- 测试稳定
- 报告可控
- 后续可以把静态映射逐步升级为“模型存在 + 字节消费率 + parser成熟度”的综合判定

### 明确哪些现在应标记为 `PartiallyDecoded`

基于当前代码，建议保守处理：

- `PSMclustertable`
- `PSMsegmenttable`
- `PSMcluster0`
- `StyleCluster`
- `Dynamic Attributes Metadata`
- `Unclustered Dynamic Attributes`

### 明确哪些现在应标记为 `FullyDecoded`

基于当前模型与 `report.rs` 用法，建议初版判定：

- `\u{5}SummaryInformation`
- `\u{5}DocumentSummaryInformation`
- `PSMroots`
- `DocVersion2`
- `DocVersion3`
- `AppObject`
- `JTaggedTxtStgList`

## 完成定义

此计划完成时，应满足：

- `pid_inspect` 能输出 coverage summary
- 代码里存在可复用的 `CoverageReport`
- 顶层流不再只有“unknown / not unknown”二分法
- 后续 Phase 2 可以直接利用 coverage 结果决定先攻哪条流

Plan complete and saved to `docs/plans/2026-04-21-sppid-coverage-inventory-implementation-plan.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration
2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints

Which approach?
