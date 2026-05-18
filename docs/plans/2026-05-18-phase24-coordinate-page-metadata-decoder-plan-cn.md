---
phase: 24-coordinate-page-metadata-decoder
plan: 01
type: execute
wave: 1
runtime: other
assurance: self_checked
depends_on:
  - Phase 23 Coordinate/Page Context guardrails
files-modified:
  - src/parsers/sheet_records.rs
  - src/model.rs
  - src/schema.rs
  - src/geometry.rs
  - tests/parse_real_files.rs
  - CHANGELOG.md
  - findings.md
  - progress.md
  - task_plan.md
autonomous: false
requirements:
  - REQ-P24-01
  - REQ-P24-02
  - REQ-P24-03
  - REQ-P24-04
non_goals:
  - 不直接把 PidPageTransform::Available 接入 normalized geometry。
  - 不修改 H7CAD。
  - 不处理 Text/Symbol promotion。
  - 不修改 PSM 0x0010 audit-only surface。
hard_boundaries:
  - 只有 cross-fixture 稳定 marker group 才能进入 typed decoder 候选。
  - decoder 第一版只输出 audit/typed metadata DTO，不改变 entity 坐标。
  - 如果无法证明 width/height/origin/scale/bounds 的完整字段组，保持 transform unavailable。
escalation_triggers:
  - top evidence marker group 只覆盖单 fixture 或单 Sheet。
  - 候选字段解释需要人工确认单位或方向。
  - 需要引入新 fixture 才能达到稳定性门槛。
  - 实现会让任何 entity 产生 PidPageTransform::Available。
approval_gates:
  - promotion 到 PidPageTransform::Available 前必须单独 review。
  - 推送前必须明确授权。
anti_regression_targets:
  - Phase 23 guardrails 继续通过。
  - coordinate_top_evidence 仍可复查。
  - page_dimension_scalar_matches 当前为 0 的事实不可被忽略。
known_unknowns:
  - marker 49215 是否是真实 page metadata record 仍未证明。
  - normalized f64 pairs 是 geometry coordinate evidence 还是 transform metadata evidence 尚未区分。
  - 现有 5 fixture / 7 sheet 的 coverage 可能不足以 promotion。
ui_proof_slots: []
no_ui_proof_rationale: Parser-only / docs-only 阶段，不声明可见 UI 行为。
high_leverage_surfaces:
  - src/parsers/sheet_records.rs
  - src/geometry.rs
second_pass_required: true
closure_claim_limit: 只能声明 typed candidate / negative evidence；不能声明 page transform decoded，除非字段组和 provenance 完整达标。
parallelism_budget:
  max_concurrent_plans: 1
  safe_parallelism: []
leverage:
  lost: 继续推迟 H7CAD 可见坐标对齐。
  kept: Phase 23 no-promotion guardrail 和 top evidence evidence trail。
  gained: 为真正的 CoordinatePageMetadata decoder 建立可执行的证据筛选路径。
must_haves:
  truths:
    - top_evidence 中的候选 marker group 会被稳定性排序和审查。
    - typed candidate decoder 不会改变 normalized geometry entity 坐标。
    - 如果 evidence 不足，阶段以 negative analysis / blocker 收口。
  artifacts:
    - path: docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md
      provides: candidate marker group evidence table
    - path: tests/parse_real_files.rs
      provides: cross-fixture candidate ratchets
    - path: src/parsers/sheet_records.rs
      provides: candidate decoder or richer investigation helper
  key_links:
    - from: SheetCoordinatePageMetadataInvestigationReport.top_evidence
      to: candidate marker-group selection
      via: support/range/normalized_f64/i32/page scalar ranking
---

# Phase 24 开发方案：CoordinatePageMetadata decoder 候选筛选

## Objective

基于 Phase 23 的 `top_evidence`，筛选最稳定的 coordinate/page metadata marker
group，尝试建立第一版 typed candidate decoder 或形成明确负证据。该阶段的重点
不是让 page transform available，而是判断是否存在足够稳定的 source record 可供
后续 transform decoder 使用。

## Context

Phase 23 当前 cross-fixture 输出：

- `fixtures_seen=5`
- `sheets_seen=7`
- `coordinate_metadata_candidates=97`
- `coordinate_top_evidence=36`
- `normalized_f64_pair_count=1397`
- `page_dimension_scalar_matches=0`

这些数字说明 coordinate evidence 很丰富，但 page dimension scalar source 仍未出现。
因此 Phase 24 必须先做候选筛选和负证据记录，不能跳到 transform promotion。

## Requirements Covered

- **REQ-P24-01:** 生成 candidate marker group evidence table。
- **REQ-P24-02:** 对 top candidate 建立稳定性/覆盖度/字段完整性 gate。
- **REQ-P24-03:** 如证据足够，新增 typed candidate DTO；如证据不足，落盘 negative analysis。
- **REQ-P24-04:** 保持 Phase 23 no-promotion guardrail 全绿。

## Must-Haves

1. 每个候选 marker group 都有 fixture/sheet/support/range/numeric evidence。
2. 候选字段必须能解释 width/height/origin/scale/bounds 中至少一组完整字段，否则不 promotion。
3. `PidPageTransform::Available` 不在本阶段自动出现。
4. 结论必须写入 analysis 文档和 progress。

## Anti-Goals

- 不做 H7CAD 视觉对齐。
- 不把 f64 coordinate pairs 直接乘 page size。
- 不将单 fixture marker group 升级为 decoded metadata。
- 不实现 Text/Symbol。

## Evidence Contract

最低验证命令：

```powershell
cargo test --locked -j 1 --test parse_real_files sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion -- --nocapture
cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture
cargo test --locked -j 1 --lib geometry::tests::default_coordinate_context_keeps_page_transform_unavailable_until_promoted -- --nocapture
```

若新增 DTO / schema：

```powershell
cargo test --locked -j 1 --lib schema -- --nocapture
cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata_candidate -- --nocapture
```

全量收口：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

## Tasks

<task id="24-01" type="auto">
  <files>
    - CREATE: docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    从 `top_evidence` 输出中整理 candidate marker group evidence table。按
    marker_type、range_len、support、fixture/sheet 覆盖、normalized_f64_pairs、
    candidate_i32_pairs、page_dimension_scalar_matches 排序。记录 top 5 候选和
    rejected 理由。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --test parse_real_files sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion -- --nocapture`
  </verify>
  <done>
    Analysis doc 能说明哪个 marker group 最值得下一步 decoder 试探，或者说明没有候选达标。
  </done>
</task>

<task id="24-02" type="checkpoint:review">
  <files>
    - MODIFY: docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md
  </files>
  <action>
    对 top candidate 做 stop-and-challenge。若候选只提供 coordinate-like f64
    evidence，而没有完整 width/height/origin/scale/bounds 字段，记录为 negative
    evidence 并停止 typed decoder 实现。
  </action>
  <verify>
    - Review candidate table manually.
  </verify>
  <done>
    用户/执行者明确选择：继续 typed candidate decoder，或以 negative analysis 收口。
  </done>
</task>

<task id="24-03" type="auto">
  <files>
    - MODIFY: src/parsers/sheet_records.rs
    - MODIFY: src/model.rs
    - MODIFY: src/schema.rs
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    仅在 task 24-02 通过 review 后执行。新增 typed candidate DTO，字段名必须保持
    byte-position / evidence-oriented，例如 `candidate_f64_pairs`、`candidate_i32_pairs`、
    `marker_type`、`range_len`、`raw_payload_prefix`。不要命名为 page transform。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --lib schema -- --nocapture`
    - Run `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata_candidate -- --nocapture`
  </verify>
  <done>
    DTO 只表达 candidate evidence，不改变 normalized geometry transform。
  </done>
</task>

<task id="24-04" type="auto">
  <files>
    - MODIFY: CHANGELOG.md
    - MODIFY: findings.md
    - MODIFY: progress.md
    - MODIFY: task_plan.md
  </files>
  <action>
    同步文档和门禁结果。若 task 24-03 未执行，则明确记录 blocker / negative
    evidence；若执行，则记录 DTO 的 audit-only 边界。
  </action>
  <verify>
    - Run `git diff --check`
    - Run full workspace gates if code changed.
  </verify>
  <done>
    Phase 24 的结论不会 overclaim transform decoded。
  </done>
</task>

## Stop-And-Challenge

- Top candidate 没有跨 fixture/sheet support。
- `page_dimension_scalar_matches` 继续为 0。
- 字段解释需要猜单位、方向或 origin。
- 任何实现会让 `PidPageTransform::Available` 出现。

## Success Criteria

- 有一份候选 evidence analysis 文档。
- 有明确 proceed / stop 结论。
- Phase 23 guardrails 继续全绿。
- 不 overclaim decoded page transform。
