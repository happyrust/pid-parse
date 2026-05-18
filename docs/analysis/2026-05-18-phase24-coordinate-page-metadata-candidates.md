# Phase 24 Task 24-01 分析：CoordinatePageMetadata 候选证据表

> 日期：2026-05-18  
> 阶段：Phase 24 — CoordinatePageMetadata decoder 候选筛选  
> Task：24-01（candidate marker group evidence table）  
> 数据来源：`cargo run --release --example probe_phase24_top_evidence`  
> 触发样本：5 fixtures × 7 sheets（DWG-0201, DWG-0202, 工艺管道-1, A01, D06）  
> 状态：审查就绪，触发 Task 24-02 stop-and-challenge

## 0. TL;DR — 推荐：以 negative evidence 收口

Phase 24 plan 在 frontmatter 已经预言三条 Stop-And-Challenge 条件，本次
Task 24-01 证据**满足其中 3 条**：

| Stop-And-Challenge 条件 | 实际状态 |
|---|---|
| Top candidate 没有跨 fixture/sheet support | ✅ 满足（25/26 marker 单 fixture） |
| `page_dimension_scalar_matches` 继续为 0 | ✅ 满足（cross-fixture 29 行全部为 0） |
| 字段解释需要猜单位、方向或 origin | ✅ 满足（normalized f64 全部缺 page-dim 锚点） |
| 任何实现会让 `PidPageTransform::Available` 出现 | ❌ 不适用（尚未实现 typed DTO） |

**结论**：按 Phase 24 plan 第 219-228 行 Task 24-04 的"若 task 24-03 未执行，
则明确记录 blocker / negative evidence" 路径，**推荐 Task 24-02 review
checkpoint 选择 stop-and-record，跳过 Task 24-03 typed DTO 实现**。

负证据已经足够明确：当前 fixture 覆盖度不支持把任何 marker group
promotion 为 `CoordinatePageMetadata` typed candidate；Phase 23
guardrails 已经在每个 candidate 上挂上
`probe_only_no_coordinate_page_metadata_promotion`，本阶段不破坏该
边界。

## 1. 数据获取方法

```powershell
cargo run --release --example probe_phase24_top_evidence > out.md
```

probe 调用链：

```text
PidParser::new().parse_package(path)
  ↓ pkg.streams["/Sheet*"].data
  ↓ probe_sheet_stream(...) → SheetProbeReport
  ↓ sheet_record_shape_inventory(...) → SheetRecordShapeInventory
  ↓ coordinate_page_metadata_investigation_report(
        data,
        inventory,
        normalized.page_dimensions_mm,
    ) → SheetCoordinatePageMetadataInvestigationReport
  ↓ report.top_evidence (每个 sheet 最多 8 项)
```

每个 sheet 上限 8 个 top_evidence；cross-fixture aggregate = 29 行
（probe 输出）/ 36（Phase 23 cross-fixture test 累计）。差异源于
probe 与 test 对 page_dimensions_mm 的获取路径略有不同（probe 用
`build_normalized_geometry` 当前结果；test 也用同一 API，但聚合发生
在不同时机），不影响结论。

## 2. 全局统计

| 指标 | 数值 |
|---|---:|
| total top_evidence 行 | 29 |
| 行 `page_dimension_scalar_matches > 0` | **0** |
| 行 `normalized_f64_pairs > 0` | 25 |
| 出现在 ≥ 2 fixture 的 marker 数 | **1** |
| 出现在 ≥ 2 fixture 且 kind 一致的 marker 数 | **0** |

## 3. 候选 kind 分布

| Kind | 行数 | 占比 |
|---|---:|---:|
| `NormalizedF64CoordinateLike` | 25 | 86 % |
| `I32CoordinateDomainLike` | 2 | 7 % |
| `InsufficientEvidence` | 2 | 7 % |
| `PageDimensionScalarLike` | 0 | 0 % |
| `MixedNumeric` | 0 | 0 % |

**关键事实**：`PageDimensionScalarLike = 0`。Phase 23 已经把这条作为
最高优先级 kind 暴露给 ranking（`coordinate_page_metadata_rank` kind_rank
PageDimensionScalarLike = 5），但全 29 行无一命中 → page dimension scalar
source 未出现。

## 4. Top 5 候选（按 support / norm_f64 / fixture 覆盖排序）

| 排名 | Fixture | Marker | Support | Norm f64 | Range | Kind | 评估 |
|---|---|---|---:|---:|---:|---|---|
| 1 | DWG-0202 | `0x6BCA (27594)` | 1 | 127 | 11489 | NormalizedF64CoordinateLike | 单 fixture / 范围巨大但无 page-dim 锚点 |
| 2 | 工艺管道-1 | `0xD5C0 (54720)` | 1 | 103 | 2617 | NormalizedF64CoordinateLike | 单 fixture / 高 norm_f64 但缺 cross-validation |
| 3 | 工艺管道-1 | `0x8DE7 (36327)` | 1 | 99 | 14044 | NormalizedF64CoordinateLike | 单 fixture / 范围巨大 |
| 4 | 工艺管道-1 | `0x3FD1 (16337)` | 1 | 70 | 4746 | NormalizedF64CoordinateLike | 单 fixture |
| 5 | DWG-0201 | `0xC03F (49215)` | 1 | 48 | 12161 | NormalizedF64CoordinateLike | Phase 24 known_unknown marker，但单 fixture |

**共同特征**：
- 全部 `support = 1`（无重复模式）
- 全部 `page_dimension_scalar_matches = 0`
- 全部跨 fixture 不存在二次确认

按 Phase 24 plan §1 "Must-Haves" 第 2 条：
> 候选字段必须能解释 width/height/origin/scale/bounds 中至少一组完整字段，
> 否则不 promotion。

**全部 Top 5 均不满足** — 因为缺 page-dim 锚点，f64 pair 数据无法被解释为
width/height/origin/scale 中任何一项的"完整字段组"。

## 5. Rejected 候选与理由

### 5.1 Marker `0xC03F (49215)`（Phase 24 plan known_unknown）

- Plan 行 48：`marker 49215 是否是真实 page metadata record 仍未证明`
- 证据：DWG-0201 出现 2 次（同 fixture / 同 /Sheet6），其它 4 个
  fixture 均无；A01 / DWG-0202 / 工艺管道-1 / D06 在 top_evidence
  里完全缺失。
- 拒绝理由：**单 fixture support** + **0 page-dim match** + Phase 24 plan
  Stop-And-Challenge 第 1 条触发。

### 5.2 Marker `0xE0D9 (57561)`

- 证据：DWG-0202 出现 2 次（同 fixture），其它 4 个 fixture 均无。
- 拒绝理由：同 5.1。

### 5.3 Marker `0x0000 (0)`

- 证据：DWG-0202 1 次 NormalizedF64CoordinateLike (norm_f64=12)；
  D06 1 次 InsufficientEvidence (norm_f64=0)。**跨 fixture 出现** 但
  **kind 不一致**。
- 拒绝理由：marker 0 是高频通配 / 0 字节填充，无 selectivity；kind
  跨 fixture 不一致说明不是 stable record shape。

### 5.4 Marker `0xFE4E (65102)`

- 证据：A01 唯一一次 `I32CoordinateDomainLike (i32_pairs=7)`，其它
  fixture 无。
- 拒绝理由：单 fixture support + 缺 f64 / page-dim 锚点 + 仅 i32 pairs
  不足以解释 width/height/origin/scale 字段组。

### 5.5 D06 `0x0000 (0) InsufficientEvidence`

- 证据：D06 唯一一行，仅 27 字节，`i32_pairs=0, f64_pairs=0,
  norm_f64=0, page-dim=0`。
- 拒绝理由：plan §Stop-And-Challenge 第 2 条 ("`page_dimension_scalar_matches`
  继续为 0") 触发，且字段数为零；典型 InsufficientEvidence。

## 6. 与 Phase 23 cross-fixture 聚合结论对照

Phase 23 `sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion`
当前输出：

```text
fixtures_seen=5
sheets_seen=7
coordinate_metadata_candidates=97
coordinate_top_evidence=36
normalized_f64_pair_count=1397
page_dimension_scalar_matches=0    ← 一致
curve_groups=97
marker_49215_groups=3              ← Phase 24 known_unknown
polyline_like=0
mixed_numeric=43
short_i32_sequences=24
```

`page_dimension_scalar_matches=0` 在 cross-fixture aggregate 与本 Task
24-01 detail dump **完全一致**，互相印证 negative evidence。

## 7. Task 24-02 review checkpoint 建议

按 Phase 24 plan Task 24-02 `<done>`：

> 用户/执行者明确选择：继续 typed candidate decoder，或以 negative
> analysis 收口。

**推荐**：**以 negative analysis 收口**。

后续 Task 路径分支：

### A) 收口路径（推荐）

- 跳过 Task 24-03（不新增 typed candidate DTO）
- 执行 Task 24-04：在 CHANGELOG.md / findings.md / progress.md / task_plan.md
  写入"Phase 24 以 negative evidence 收口"的明确边界
- Phase 24 关闭，保持 Phase 23 guardrail 不变
- 不消耗 `closure_claim_limit` 中的 "page transform decoded" 配额

### B) 强行实现 typed candidate DTO 路径（不推荐）

如果用户仍想推进 Task 24-03，则必须接受以下成本：
- DTO 字段名只能是 byte-position / evidence-oriented（如
  `marker_type / range_len / candidate_f64_pairs`），不能命名为
  `page_transform_*`
- DTO 只能锁 **单 fixture** 的 evidence，无 cross-fixture ratchet
- 后续若发现 marker / fields 不稳定，必须显式标记为 blocked / withdrawn
- 增加 schema 维护负担而不带来 transform availability 的实际进展

### C) 引入新 fixture 路径（escalation_triggers 第 3 条）

如果未来新增 PID fixture，可以重跑本 probe，再决定是否启动 Task 24-03。
触发条件：至少 2 个新 fixture 在 **相同 marker** 上出现 **kind
一致** 的 top_evidence，且至少 1 行 `page_dimension_scalar_matches > 0`。

## 8. 不变量与边界声明

本 Task 24-01 严格遵守 Phase 24 plan frontmatter：

| 边界 | 验证 |
|---|---|
| `non_goals: 不直接把 PidPageTransform::Available 接入 normalized geometry` | ✅ probe 只读，未触发 promotion |
| `hard_boundaries: 只有 cross-fixture 稳定 marker group 才能进入 typed decoder 候选` | ✅ 找不到这样的 marker，决策对齐 |
| `hard_boundaries: decoder 第一版只输出 audit/typed metadata DTO，不改变 entity 坐标` | ✅ 本 Task 不动 entity 坐标 |
| `anti_regression_targets: Phase 23 guardrails 继续通过` | ✅ probe 用 Phase 23 API，未改变 guardrail 行为 |
| `closure_claim_limit: 只能声明 typed candidate / negative evidence` | ✅ 本文档声明 negative evidence，未越界 |
| `approval_gates: 推送前必须明确授权` | ⏳ 本文档完成后等用户授权再 push |

## 9. 复现命令

```powershell
# 1. 验证 Phase 23 cross-fixture investigation 仍然过 ratchet
cargo test --locked -j 1 --test parse_real_files `
  sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion `
  -- --nocapture

# 2. 重新生成 candidate evidence table
cargo run --release --example probe_phase24_top_evidence

# 3. 全工作区 gates（必须在 push 前全绿）
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```
