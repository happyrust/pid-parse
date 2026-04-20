# 开发计划：Phase 10c — cluster / dynamic-attrs 动态 probe

> 起稿：2026-04-21
> 背景：Phase 10b 给 8 个 "单一顶层流 → 单一 model 字段" 的项配置了
> 动态 probe，但把 4 个 cluster / dynamic-attrs 流留到 Phase 10c 处理
> （理由：它们对应 `doc.clusters: Vec<ClusterInfo>` 这种多流合并聚合
> 字段，探针写法不同于"field Some/None 即答案"）。本轮补齐它们。
> 目标：v0.6.2 patch ship。

## 动机

`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` /
`Unclustered Dynamic Attributes` 目前在 coverage 报告里始终显示
`[PART]`，**即使 parser 实际上没有识别出任何 cluster**。这与 Phase
10b 为 `DocVersion3` 等设计的"暴露 silent failures"目标不一致。

## 非目标

- 不改 cluster parser 行为
- 不新增 `PartiallyDecoded` → `FullyDecoded` 的升级路径
  （cluster 类流本身就是 `PartiallyDecoded`，只做 `PartiallyDecoded → IdentifiedOnly` 降级）

## 范围

| 文件 | 改动 | 行数估计 |
|---|---|---|
| `src/inspect/coverage.rs` | `stream_is_populated` 4 arm | +30 |
| `src/inspect/coverage.rs::tests` | +4 单测（每 arm 一个 populate/empty pair）| +80 |
| `CHANGELOG.md` | `[0.6.2]` | +20 |
| `Cargo.toml` | 0.6.1 → 0.6.2 | ±1 |
| **本 plan** | | +本文件 |

~150 行。纯 additive。

## 关键决策

### A. probe 逻辑：按 `ClusterKind` 过滤

```rust
"PSMcluster0" => Some(
    doc.clusters
        .iter()
        .any(|c| matches!(c.kind, ClusterKind::PsmCluster)),
),
"StyleCluster" => Some(
    doc.clusters
        .iter()
        .any(|c| matches!(c.kind, ClusterKind::StyleCluster)),
),
"Dynamic Attributes Metadata" => Some(
    doc.clusters
        .iter()
        .any(|c| matches!(c.kind, ClusterKind::DynamicAttributesMetadata)),
),
"Unclustered Dynamic Attributes" => Some(
    // 此流对应 UnclusteredDynamicAttributes cluster_kind，或者
    // doc.dynamic_attributes blob 非空。两个任一 populate 即算通过。
    doc.clusters
        .iter()
        .any(|c| matches!(c.kind, ClusterKind::UnclusteredDynamicAttributes))
        || doc.dynamic_attributes.is_some(),
),
```

用 `matches!` 宏做 enum 模式匹配，不引入额外 helper。

### B. `document_field_for_known_stream` 扩展

4 个新 arm 返回的字段名用于降级 note：

- `PSMcluster0` → `"clusters (kind=PsmCluster)"`
- `StyleCluster` → `"clusters (kind=StyleCluster)"`
- `Dynamic Attributes Metadata` → `"clusters (kind=DynamicAttributesMetadata)"`
- `Unclustered Dynamic Attributes` → `"clusters (kind=UnclusteredDynamicAttributes) / dynamic_attributes"`

### C. module doc 更新

`src/inspect/coverage.rs` 顶层 doc comment 里 Phase 10b 段需要追加：
"Phase 10c: cluster & dynamic-attrs probes now wired; the four names
that were parked in v0.6.1 are fully dynamic as of v0.6.2."

## 实施步骤

### W1 — probe + field-name 扩展

改 `stream_is_populated` + `document_field_for_known_stream` 两处 match
各加 4 个 arm。

### W2 — 测试

`src/inspect/coverage.rs::tests` 新增：

1. `coverage_downgrades_psm_cluster0_when_no_cluster_kind_psmcluster`
2. `coverage_downgrades_style_cluster_when_no_cluster_kind_style`
3. `coverage_downgrades_dynamic_attrs_metadata_when_no_cluster_kind_dam`
4. `coverage_keeps_unclustered_dynamic_attrs_when_blob_or_cluster_populated`

每个测试构造一个 `ClusterInfo` 实例填到 `doc.clusters`，验证
`PartiallyDecoded` 在 populated 情形下保持，在非 populated 情形下降级
到 `IdentifiedOnly`。

### W3 — ship

bump 0.6.1 → 0.6.2；CHANGELOG 扩 `[0.6.2]` 段；commit + tag。

## 预计工时

- W1 ~15 min
- W2 ~30 min
- W3 ~10 min
- **合计 ~1 hr**

## 验证清单

- [ ] test count 312 → 316
- [ ] `cargo fmt/clippy/test` 全 0
- [ ] `Cargo.toml` 0.6.2
- [ ] `git tag v0.6.2` 存在

## Next 候选

- **Phase 10d**：`DocVersion3` parser 升级（operation enum 化、
  timestamp 格式校验、与 `DocVersion2` cross-validate）
- **Phase 10e**：PSMclustertable per-record 字段精确映射
- **Phase 10f**：字节级 consumed / leftover 报告框架
