# Phase 25+ 下一步开发计划（候选评估 + 推荐路径）

> 日期：2026-05-23
> 输入：Phase 14–24 全部已完成 / Phase 24 已 negative 收口
> 当前 dirty 文件：`examples/probe_phase24_top_evidence.rs`（Phase 24 Task 24-01 probe 微调）
> 目的：在 Phase 24 negative closeout 之后，决定下一阶段方向 + 拆解为可执行 goal package

---

## 0. 背景速览（一张表说清楚现在在哪）

| 维度 | 当前事实 |
|---|---|
| Cross-fixture 几何 entities | 769（Phase 14 八大解码器，5 fixture × 7 sheet） |
| Audit-only PSM record families | GraphicGroup 352 · JStyleOverride 98 · `0x0010` 582 |
| `0x0010` IDA RAD class identity | GUID `1D1928C0-0000-0000-C000-000000000046` 已锁定 / class name + Read-DoIO + sub-kind 未恢复 |
| `inferred_points` (H7CAD 可见) | 80（Phase 10） |
| `inferred_lines` (H7CAD 可见) | **0**（Phase 10 Slice 3 endpoint pair 两端不对称，未闭环） |
| `coordinate_metadata_candidates` | 97 |
| `coordinate_top_evidence` | 36 |
| `normalized_f64_pair_count` | 1397 |
| `page_dimension_scalar_matches` | **0**（Phase 24 negative） |
| Pre-commit gates | 5/5 全绿（build / test / clippy / fmt / missing-docs） |
| Workspace tests | 1000+（851 unit + 91 integration + 59 binaries） |
| 未提交修改 | `examples/probe_phase24_top_evidence.rs` |

---

## 1. 价值方向矩阵（候选汇总）

按"风险↓ × 收益↑ × 可执行性↑"打分（每项 1–5）：

| 候选 | 主题 | 风险 | 收益 | 可执行性 | 综合 | 备注 |
|---|---|---:|---:|---:|---:|---|
| P0 | 处理未提交 probe 修改 | 1 | 1 | 5 | **基础** | 必做，不算 phase |
| 25-A | `normalized_f64_pair` 空间分布挖掘（驱动 H7CAD inferred point 质量） | 2 | 4 | 5 | **11** | 低风险高价值 |
| 25-B | D06 `/Sheet6` 反向 + 跨 fixture text probe 映射 | 2 | 3 | 4 | **9** | 同时为 Phase 24 reopen 创造条件 |
| 25-C | endpoint pair → `Inferred Line` 闭环（Phase 10 Slice 3 未完工） | 3 | 5 | 3 | **11** | 直接解锁 H7CAD line render |
| 25-D | parser hardening 第二轮（PR #8 候选） | 1 | 2 | 5 | **8** | 持续性，可作为 P2 |
| 25-E | AGENTS.md Phase 14 GArc2d caveats 清理 | 1 | 2 | 5 | **8** | 文档卫生 |
| 25-F | PSM `0x0010` runtime hook（解 Phase 20 死结） | 5 | 5 | 1 | **11** | 需 SmartPlant 运行环境 + 动态分析工具 + 用户授权 |
| 25-G | H7CAD visual integration snapshot test | 4 | 4 | 2 | **10** | 跨仓 collaboration |
| 25-H | Publish XML / DWG 闭环加固（task_plan Phase 5） | 3 | 3 | 3 | **9** | MDF publish 路线，与 .pid 反向解析平行 |
| 25-I | 多 fixture 接入（H7CAD 真实工程项目）| 3 | 5 | 1 | **9** | 强依赖外部样本供给 |

---

## 2. 推荐路径（3 phase 滚动执行）

```
P0 → Phase 25-A → Phase 25-C → Phase 25-B
    (1h)         (1–2 session)   (3–5 session)   (1–2 session)
```

### 2.1 P0：清理未提交 probe（≤1h）

**动作**：

```powershell
git diff examples/probe_phase24_top_evidence.rs
```

判断后选择 commit / discard：
- 若是 Phase 24 Task 24-01 的微调残留 → `git commit -m "chore(phase24): polish top evidence probe"`
- 若是实验残留 → `git checkout examples/probe_phase24_top_evidence.rs`

**Done 条件**：`git status` clean，无 modified。

---

### 2.2 Phase 25-A：normalized_f64_pair 空间分布挖掘

**目标**：在 **不 promotion page transform** 的前提下，分析 cross-fixture 1397 个 f64 pair 的空间分布（聚簇 / 邻接 / 边界包络），为 H7CAD `Inferred Point` 渲染提供更稳定的拓扑证据。

**Scope**：
- ✅ 新增 `coordinate_pair_spatial_analysis` API：每 sheet 输出 cluster count / bounding box / nearest-neighbor distribution
- ✅ 新增 cross-fixture aggregate 测试
- ✅ 输出 analysis doc 描述每个 fixture 的空间特征
- ❌ 不 promotion 任何 entity 为 `PidPageTransform::Available`
- ❌ 不引入新的 `PidGraphicKind` variant
- ❌ 不改变 normalized geometry transform

**Gate**：
- Phase 23 `probe_only_no_coordinate_page_metadata_promotion` 不退化
- Phase 24 negative 结论不被推翻
- 5 道 pre-commit gate 全绿

**Risk**：
- 低：纯 read-only 分析层
- 已知 unknown：若聚簇分布在所有 fixture 都是"全 sheet 均匀分布"，则该分析无法提供拓扑证据

**Done 条件**：
- Analysis doc 落地 `docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md`
- 至少一个 fixture 产生非平凡聚簇证据（≥3 distinct cluster groups）
- 若无任何 fixture 产生有意义聚簇 → negative analysis 收口，跳到 25-C

**预计**：1–2 session

---

### 2.3 Phase 25-C：endpoint pair → Inferred Line 闭环

**目标**：把 task_plan Phase 10 Slice 3 的 `inferred_lines = 0` 困局推进到 ≥40/59 endpoint pair → Line（H7CAD 可见）。

**Scope**：
- ✅ 重新检视 endpoint pair 两端不对称的字节级证据
- ✅ 新增对称性条件松弛（或新增匹配维度）
- ✅ Cross-fixture ratchet：`inferred_lines >= 40`
- ❌ 不创造新的 promotion 类型（仍走 `PidGraphicKind::Line`）
- ❌ 不绕过 source-proven gate

**Gate**：
- Phase 10 Slice 1/2 现有 `f64 pair` 候选不退化
- `inferred_points = 80` 不退化
- 5 道 pre-commit gate 全绿

**Risk**：
- 中：endpoint pair 对称性是历史阻塞，需要新的字段或更松匹配规则；可能引发 false-positive line
- 已知 unknown：若对称性硬阻塞 = endpoint pair 本来就不该全配对（即"端点本来就是单端"），则需 negative 收口

**Done 条件**：
- `inferred_lines >= 40` 跨 fixture
- 新增 H7CAD inferred line snapshot test
- Phase 10 Slice 3 标 complete

**预计**：3–5 session

---

### 2.4 Phase 25-B：D06 /Sheet6 反向 + text probe 映射

**目标**：D06 是唯一 attribute-fallback fixture（compact PID），与 DWG fixture 字节结构差异大。深入反向 D06 `/Sheet6` raw text probe ↔ decoded `igTextBox` 之间的映射，扩大 fixture 多样性。

**Scope**：
- ✅ Cross-fixture probe：D06 `/Sheet6` 字节 audit
- ✅ Text probe ↔ igTextBox 映射 evidence
- ✅ 若发现新 marker group cross-fixture 一致 → 触发 Phase 24 Task 24-03 reopen
- ❌ 不强行 promotion text probe 为 inferred `Text`

**Gate**：
- `d06_pid_parses_with_expected_structure_and_geometry_summary` 不退化
- `d06_text_placement_regression_keeps_text_probes_unpromoted` 不退化
- 5 道 pre-commit gate 全绿

**Risk**：
- 中：D06 是 compact PID，可能根本没有可反向的额外字段
- 已知 unknown：若反向产物完全是 DWG fixture 的子集，则该 phase = no-op

**Done 条件**：
- D06 `/Sheet6` audit inventory 完整记录
- 若发现 cross-fixture 一致 marker → 创建 Phase 24 reopen goal package
- 若无新发现 → negative analysis 收口

**预计**：1–2 session

---

## 3. Stop-And-Challenge（硬边界）

任一触发，**必须停下来与用户对齐**：

1. 任何 phase 会让 `PidPageTransform::Available` 自动出现（违反 Phase 23 guardrail）
2. 任何 phase 命名新的 `sub_kind` 字段（违反 Phase 18/19 audit-only 边界）
3. 任何 phase 在没有 IDA-confirmed 证据下创建 typed RAD class DTO（重蹈 Phase 14 GArc2d 覆辙）
4. `inferred_lines` 提升会引发任何 H7CAD false-positive line（违反 source-proven gate）
5. 任何 phase 跨 ≥3 session 仍未达成 Done 条件（IDA-style 阻塞，需切换路径）

---

## 4. 长期保留候选（暂不进入推荐路径）

| 候选 | 暂不推进理由 |
|---|---|
| 25-D parser hardening | 持续性工作，可在任何 phase 间穿插（PR #3-7 模板） |
| 25-E AGENTS.md 文档清理 | 卫生级，建议合并到 25-A 或 25-C 的同一 PR |
| 25-F PSM 0x0010 runtime hook | 需 SmartPlant 运行环境 + 动态分析工具 + 用户明确授权 |
| 25-G H7CAD visual snapshot | 跨仓 collaboration，需用户协调 H7CAD 团队 |
| 25-H Publish XML / DWG 闭环 | MDF publish 已接近交付状态，节奏与 `.pid` 反向解析不同 |
| 25-I 多 fixture 接入 | 强依赖外部样本供给，已多次被外部阻塞 |

---

## 5. 待用户确认的关键决策

1. **是否同意"P0 → 25-A → 25-C → 25-B"作为推荐执行序列？**
2. **是否在 25-A 启动前先用 plannotator 复审 25-A 的 brief.md（goal package 第一件套）？**
3. **是否允许在 25-C 中放宽 endpoint pair 对称性条件（可能引入 false-positive line）？**
4. **是否启用 25-D parser hardening 作为 25-A/25-C 之间的填充任务？**
5. **是否需要为 25-F runtime hook 准备一份风险评估文档（即使暂不执行）？**

---

## 6. 下一动作（如果计划被批准）

```
1. git diff examples/probe_phase24_top_evidence.rs  → 用户决定 commit/discard
2. 创建 goals/phase25-f64-spatial-distribution/ 五件套
3. 用 `plannotator annotate goals/phase25-f64-spatial-distribution/brief.md --gate` 做 brief gate
4. 通过 brief gate 后进入 plan / verification / blockers
5. 全部 gate 通过后开始 Slice 0–N 实施
```

---

## 7. 引用

- `task_plan.md` Phase 9–24 历史
- `CHANGELOG.md` Phase 14–24 entries
- `goals/phase15-graphic-group-records/progress.jsonl` Phase 15 final summary
- `goals/phase18-psm-0x0010-sub-record/progress.jsonl` Phase 18 audit baseline
- `goals/phase19-psm-0x0010-leading-word-audit/progress.jsonl` Phase 19 leading_word ratchet
- `goals/phase20-psm-0x0010-ida-class-identity/progress.jsonl` Phase 20 partial closeout
- `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md` Phase 24 negative evidence
- `docs/plans/2026-05-18-phase23-coordinate-page-context-plan-cn.md` Phase 23 guardrails
- `docs/plans/2026-05-18-phase24-coordinate-page-metadata-decoder-plan-cn.md` Phase 24 plan
