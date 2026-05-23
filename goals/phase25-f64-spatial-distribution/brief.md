# Phase 25-A: normalized_f64_pair 空间分布挖掘

## 目标产出

在 **不 promotion page transform** 的前提下，把 cross-fixture 1397 个
`normalized_f64_pair` 从 "raw coordinate evidence" 升级为 **可被 H7CAD
消费的 spatial topology evidence**。具体回答 3 个问题：

1. **每个 sheet 的 f64 pair 在它自己的坐标域内是否形成可识别的
   cluster？** （聚簇个数 / 包络盒 / nearest-neighbor 直方图）
2. **跨 fixture 的 cluster 形态是否一致？** （比如所有 fixture 都是
   "一个大 cluster + 若干小边角 cluster"，还是因 fixture 而异？）
3. **现有 `inferred_points = 80` 是否可以基于 cluster 信息做
   topology-aware 排序 / 邻接性增强？** （不引入新 point，只增强
   provenance 信息）

完成时输出：

- `docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md`（每
  fixture × 每 sheet 的 cluster 报告 + cross-fixture 形态对比）
- 新增 `coordinate_pair_spatial_analysis` API（read-only，附在现有
  `SheetCoordinatePageMetadataInvestigationReport` 之后）
- 新增 cross-fixture aggregate 测试（ratchet cluster count 下限）
- `PidGraphicProvenance::note` 字段附加 cluster id（**仅 evidence**，
  不改变 entity 的 `confidence` 或 `kind`）
- Phase 23 / Phase 24 guardrails 全部不退化

## 背景

Phase 23 的 `coordinate_page_metadata_investigation_report` 已经记录：

- `coordinate_metadata_candidates = 97`
- `coordinate_top_evidence = 36`
- `normalized_f64_pair_count = 1397`
- `page_dimension_scalar_matches = 0`（Phase 24 negative）

Phase 24 已经证明：**没有 cross-fixture 一致的 marker 可以让 page
transform available**。但 1397 个 f64 pair 本身是 high-confidence 的
坐标证据 — 它们只是 **缺乏可信的尺度锚点**。这意味着：

- f64 pair 在 **同一 sheet 内** 的 **相对** 位置关系是可信的
- 跨 sheet / 跨 fixture 的 **绝对** 位置不可信（因为缺尺度锚点）

H7CAD 当前消费 `inferred_points` 的方式是 **平铺所有 point**，没有
任何 topology / cluster / neighborhood 信息。如果能在
`PidGraphicProvenance::note` 里附加 cluster id，H7CAD 渲染时可以：

- 按 cluster 着色（让用户看出"哪些点属于同一设备 / 同一管段"）
- 优先渲染 cluster 中心点（降低噪点）
- 在 cluster 边界做高亮（突出几何特征）

这一切 **不需要** page transform，只需要 sheet-local 的相对几何。

## 上下文（必读）

| 文档 / 文件 | 作用 |
|---|---|
| `docs/plans/2026-05-23-phase25-next-development-plan-cn.md` | Phase 25+ 候选评估，本 phase 是其中 25-A |
| `docs/plans/2026-05-18-phase23-coordinate-page-context-plan-cn.md` | Phase 23 guardrails，本 phase 必须不退化 |
| `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md` | Phase 24 negative evidence，提示为什么不能 promotion page transform |
| `src/parsers/sheet_records.rs::SheetCoordinatePageMetadataInvestigationReport` | 现有 report，本 phase 新增 spatial analysis 作为伴随结构 |
| `src/geometry.rs::build_normalized_geometry` | 现有 normalize 流水线 |
| `src/model.rs::PidGraphicProvenance` | `note` 字段（本 phase 在这里附加 cluster id） |
| `examples/probe_phase24_top_evidence.rs` | Phase 24 probe，本 phase 可借用其 fixture 枚举逻辑 |
| `tests/parse_real_files.rs` | cross-fixture aggregate test 接入点 |

## 关键约束

- **只读分析**：不修改任何 entity 的 `confidence` / `kind` / `coords`
- **不引入新 PidGraphicKind variant**
- **不引入新 promotion 通道**（page transform 仍 unavailable）
- **不改变 PidPageTransform 状态**
- **cluster 算法必须 deterministic**（同输入产出同输出，可 ratchet）
- **cluster 算法必须 sheet-local**（不跨 sheet / 不跨 fixture 比较坐标）
- **5 道 pre-commit gate 保持绿**
- **Phase 23 `template_page_dimensions_do_not_make_page_transform_available`
  guardrail 全绿**
- **Phase 24 `probe_only_no_coordinate_page_metadata_promotion`
  guardrail 全绿**

## 非目标

- **不**修改 Phase 14 八大解码器的任何 baseline（`igLine2d=284` 等）
- **不**修改 Phase 15/16/17/18/19/20 任何 audit collection 的 shape
- **不**修改 Phase 22 D06 text placement regression
- **不**实现 Text/Symbol promotion
- **不**做跨 fixture 坐标对齐 / page transform 反推
- **不**接入新 fixture（本 phase 在现有 5 fixture 上闭环）
- **不**修改 H7CAD 仓库代码（H7CAD 端 cluster 着色由下游负责）
- **不**改变 `coordinate_top_evidence = 36` 的当前数值

## Ask Before（要先问）

- 选择 cluster 算法（DBSCAN / k-means / spatial grid bucketing 各有
  trade-off，需用户拍板）
- 在 `PidGraphicProvenance::note` 字段格式上做 breaking change
  （当前是 free-form string，是否需要保持 backward-compat）
- 若 cluster 分析发现某个 fixture 全 sheet 都是 "1 个大 cluster + 0 个
  小 cluster"，是否要把该 fixture 排除（影响 cross-fixture ratchet）
- 若所有 fixture 都得到 "高度均匀分布" 的结论 →
  negative analysis 收口（提前结束本 phase）
- commit / push（本 phase 完成时再问）

## Done Means（完成判据）

同时满足：

1. **AC1（spatial analysis API）**：新增
   `coordinate_pair_spatial_analysis(data, normalized.f64_pairs)` 返回
   `SheetSpatialAnalysisReport`，含每 sheet 的 cluster count /
   bounding box / nearest-neighbor distribution / cluster centroid 列表
2. **AC2（cluster algorithm 确定性）**：cluster 算法对同一输入产出
   同一输出（hash stable across runs），可 ratchet
3. **AC3（cross-fixture aggregate）**：新增
   `spatial_analysis_cross_fixture_baseline` test，ratchet
   每个 fixture 至少 N 个 cluster（N 由实际数据决定，写入 plan
   Slice E 后再 ratchet）
4. **AC4（PidGraphicProvenance 增强）**：现有 80 个
   `inferred_points` 的 `provenance.note` 字段附加
   `cluster=<id>` 标签（**仅** evidence，不改 confidence / kind）
5. **AC5（analysis doc）**：写
   `docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md`，
   含每 fixture × 每 sheet 的 cluster 报告 + cross-fixture 形态
   对比 + ASCII 散点示意（限制 ≤ 20 行 / sheet）
6. **AC6（Phase 23 / 24 guardrails 保持）**：
   `template_page_dimensions_do_not_make_page_transform_available`
   + `probe_only_no_coordinate_page_metadata_promotion` 全绿
7. **AC7（5 道 pre-commit gate）**：build / test / clippy / fmt /
   missing-docs 全绿
8. **AC8（progress.jsonl 完整 evidence trail）**：每个 Slice 完成时
   append 一条 entry，含命令 / 输出摘要 / AC 编号

## Closure 子集（negative 路径）

若 cluster 分析发现所有 fixture / 所有 sheet 都是 "高度均匀分布"
（即聚簇分布不显著），允许走 **negative analysis 收口**：

- 跳过 AC4（`PidGraphicProvenance` 增强）
- AC1 / AC2 / AC3 / AC5 / AC6 / AC7 / AC8 仍要满足
- analysis doc 必须明确记录 "negative result + 不 promotion 推理链"
- progress.jsonl append `partial_complete` entry（不 append `goal_complete`）

negative 收口后，Phase 25-A 标 partial complete，下个 phase 应跳到 25-C。
