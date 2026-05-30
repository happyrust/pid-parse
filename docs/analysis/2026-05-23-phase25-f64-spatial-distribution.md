# Phase 25-A — normalized_f64_pair 空间分布分析（API + model 落地）

> 日期：2026-05-23（落地收敛 2026-05-30）
> 范围：把 Slice A probe 验证过的空间分布算法固化为正式 API +
> model 字段 + pipeline 接入 + cross-fixture ratchet。**纯 read-only
> 分析层**：不 promotion 任何 entity、不改 normalized geometry
> transform、不引入新 `PidGraphicKind` variant、不令
> `PidPageTransform::Available` 出现。
> 复现命令：
> ```powershell
> cargo test --locked --lib parsers::sheet_records::tests::spatial_analysis -- --nocapture
> cargo test --locked --test parse_real_files spatial_analysis_cross_fixture -- --nocapture
> cargo test --locked --test parse_real_files spatial_analysis_pipeline_populates_sheet_geometry -- --nocapture
> cargo run --release --example probe_phase25_f64_spatial   # Slice A 原始 heatmap
> ```

---

## 0. 结论（一句话）

Cross-fixture 的 `normalized_f64_pair` 空间分布**显著非均匀**：5 个
`geometry_fixture_cases` fixture 的 7 个含 pair 的 `/Sheet*` 全部
产出 ≥2 个连通簇，合计 **83 簇 / 1698 对**，单 sheet 最高 27 簇。
Slice A 的 positive 信号被正式 API + model 字段 + pipeline ratchet
锁定，未触发 negative 收口。

---

## 1. 落地内容

| 层 | 符号 | 文件 |
|---|---|---|
| Parser API | `coordinate_pair_spatial_analysis(pairs, n_grid)` | `src/parsers/sheet_records.rs` |
| Parser DTO | `SheetSpatialAnalysisReport` / `SheetSpatialCluster` | `src/parsers/sheet_records.rs` |
| 取数 helper | `collect_normalized_f64_pairs(bytes)`（`normalized_f64_pair_count` 复用它） | `src/parsers/sheet_records.rs` |
| 默认网格常量 | `SPATIAL_ANALYSIS_DEFAULT_GRID_N = 20` | `src/parsers/sheet_records.rs` |
| Model DTO | `DecodedSpatialAnalysis` / `DecodedSpatialCluster` + `From` | `src/model.rs` |
| Model 字段 | `SheetGeometry::spatial_analysis: Option<DecodedSpatialAnalysis>` | `src/model.rs` |
| Pipeline | `sheet_geometry_from_probe` 填充 `spatial_analysis` | `src/streams/cluster.rs` |
| Schema 合约 | schema needles（`DecodedSpatialAnalysis` / `spatial_analysis` / `grid_resolution` / `uniform_distribution` / `centroid_x` …） | `src/schema.rs` |

**算法**：16 字节窗口、4 字节对齐扫描 `(x, y)` 双 f64 →
保留 finite 且 ∈ `[-1e-9, 1+1e-9]` 且非全零的对 → 投到 `N×N`
（默认 20×20）网格 → 4-邻接连通分量合并 → 每簇输出
`id / pair_count / bbox / centroid`。确定性 + panic-safe（NaN /
Inf / 越界统一 clamp 到 `[0,1]`）。

---

## 2. Cross-fixture cluster baseline（`geometry_fixture_cases`，N=20）

来源：`spatial_analysis_cross_fixture`（exhaustive 整 sheet 扫描）。

| Fixture | category | /Sheet* (pairs/clusters) | fixture pairs | fixture clusters |
|---|---|---|---:|---:|
| `DWG-0201GP06-01.pid` | dwg | `/Sheet6` 655/15 | 655 | 15 |
| `DWG-0202GP06-01.pid` | dwg | `/Sheet6` 268/10 · `/Sheet6615` 25/5 | 293 | 15 |
| `工艺管道及仪表流程-1.pid` | non_ascii | `/Sheet6` 427/27 | 427 | 27 |
| `export-test/.../A01/A01.pid` | publish_a01 | `/Sheet6` 30/11 | 30 | 11 |
| `export-test/.../DWG-0202GP06-01.pid` | publish_dwg | `/Sheet6` 268/10 · `/Sheet6615` 25/5 | 293 | 15 |
| **合计** | — | 7 sheets with pairs | **1698** | **83** |

聚合：`fixtures_seen=5, sheets_with_pairs=7, multi_cluster_sheets=7,
non_uniform_fixtures=5, max_clusters_on_single_sheet=27`。

`publish_dwg` 与独立 `DWG-0202` 逐项一致 → publish 通道未引入空间漂移。

> **与 Slice A probe 的差异**：Slice A probe（`docs/analysis/2026-05-23-phase25-slice-a-probe-output.md`）
> 的 fixture 集含 `D06`（`/Sheet6` 15/8）但不含 `publish_dwg`，故其
> 合计为 1420 对 / 76 簇 / 6 sheet。本节用 `geometry_fixture_cases`
> 作为 ratchet 锁定集；两者每个共同 fixture 的逐 sheet 数字一致。
> 每 sheet 的 ASCII 密度 heatmap 见 Slice A probe 文档。

---

## 3. Pipeline ↔ API 一致性

`spatial_analysis_pipeline_populates_sheet_geometry`：解析 DWG-0201 后
`/Sheet6` 的 `SheetGeometry::spatial_analysis` 被 pipeline 填充，且与
对同一原始字节直接调用 `coordinate_pair_spatial_analysis` 的结果在
`grid_resolution / pair_count / clusters.len() / uniform_distribution`
上完全一致；模型侧每簇 bbox ∈ `[0,1]²`、centroid ∈ bbox、各簇
`pair_count` 之和 = 总 pair 数。

---

## 4. Slice F（attach cluster id 到 provenance.note）—— Stop-And-Challenge 暂缓

计划 §2.2 / §3.5 的可选 Slice F 拟把 `cluster=<id>` append 到
inferred point 的 `provenance.note`。**暂缓，理由（坐标系不匹配）**：

- 本 phase 的空间簇建立在 **归一化 `[0,1]²` 的 f64 pair 空间**
  （16 字节整 sheet 扫描得到的候选坐标）。
- 而 `build_normalized_geometry` 产出的 `inferred_points`（Phase 10，
  80 个）来自 **i32 coordinate hint / endpoint 空间**（例如
  `PidPoint { x: 1200.0, y: -450.0 }`），与 `[0,1]²` 并非同一坐标系，
  二者之间的对应关系**尚未被字节级证据证明**。
- 在未证明两空间同源前，把 f64-pair-space 的 cluster id 贴到
  i32-hint-space 的点上会产生**误导性 provenance**，违反 source-proven
  gate 与计划 §3 Stop-And-Challenge 精神。

因此 Slice F 维持 deferred；簇证据以 `SheetGeometry::spatial_analysis`
独立 audit 字段呈现，**不**跨空间注入 entity provenance。解锁条件：
先证明 normalized f64 pair 与 inferred point 描述同一几何（独立任务）。

---

## 5. Guardrail 回归

| Guardrail | 状态 |
|---|---|
| Phase 23 `*_no_coordinate_page_metadata_promotion` / page-transform unavailable | 未退化 |
| Phase 24 negative（`page_dimension_scalar_matches=0`） | 未推翻 |
| Phase 14 八大解码器 cross-fixture inventory | 未退化 |
| `inferred_points=80` / `inferred_lines` DWG-0201 floor | 未退化 |
| 5 道 pre-commit gate | 全绿（含本 phase 新增测试） |

新增字段 `spatial_analysis` 仅在 sheet 本就产出 `SheetGeometry` 时填充
（emptiness 判定未改），故"哪些 sheet 有几何"行为零变化。

---

## 6. 引用

- `src/parsers/sheet_records.rs`：`coordinate_pair_spatial_analysis` /
  `collect_normalized_f64_pairs` / `SPATIAL_ANALYSIS_DEFAULT_GRID_N` +
  11 unit tests（`parsers::sheet_records::tests::spatial_analysis_*`）
- `tests/parse_real_files.rs`：`spatial_analysis_cross_fixture` /
  `spatial_analysis_pipeline_populates_sheet_geometry`
- `docs/analysis/2026-05-23-phase25-slice-a-probe-output.md`：Slice A
  per-sheet heatmap + D06
- `docs/plans/2026-05-23-phase25-next-development-plan-cn.md`：推荐路径
- `goals/phase25-f64-spatial-distribution/`：goal package + progress.jsonl
