# Plan: Phase 25-A normalized_f64_pair 空间分布挖掘

## 1. Solution Overview

```
[Phase 23 coordinate_page_metadata_investigation_report]
         ↓ 提取 normalized_f64_pair (1397 个)
[Phase 25-A coordinate_pair_spatial_analysis] (新增)
         ↓ per-sheet cluster + bbox + nearest-neighbor
[SheetSpatialAnalysisReport] (新 DTO)
         ↓ cluster id 附加到 provenance.note
[PidGraphicProvenance.note += "cluster=<id>"] (现有字段)
         ↓ docs/analysis/...
[analysis doc] (新增)
```

**纯 read-only 分析层**，不引入新 promotion，不改变 entity confidence / kind。

## 2. Why This Approach（only this approach）

| 候选 | 优点 | 缺点 | 决策 |
|---|---|---|---|
| **A. spatial bucketing + DBSCAN-style cluster**（推荐）| 算法简单 / 确定性强 / 对噪点鲁棒 / 不需 page transform | 阈值需要 fixture-level tuning | **本 phase 采用** |
| B. k-means | 算法成熟 | 需要预设 K / 对 outlier 敏感 / 不确定 | 拒绝 |
| C. 跨 fixture 坐标对齐后做 global cluster | 全局视野 | 违反 Phase 24 negative — 跨 fixture 坐标不可信 | 拒绝 |
| D. 不做 spatial analysis，直接提升 inferred_points 的 confidence | 落地快 | 没有新证据，违反 source-proven gate | 拒绝 |

## 3. How It Will Work

### 3.1 Cluster 算法选型

**DBSCAN-style spatial bucketing**（不依赖 sklearn / external crate）：

1. 对一个 sheet 的所有 f64 pair `(x, y)`：
   - 计算 sheet-local bounding box `(xmin, ymin, xmax, ymax)`
   - 划分为 `N × N` grid（N 由 pair 数量决定，初版 N=20）
   - 每个 grid cell 是一个候选 cluster
2. 合并相邻 non-empty cell 为同一 cluster
3. 输出每个 cluster 的：
   - `cluster_id`（连续整数，per-sheet stable）
   - `pair_count`
   - `bounding_box`（cluster-local）
   - `centroid`（mean of pairs in cluster）

**确定性**：grid 划分按 bounding box 等分，pair 分配按 floor(x / grid_size)
顺序遍历，不依赖 hash / random。

### 3.2 数据流

```
sheet.geometry.normalized_f64_pairs  (Vec<(f64, f64)>)
                ↓
spatial_bucketing(pairs, n_grid=20)
                ↓
Vec<ClusterCandidate>
                ↓
merge_adjacent_non_empty_cells(candidates)
                ↓
Vec<Cluster { id, pair_count, bbox, centroid }>
                ↓
attach_cluster_id_to_provenance(geometry.entities, clusters)
                ↓
PidGraphicProvenance.note += "cluster=<id>"
```

### 3.3 新增 DTO

```rust
// src/parsers/sheet_records.rs（紧跟 SheetCoordinatePageMetadataInvestigationReport）
pub struct SheetSpatialAnalysisReport {
    pub cluster_count: usize,
    pub clusters: Vec<SpatialCluster>,
    pub uniform_distribution: bool,  // true 时触发 negative 收口
}

pub struct SpatialCluster {
    pub id: u32,
    pub pair_count: usize,
    pub bbox: ((f64, f64), (f64, f64)),  // ((xmin, ymin), (xmax, ymax))
    pub centroid: (f64, f64),
}
```

### 3.4 New API

```rust
pub fn coordinate_pair_spatial_analysis(
    pairs: &[(f64, f64)],
    n_grid: usize,
) -> SheetSpatialAnalysisReport;
```

### 3.5 Provenance Note 格式

现有 note 字段是 free-form string（`PidGraphicProvenance::note: Option<String>`）。
本 phase **append**（不 replace）：

```
原 note: "decoded igPoint2d at offset 0x1234"
增强后: "decoded igPoint2d at offset 0x1234 | cluster=3"
```

格式约束：
- 分隔符固定为 ` | `
- key=value 形式
- 后续 phase 可继续 append 其它 evidence

## 4. Slices

| Slice | Purpose | Files | Done when | Risks |
|---|---|---|---|---|
| A | 探索现有 normalized_f64_pair 的 cross-fixture 实际分布 | `examples/probe_phase25_f64_spatial.rs`（新增） | dump 每 sheet 的 pair count / bbox / nearest-neighbor 直方图 | 数据可能 fixture-uniform，触发 negative 路径 |
| B | 新增 `SheetSpatialAnalysisReport` DTO + `coordinate_pair_spatial_analysis` API | `src/parsers/sheet_records.rs` | DTO + API + 6-10 unit tests | 算法确定性需要 panic-safety + hash-stable test |
| C | 接入 `model.rs`：新增 `SheetGeometry::spatial_analysis` field | `src/model.rs` + `src/schema.rs` | schema ratchet 通过；From impl 完整 | schema 字段新增需要 backward-compat |
| D | 在 `streams/cluster.rs` pipeline 里填充 spatial_analysis | `src/streams/cluster.rs` + `src/cfb/reader.rs` + `src/geometry.rs` | cargo test --workspace 通过 | cluster pipeline test fixture 需要同步 |
| E | Cross-fixture aggregate test + ratchet | `tests/parse_real_files.rs` | cluster count baseline 锁定（具体 N 由 probe 结果决定） | 若所有 fixture cluster_count <= 1 → 触发 negative |
| F | （可选，**仅** non-negative 路径）attach cluster id 到 inferred_points 的 `provenance.note` | `src/geometry.rs::build_normalized_geometry` | 80 inferred_points 都有 cluster id | provenance.note 格式向后兼容 |
| G | `docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md` | docs/ | 每 fixture × 每 sheet 的 cluster 报告 + cross-fixture 形态对比 | doc 长度控制 |
| H | 5 道 pre-commit gate + Phase 23/24 guardrail 回归 | `.github/scripts/` + tests | 全绿 | missing-docs 不能上升 |

## 5. Estimated Workload

- Slice A: 1 session（probe + 数据收集）
- Slice B-E: 1 session（DTO + API + pipeline + ratchet）
- Slice F: 0.5 session（仅 non-negative 路径）
- Slice G: 0.5 session（analysis doc）
- Slice H: 0.5 session（gates）

**总计**：1-2 session（按 Phase 25-A brief 的预估一致）。

若 Slice A 触发 negative 路径，Slice F 跳过，总计 1 session 可收口。

## 6. Decision Points

| Slice | Decision | 触发 |
|---|---|---|
| A → B | continue / negative 收口 | 若 Slice A 显示所有 fixture cluster_count <= 1，转 negative |
| E → F | attach cluster id / skip | 若 Slice E 显示 cluster 分布不显著（majority sheet cluster=1），跳过 F |
| F → G | full goal_complete / partial_complete | 同上 |

## 7. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Cluster algorithm 阈值选错，跨 fixture 不可比 | Medium | High | Slice A probe 先 dump 实际分布，阈值用 data-driven 默认 |
| spatial_analysis 引入新 schema 字段破坏 backward-compat | Low | Medium | 用 `#[serde(default)]` 标注新字段 |
| Phase 23/24 guardrail 因 spatial_analysis 副作用退化 | Low | High | Slice H 显式跑 guardrail 测试；若退化立即回滚 |
| 全 fixture uniform 分布 → 整个 phase 失败 | Medium | Low | 接受 negative 收口；记录到 analysis doc；下 phase 跳 25-C |
| Provenance.note append 影响下游消费 | Low | Medium | 格式固定 ` | key=value`；下游 split 即可 |
