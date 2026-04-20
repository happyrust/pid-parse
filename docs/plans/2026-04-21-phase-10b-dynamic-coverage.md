# 开发计划：Phase 10b — 动态 coverage 分类

> 起稿：2026-04-21
> 背景：Phase 10a（v0.6.0）给 SPPID 覆盖清单建立了基础设施，但
> 分类方式是**纯静态**：`KNOWN_TOP_LEVEL_STREAM_NAMES` 里的每个名字
> 被硬编码到一个状态。这在 Phase 10a 的 plan 和模块 doc 里都明确
> 标为"v0.6.0 初版 — Phase 10b+ 会动态化"。本轮兑现该承诺。
> 目标：v0.6.1 patch ship。

## 动机

静态映射的两个可观察问题：

1. **假阳性 FullyDecoded**：某些 fixture 里 `DocVersion3` 流存在但
   `doc.version_history` 因 parser 容忍性失败而为 `None`；静态映射
   仍报 `FullyDecoded`，掩盖了 parser 的 silent failure。
2. **假 PartiallyDecoded**：空 fixture 跑下来，`PSMclustertable` 流
   不存在但也不会出现在 coverage（因为 coverage 只遍历 `doc.streams`
   里实际存在的条目），这 OK；但如果流存在而 `doc.psm_cluster_table
   == None`，仍被标为 `PartiallyDecoded`，不反映真实状态。

Phase 10b 用 `&PidDocument` 里的实际字段填充来校正状态：

- `FullyDecoded` 要求相应 `doc.X` 字段是 `Some` 且记录非空
- `PartiallyDecoded` 要求相应字段至少有内容，否则降级为
  `IdentifiedOnly`
- `IdentifiedOnly` 保持不变（storage prefix 的定义就是"识别但不必
  解读内部"）
- `Unknown` 保持不变

## 非目标

- 不改 parser 本身
- 不新增 coverage 状态枚举值
- 不动 CLI 签名（`--coverage` 行为不变）
- 不改 JSON schema（`CoverageReport` 序列化字段集不变）

## 范围

| 文件 | 改动类型 | 行数估计 |
|---|---|---|
| `src/inspect/coverage.rs` | `classify` 加 `&PidDocument` 参数；新增 `would_be_fully_decoded` / `is_partially_populated` helpers | +90 |
| `src/inspect/coverage.rs::tests` | +4 动态分类单测（`version_history` 缺失降级 / `psm_cluster_table` 空降级 / 填满仍 Full / Unknown 不受影响）| +120 |
| `tests/inspect_cli.rs` | +1 端到端测试：空 pid 字段但带流时 coverage 反映 degraded | +40 |
| `CHANGELOG.md` | `[0.6.1]` 段 | +30 |
| `Cargo.toml` | 0.6.0 → 0.6.1 | ±1 |
| **本 plan** | | +本文件 |

合计 ~280 行。全部是加法。

## 关键决策

### A. `classify` 签名

```rust
fn classify(name: &str, doc: &PidDocument) -> CoverageEntry
```

之前是 `fn classify(name: &str) -> CoverageEntry`。加 `&PidDocument`
给每个 match 分支可以做额外校验。

### B. 状态降级表

| 流名 | 静态 | 动态条件 | 降级到 |
|---|---|---|---|
| `\x05SummaryInformation` / `\x05DocumentSummaryInformation` | FullyDecoded | `doc.summary.is_some()` | `IdentifiedOnly` |
| `PSMroots` | FullyDecoded | `doc.psm_roots.is_some() && !entries.is_empty()` | `IdentifiedOnly` |
| `DocVersion2` | FullyDecoded | `doc.doc_version2_decoded.is_some()` | `IdentifiedOnly` |
| `DocVersion3` | FullyDecoded | `doc.version_history.is_some() && !records.is_empty()` | `IdentifiedOnly` |
| `AppObject` | FullyDecoded | `doc.app_object_registry.is_some()` | `IdentifiedOnly` |
| `JTaggedTxtStgList` | FullyDecoded | `doc.tagged_storages.is_some()` | `IdentifiedOnly` |
| `PSMclustertable` | PartiallyDecoded | `doc.psm_cluster_table.is_some()` | `IdentifiedOnly` |
| `PSMsegmenttable` | PartiallyDecoded | `doc.psm_segment_table.is_some()` | `IdentifiedOnly` |

`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` /
`Unclustered Dynamic Attributes` 的"PartiallyDecoded" 状态依赖 
`doc.clusters` / `doc.dynamic_attrs` 的相应条目，但这些 model 形状
复杂（多条 record 分流），本期保留静态映射 + `note` 显式说明"本
轮未升级"，等 Phase 10c 再系统处理。

### C. 降级时 `note` 的更新

当动态校验触发降级（如 `DocVersion3` → `IdentifiedOnly`），`note`
字段补一条明确信息：

```text
Fully decoded: DocVersion3 → version_history  (stream present; parser unable to decode — investigate)
```

降级后的 note 明确"signal"而不是仅"status"，下游阅读者能立刻看出
问题点。

### D. `classify` 的可测试性

目前 `classify` 是私有 helper。本期保持私有；单元测试走
`coverage_report(&doc)` 的公共入口，通过填充不同 `doc` 状态来触发
各分支。这样既保持 API 边界清晰，也让测试反映真实 caller 行为。

## 实施步骤

### W1 — `classify` 改签名

扩 `classify(name, doc)`，在每个 Full/Partial 分支加动态校验。

### W2 — 动态分类单元测试

`src/inspect/coverage.rs::tests` 新增：

1. `coverage_downgrades_docversion3_when_parser_did_not_populate`：
   构造 doc with `/DocVersion3` stream but `version_history = None`
   → expect `IdentifiedOnly` + note 含 "stream present; parser..."
2. `coverage_downgrades_psm_cluster_table_when_empty_model`
3. `coverage_keeps_fully_decoded_when_model_populated`：填充
   `version_history` with records → `FullyDecoded`
4. `coverage_unknown_and_identified_unaffected_by_model_state`

### W3 — CLI 集成验证

`tests/inspect_cli.rs` 新增一条：构造 fixture 含
`/DocVersion3` 但内容不合法（如全 `\0`）；CLI `--coverage` 输出
`[ID]` 而非 `[FULL]`，note 暴露降级原因。

### W4 — ship

bump 0.6.0 → 0.6.1；CHANGELOG 加 `[0.6.1]` 段说明"动态化兑现"；
commit + tag v0.6.1。

## 预计工时

- W1: 40 min
- W2: 45 min
- W3: 25 min
- W4: 15 min
- **合计**: ~2 hr

## 验证清单

- [ ] `cargo fmt --check` / clippy / test 全 0
- [ ] test count 307 → 312+
- [ ] 手工验证：如果 fixture 里 `version_history` 为 None，CLI
      `--coverage` 输出 `[ID]   DocVersion3` 而不是 `[FULL]`
- [ ] `Cargo.toml` version = "0.6.1"
- [ ] `git tag --list v0.6.1` 有输出

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 降级 note 让 coverage 输出变长 | 只在 note 为 None 时补 note，不重复拼接；降级的 note 替换原静态 note |
| 某些测试之前用 coverage 静态语义，改动后 fail | W2 单测前跑一次全测，定位所有 coverage-dependent tests 并按需修；Phase 10a 只新增 3 个报告测试 + 6 个 coverage 模块测试，预期影响点可控 |
| `doc` 字段 None 判定逻辑分散到多个 match arm 难维护 | 抽一个 `is_fully_decoded_dynamic(name, doc) -> bool` helper，集中在一个函数里 |

## 回滚

`classify` 的签名改动会波及调用点（只有 `top_level_coverage_entries`
一处），revert 单 commit 即可。

## Next 候选

- **Phase 10c**：PSMcluster0 / Dynamic Attributes 等"结构形状复杂"
  的流的动态分类
- **Phase 10d**：`DocVersion3` parser 升级（operation 字段 enum 化，
  timestamp format 验证，cross-validate with `DocVersion2`）
- **Phase 10e**：PSMclustertable per-record 字段精确映射（roadmap 2.2）
