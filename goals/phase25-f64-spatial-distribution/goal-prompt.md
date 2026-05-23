# Codex Goal Prompt: Phase 25-A normalized_f64_pair 空间分布挖掘

本目录下的 goal package 用于启动 Phase 25-A。准备执行时，把下面 `/goal`
段落粘到 Codex：

```text
/goal 开始 Phase 25-A：在 **不 promotion page transform** 的前提下，把 cross-fixture 1397 个 normalized_f64_pair 从 "raw coordinate evidence" 升级为 H7CAD 可消费的 spatial topology evidence。本 phase 是 **read-only spatial analysis**，不改变 entity 的 confidence / kind / coords，不引入新 promotion 通道，不接入新 fixture。Phase 23 / Phase 24 guardrails 必须全部保持。

用 `goals/phase25-f64-spatial-distribution/` 作为 durable source of truth：

- 读 `brief.md`：目标、背景、上下文、约束、非目标、Ask Before、Done Means（AC1-AC8）
- 跟 `plan.md`：solution overview、cluster 算法选型、数据流、新增 DTO、新 API、Slice A-H
- 跑 `verification.md`：API 单元测试、cross-fixture ratchet、Phase 23/24 guardrail 回归、5 道 pre-commit gate
- 遇到 `blockers.md` 的 Stop-And-Ask 条件时立即暂停、写 `progress.jsonl`、等用户

执行顺序：

1. **Slice A**：新增 `examples/probe_phase25_f64_spatial.rs`，跑 `cargo run --release --example probe_phase25_f64_spatial -- --json`，dump 每 sheet 的 cluster_count / bounding_box / nearest-neighbor 直方图。每发现 append progress.jsonl `slice_complete` entry。
2. **Slice A decision point**：若所有 fixture 的 cluster_count <= 1，触发 negative 路径，跳到 Slice E (with `uniform_distribution=true`) → Slice G → Slice H → partial_complete。
3. **Slice B**：新增 `SheetSpatialAnalysisReport` DTO + `coordinate_pair_spatial_analysis` API 在 `src/parsers/sheet_records.rs`，含 6-10 unit tests 覆盖 determinism + edge cases (empty / single pair / uniform / clustered / panic-safety)。
4. **Slice C**：`src/model.rs` 接入 `SheetGeometry::spatial_analysis` field（`#[serde(default)]`）+ `From` impl + JsonSchema derive；`src/schema.rs` 默认 schema needle ratchet 加 `spatial_analysis`。8 schema tests 全绿。
5. **Slice D**：`src/streams/cluster.rs` pipeline 填充 spatial_analysis；同步 `src/cfb/reader.rs` + `src/geometry.rs` 6 个 test fixture init。cargo build --workspace 通过。
6. **Slice E**：`tests/parse_real_files.rs` 新增 `spatial_analysis_cross_fixture_baseline` test，ratchet 每 fixture cluster_count >= N（N 由 Slice A 实际数据决定）。Phase 23/24 guardrail tests 显式跑确认。
7. **Slice F**（**仅** non-negative 路径）：`src/geometry.rs::build_normalized_geometry` 对 80 inferred_points attach `cluster=<id>` 到 `provenance.note`。格式：原 note + " | cluster=<id>"。Outlier 不 attach。
8. **Slice G**：写 `docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md`，结构：(1) 数据获取方法, (2) 全局统计, (3) 每 fixture × 每 sheet cluster 报告, (4) cross-fixture 形态对比, (5) ASCII 散点示意 (≤ 20 行/sheet), (6) 与 Phase 23/24 evidence 的关系, (7) Known unknowns, (8) Phase 25-C 工作衔接。若 negative 路径，标题改为 "negative result"。
9. **Slice H**：跑 5 道 pre-commit gate 确认无意外退化：cargo build --locked --workspace --all-targets / cargo test --locked --workspace --all-targets / cargo clippy --locked --workspace --all-targets -- -D warnings / cargo fmt --all -- --check / bash .github/scripts/check-missing-docs.sh。

每个 Slice 完成后 append progress.jsonl `slice_complete` entry，含命令 / 输出摘要 / AC 编号。

不要做：

- 不修改 Phase 14–24 任何 baseline / DTO / collection。
- 不引入新 `PidGraphicKind` variant。
- 不引入新 promotion 通道。
- 不修改 `PidPageTransform` enum 或 promotion gate。
- 不做跨 fixture 坐标对齐。
- 不接入新 fixture。
- 不修改 H7CAD 仓库代码。
- 不 commit / push，除非用户明确授权。

完成时（non-negative 路径）append：

{"type":"goal_complete","timestamp":"...","phase":"25-A","work_type":"spatial_analysis","cluster_baseline":{...},"cluster_total":<sum>,"provenance_enhanced_entities":<n>,"analysis_doc":"docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md","phase14_baselines_preserved":true,"phase15_audit_preserved":true,"phase16_jstyle_preserved":true,"phase17_primitive_arc_removed":true,"phase18_audit_preserved":true,"phase19_leading_word_preserved":true,"phase20_partial_preserved":true,"phase21_d06_preserved":true,"phase22_micro_preserved":true,"phase23_guardrails_green":true,"phase24_negative_preserved":true,"gates":"5/5 green","new_promotion_types":false,"src_code_changes":true,"test_changes":true}

若 negative 路径完成，append `partial_complete`（详见 verification.md）。

然后暂停等用户签收。Phase 25-C 实施需要单独 /goal 启动。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 含 initial scaffold entry
- [ ] 已读 `docs/plans/2026-05-23-phase25-next-development-plan-cn.md`（Phase 25+ 候选评估）
- [ ] 已读 `docs/plans/2026-05-18-phase23-coordinate-page-context-plan-cn.md`（Phase 23 guardrails）
- [ ] 已读 `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md`（Phase 24 negative evidence）
- [ ] 已确认 working tree clean（除新增的 5 件套 + progress.jsonl 外）
- [ ] 已确认 5 道 pre-commit gate 当前全绿（baseline）
- [ ] 首个执行动作是 Slice A probe，不是直接写 DTO
