---
phase: 23-coordinate-page-context
plan: 01
type: execute
wave: 1
runtime: other
assurance: self_checked
depends_on:
  - Phase 20 partial closeout
  - Phase 21 D06 coverage closeout
  - Phase 22 D06 text-placement regression
files-modified:
  - src/geometry.rs
  - src/parsers/sheet_records.rs
  - src/model.rs
  - src/schema.rs
  - tests/parse_real_files.rs
  - docs/prd-pid-parse-current-state.md
  - docs/architecture-guide.md
  - CHANGELOG.md
  - findings.md
  - progress.md
autonomous: true
requirements:
  - REQ-P23-01
  - REQ-P23-02
  - REQ-P23-03
  - REQ-P23-04
non_goals:
  - 不实现 typed PSM 0x0010 DTO。
  - 不命名 sub_kind，不新增 reference resolver。
  - 不把 coordinate-page metadata investigation 结果升级为 decoded transform。
  - 不把 D06 raw text probes 提升为 inferred Text。
  - 不改 Publish XML 管线。
hard_boundaries:
  - 保持 Phase 18/19 audit-only 0x0010 边界。
  - 保持 PidPageTransform::Unavailable，直到有完整 source record / scalar source / decoded semantics。
  - 不改变现有 normalized geometry entity 坐标值，除非 Slice C 证据达标并另开实现切片。
  - 不提交私有 fixture、dlls、IDA 数据库或 .superdesign 目录。
escalation_triggers:
  - 发现 page transform 需要新 fixture 或新 IDA instance 才能证明。
  - 需要把 template-derived page size 当作 transform source。
  - 需要修改 H7CAD 渲染坐标或 viewport fitting 行为。
  - 任一 Phase 14-22 geometry / schema / D06 ratchet 退化。
approval_gates:
  - 修改 H7CAD 工作树前需要明确授权。
  - 推送提交前需要明确授权。
  - 引入 decoded CoordinatePageMetadata DTO 前需要 review 证据表。
anti_regression_targets:
  - D06 relationship fallback 保持 10 unresolved relationships。
  - D06 text-placement raw probes 继续 no-promotion。
  - PSM 0x0010 leading_word 继续 audit-only。
  - Normalized geometry warnings 继续明确 units/page transform unavailable。
known_unknowns:
  - Sheet coordinate/page metadata 的完整 source record 尚未定位。
  - f64 normalized coordinates 到 page/model coordinates 的 transform 方向尚未 decoded。
  - 现有 5 fixture 覆盖仍不足以扩大 promotion。
ui_proof_slots: []
no_ui_proof_rationale: 这是 parser / CLI / docs 阶段，不声明可见 UI 行为；H7CAD 对齐留到后续独立阶段。
high_leverage_surfaces:
  - src/geometry.rs coordinate contract
  - src/parsers/sheet_records.rs investigation report
second_pass_required: true
closure_claim_limit: 只能声明 coordinate/page context guardrail 和 report hardening 完成；不能声明 page transform decoded 或 H7CAD 坐标对齐完成。
parallelism_budget:
  max_concurrent_plans: 1
  safe_parallelism: []
leverage:
  lost: 暂缓 Text/Symbol promotion 和 H7CAD 视觉对齐，先增加规划与 guardrail 成本。
  kept: 保留 Probe/Decode 分层、现有 fixture ratchet、normalized geometry public contract。
  gained: 为后续 transform decoder、H7CAD page-space rendering 和 Text/Symbol source-proven promotion 建立证据门槛。
must_haves:
  truths:
    - page_dimensions_mm 可以存在，但不能让 page transform 自动 available。
    - coordinate-page metadata report 能汇总 top evidence，供下一步 reverse engineering 使用。
    - transform promotion gate 有测试和文档约束。
    - 下游文档明确 source/page/viewport 坐标边界。
  artifacts:
    - path: tests/parse_real_files.rs
      provides: coordinate/page focused ratchets and cross-fixture evidence checks
    - path: src/parsers/sheet_records.rs
      provides: coordinate-page metadata investigation report
    - path: src/geometry.rs
      provides: PidCoordinateContext and PidPageTransform contract
    - path: docs/prd-pid-parse-current-state.md
      provides: consumer-facing current state
  key_links:
    - from: src/parsers/sheet_records.rs
      to: tests/parse_real_files.rs
      via: coordinate_page_metadata_investigation_report
    - from: src/geometry.rs
      to: tests/parse_real_files.rs
      via: build_normalized_geometry
    - from: docs/prd-pid-parse-current-state.md
      to: H7CAD consumers
      via: documented geometry contract
---

# Phase 23 详细开发方案：Coordinate/Page Context 收敛与 Transform Guardrail

## Objective

本阶段把当前 PID 解析进度从“已有坐标证据但 transform 未 decoded”的状态，
推进到“坐标上下文 contract 清晰、cross-fixture evidence 可复查、promotion
gate 可测试”的状态。它是 H7CAD 坐标对齐、CoordinatePageMetadata typed
decoder、Text/Symbol source-proven rendering 的前置阶段。

## Context

- `.planning/` 体系不存在；本计划按 `gsdd-plan` 结构写入仓库现有
  `docs/plans/` 体系。
- Phase 20 partial closeout：PSM `0x0010` 已确认 GUID
  `1D1928C0-0000-0000-C000-000000000046`，但 human class name、Read/DoIO
  sequence、sub-kind discriminator 未恢复。
- Phase 21：D06 relationship fallback、D06 Sheet audit 已完成。
- Phase 22：D06 text-placement regression 已完成，raw text probes 继续
  no-promotion。
- 当前代码已具备 `NormalizedPidGeometry.page_dimensions_mm`、
  `PidCoordinateContext`、`PidPageTransform` 和
  `coordinate_page_metadata_investigation_report`，但 page transform 仍保持
  unavailable。

## Requirements Covered

- **REQ-P23-01:** 锁定当前 coordinate context baseline，防止 page size evidence
  被误升为 transform。
- **REQ-P23-02:** 增强 cross-fixture coordinate metadata report，使 top
  evidence 可由测试输出直接复查。
- **REQ-P23-03:** 定义并测试 transform promotion gate。
- **REQ-P23-04:** 更新下游文档，明确 source/page/viewport 坐标边界。

## Must-Haves

1. `page_dimensions_mm` 与 `PidPageTransform::Unavailable` 可以同时存在，并被
   focused test 锁住。
2. cross-fixture report 输出 marker/range/support/i32/f64/page scalar 等 top
   evidence。
3. `PidPageTransform::Available` 的 promotion gate 写入代码注释、测试和文档。
4. 文档明确 H7CAD / JSON consumer 不应把 source coordinates 当 viewport pixels。
5. 全阶段不得新增不具备 source-proven evidence 的 decoded transform。

## Anti-Goals

- 不继续 Phase 20 IDA 反向。
- 不实现 typed `0x0010` DTO。
- 不把 `leading_word` 改名为 `sub_kind`。
- 不实现 Text/Symbol promotion。
- 不修改 H7CAD 工作树。

## Hard Boundaries

- `PidPageTransform::Available` 只能在满足完整证据 gate 后出现。
- `coordinate_page_metadata_investigation_report` 的输出仍是 probe/investigation
  surface，不是 decoded DTO。
- 所有新增 tests 必须 soft-skip 缺失 fixture，不能把私有样本变成 hard
  requirement。
- 不改变 Publish XML、writer passthrough、PSM `0x0010` JSON surface。

## Evidence Contract

完成声明必须同时满足：

- focused tests 通过：
  - `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture`
  - `cargo test --locked -j 1 --test parse_real_files non_sheet_stream_page_metadata -- --nocapture`
  - `cargo test --locked -j 1 --test parse_real_files sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion -- --nocapture`
- schema / geometry unit tests 通过：
  - `cargo test --locked -j 1 --lib geometry::tests::available_page_transform_json_exposes_bounds_and_matrix -- --nocapture`
- 全量门禁通过：
  - `cargo build --locked --workspace --all-targets`
  - `cargo test --locked --workspace --all-targets`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `cargo rustdoc --lib --locked -- -W missing-docs`

## Common Pitfalls

- 把 template name 推断出的 `page_dimensions_mm` 当成真实 transform。
- 因为 f64 值域在 0-1 范围内，就直接执行 `x * width_mm` / `y * height_mm`。
- 把 non-Sheet scalar hits 当作完整 width/height/source record。
- 让 H7CAD 在 renderer 层猜测单位，绕过 parser contract。
- 顺手把 D06 text probes 或 `0x0010` payload 语义化。

## Stop-And-Challenge

- 如果找到疑似完整 transform record，但只覆盖单 fixture，停止并记录证据表，
  不直接 promotion。
- 如果实现需要修改 `PidDocument` public schema，先补 schema impact list。
- 如果 existing focused tests 与计划假设冲突，以测试事实为准，更新方案。
- 如果需要新 fixture 或新 IDA instance，停止等用户授权。

## Approval Gates

- 修改 H7CAD 工作树前必须确认。
- 推送到远端前必须确认。
- 把 investigation result 升级为 typed decoded DTO 前必须做独立 review。

<checks>
<plan_check>
checker: self
checker_runtime: other
status: passed
blocking: false
notes: ".planning 体系不存在；已按 gsdd-plan schema 迁移到 docs/plans。计划覆盖 REQ-P23-01..04，任务均有 runnable verify，硬边界明确禁止 transform overclaim。"
</plan_check>
</checks>

## Tasks

<task id="23-01" type="auto">
  <files>
    - MODIFY: tests/parse_real_files.rs
    - MODIFY: src/geometry.rs
  </files>
  <action>
    Hardening 当前 coordinate context baseline。补强或新增 focused tests，明确
    `NormalizedPidGeometry.page_dimensions_mm` 只代表 template-derived page-size
    evidence；所有实体的 `PidCoordinateContext.page_transform` 在当前证据下仍为
    `Unavailable`。必要时收紧 warning 文案，表达“有页面尺寸不等于有 transform”。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture`
    - Run `cargo test --locked -j 1 --test parse_real_files non_sheet_stream_page_metadata -- --nocapture`
    - Run `cargo test --locked -j 1 --lib geometry::tests -- --nocapture`
  </verify>
  <done>
    page dimensions 与 transform unavailable 的组合被测试锁定；没有实体因为
    template/scalar evidence 获得 `PidPageTransform::Available`。
  </done>
</task>

<task id="23-02" type="auto">
  <files>
    - MODIFY: src/parsers/sheet_records.rs
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    增强 `coordinate_page_metadata_investigation_report` 的 top evidence 输出。
    对 cross-fixture top candidates 汇总 marker type、range length、support、
    i32 pair count、normalized f64 pair count、page-dimension scalar match count、
    example offset 和 bounded hex prefix。保持所有 candidates 带
    `probe_only_no_coordinate_page_metadata_promotion` note。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --test parse_real_files sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion -- --nocapture`
    - Run `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture`
  </verify>
  <done>
    test output 可直接用于人工复查 top coordinate/page metadata evidence，且 report
    不会让 transform promotion 发生。
  </done>
</task>

<task id="23-03" type="auto">
  <files>
    - MODIFY: src/geometry.rs
    - MODIFY: src/model.rs
    - MODIFY: src/schema.rs
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    定义 transform promotion gate。把 `PidPageTransform::Available` 的最低证据
    写入 doc comments，并补 schema / JSON contract 测试。新增 guardrail test：
    仅 page dimensions、scalar hits、normalized f64 pairs 或 template evidence
    都不足以使 transform available。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --lib geometry::tests::available_page_transform_json_exposes_bounds_and_matrix -- --nocapture`
    - Run `cargo test --locked -j 1 --lib schema -- --nocapture`
    - Run `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata -- --nocapture`
  </verify>
  <done>
    Promotion gate 在代码注释、schema contract 和 fixture guardrail 中一致；
    executor 不能再从 partial evidence 静默 overclaim page transform。
  </done>
</task>

<task id="23-04" type="auto">
  <files>
    - MODIFY: docs/prd-pid-parse-current-state.md
    - MODIFY: docs/architecture-guide.md
    - MODIFY: CHANGELOG.md
    - MODIFY: findings.md
    - MODIFY: progress.md
    - MODIFY: task_plan.md
  </files>
  <action>
    同步下游契约文档。说明当前 normalized geometry 保留 source coordinates，
    `page_dimensions_mm` 是 page-size evidence，`PidPageTransform::Unavailable`
    是真实状态；H7CAD / JSON consumer 不应自行猜测 source/page/viewport 映射。
    同步 changelog、findings、progress、task_plan。
  </action>
  <verify>
    - Run `git diff --check`
    - Run `cargo fmt --all -- --check`
  </verify>
  <done>
    文档消费者能清楚知道当前坐标语义边界；阶段进度记录与实现状态一致。
  </done>
</task>

<task id="23-05" type="auto">
  <files>
    - MODIFY: progress.md
  </files>
  <action>
    执行最终门禁并记录结果。若任何 gate 失败，停止并把失败写入 progress；
    不声明 Phase 23 完成。
  </action>
  <verify>
    - Run `cargo build --locked --workspace --all-targets`
    - Run `cargo test --locked --workspace --all-targets`
    - Run `cargo clippy --locked --workspace --all-targets -- -D warnings`
    - Run `cargo fmt --all -- --check`
    - Run `cargo rustdoc --lib --locked -- -W missing-docs`
  </verify>
  <done>
    5 道门禁全部通过，progress 记录 gate evidence；Phase 23 仅声明
    Coordinate/Page Context hardening 完成，不声明 transform decoded。
  </done>
</task>

## Verification

先跑 focused tests，后跑 workspace gates。若 focused tests 发现当前事实与计划不一致，
优先更新方案或 stop-and-challenge，不进入全量门禁。

## Success Criteria

- `PidPageTransform` overclaim 被测试阻止。
- cross-fixture coordinate metadata evidence 可从测试输出中复查。
- geometry public contract 文档与代码一致。
- Phase 20/21/22 边界保持不变。
- 全量门禁通过。

## High-Leverage Review

- `src/geometry.rs` 与 `src/parsers/sheet_records.rs` 是高杠杆 surface。
- `second_pass_required: true`，因为错误的 transform contract 会影响 H7CAD、
  JSON consumer 和后续 typed decoder。

## Leverage Review

- Lost: 暂缓可见 UI 对齐和 Text/Symbol promotion。
- Kept: 保留已验证的 Probe/Decode 分层、fixture gates 和 schema contract。
- Gained: 获得可执行的 transform promotion gate，为后续 decoder/H7CAD 阶段降风险。

## Notes

- 本计划是执行前计划文件，不代表已执行 Phase 23。
- 任何提交/推送仍需要用户明确授权。
- 如果后续用户要求“继续执行”，从 task `23-01` 开始。
