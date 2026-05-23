# Blockers: Phase 25-A normalized_f64_pair 空间分布挖掘

## Open Questions

### Q1 — Cluster 算法阈值选型 [OPEN]

候选：

- N × N grid（N = 20 默认）+ 相邻 cell 合并（**推荐**，简单 / 确定性）
- DBSCAN（eps 由 nearest-neighbor 直方图 1st quartile 决定）
- HDBSCAN（更鲁棒，但实现复杂度高）

执行时按推荐 grid + adjacent merge。若 Slice A probe 发现 grid=20
对所有 fixture 都产生 cluster_count <= 1，调整 N=10 / 50 再 probe。

### Q2 — Cluster id 在 PidGraphicProvenance.note 的格式 [OPEN]

候选：

- `cluster=<id>`（**推荐**，简单 / 易 grep）
- `cluster_id=<id>`（更明确）
- JSON-encoded payload（最灵活，但 note 字段会变成 mini-DSL）

执行时按推荐 `cluster=<id>`。若下游消费方明确要求 JSON，再调整。

### Q3 — Uniform distribution 阈值 [OPEN]

判定"uniform"的标准：

- 候选 A：所有 fixture 的 cluster_count <= 1（**推荐**，严格）
- 候选 B：majority fixture 的 cluster_count <= 2
- 候选 C：cross-fixture cluster_count 方差 < 阈值

执行时按候选 A。若 A 标准下所有 fixture 都不 uniform 但分布很相似，
analysis doc 记录形态相似性，不触发 negative 收口。

### Q4 — Slice F provenance attach 是否阻塞 Slice G/H [DEFERRED]

若 Slice F 在 attach cluster id 时发现 80 inferred_points 中有少数
不属于任何 cluster（即 outlier），如何处理？

候选：

- 不 attach（**推荐**，note 字段保持原样）
- attach `cluster=outlier`（更显式）

执行时按 "不 attach 给 outlier"，并在 analysis doc 记录 outlier 数量。

### Q5 — Negative 路径下是否仍写 analysis doc [PARTIAL-AC PRESET]

**决定**：写。即使 negative 结论，analysis doc 也必须详细记录：

- 各 fixture 的 cluster_count 实际分布
- 为什么这种分布不适合做 spatial cluster
- 后续 phase 可以尝试的方向（cluster algorithm 替代 / 加 fixture）

negative analysis 比 silent 结束更有价值。

## Stop And Ask

任一条件成立立即停手，写 `progress.jsonl`，等用户回复：

1. Phase 14–24 任一 baseline 退化（5 道 gate 失败原因不是简单错误）。
2. 出现要修改 `PidGraphicKind` enum 的诱因（属于 source-proven gate
   工作）。
3. 出现要 promotion entity 为 `PidPageTransform::Available` 的诱因
   （违反 Phase 23 guardrail）。
4. 出现要跨 fixture 坐标对齐的诱因（违反 Phase 24 negative）。
5. 出现要接入新 fixture 的诱因（本 phase 在现有 5 fixture 上闭环）。
6. Cluster 算法 unit test 中 determinism 失败（hash 不稳定）。
7. `missing_docs` ratchet 上升（current > baseline=0）。
8. Slice A probe 在单 fixture 上耗时 > 5 分钟（说明 algorithm 效率
   有问题，需 profile）。

## Dangerous Or High-Risk Actions

必须先获得用户授权：

- 修改任何 src/ 代码以外的 vendored `oxidized-mdf/` 内容
- 修改任何 Phase 14-24 stable DTO 既有字段
- 删除任何现有 inferred point / line entity
- 修改 `PidPageTransform` enum
- 引入新 promotion 通道（即使 confidence=Inferred）
- commit / push（本 phase 完成时再问）

## Known Blockers

| ID | 类型 | 状态 | next action | owner |
|---|---|---|---|---|
| Q1 | algorithm | OPEN | 按推荐 grid=20 默认，Slice A probe 后微调 | agent |
| Q2 | format | OPEN | 按推荐 `cluster=<id>` | agent |
| Q3 | threshold | OPEN | 按推荐严格 cluster_count <= 1 | agent |
| Q4 | edge-case | DEFERRED | outlier 不 attach；记录 count | agent |
| Q5 | negative | PRESET | negative 路径也写 analysis doc | agent |

## 当前状态总表

- 数据已就绪：1397 个 normalized_f64_pair 跨 5 fixture × 7 sheet
- Phase 24 negative 不阻塞：本 phase 是 read-only spatial analysis，
  不动 page transform
- Phase 23 guardrail 完全保留
- 风险点：cluster algorithm 在数据上的效果未知，可能 negative 收口
  （已预设处理路径）
- 工作量预估：1-2 session（详见 plan.md §5）
