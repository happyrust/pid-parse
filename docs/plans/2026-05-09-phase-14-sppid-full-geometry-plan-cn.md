---
phase: 14-sppid-full-geometry
plan: 01
type: execute
runtime: cursor
assurance: self_checked
depends_on:
  - docs/plans/2026-05-09-phase-12-page-transform-text-extraction-plan-cn.md
  - docs/plans/2026-05-09-phase-13-da-text-association-plan-cn.md
autonomous: true
requirements:
  - GEOM-FIXTURE-BASELINE
  - GEOM-RECORD-INVENTORY
  - GEOM-TYPED-DECODERS
  - GEOM-PAGE-TRANSFORM
  - GEOM-COVERAGE-API
non_goals:
  - 不声明完整编辑/回写 SPPID 几何。
  - 不把 endpoint topology 或 probe-only text 伪装成 decoded CAD primitive。
  - 不改动 Publish XML/MDF writer fidelity 主线。
hard_boundaries:
  - `Sheet*` coverage 只有在 typed decoder 消费真实 record bytes 后才可升级。
  - decoded geometry 必须携带 stream、byte range、record kind、confidence。
  - 未证明稳定的 shape 只能进入 probe/investigation 报告。
escalation_triggers:
  - 新 fixture 的 record shape 与现有 5E/FA/CE family 冲突。
  - decoded primitive 需要依赖未解析单位或页面 transform 才能解释。
  - 某类 geometry 只能靠语义图/topology 推断，缺少 Sheet byte provenance。
approval_gates:
  - 新增私有真实 fixture 入仓或改动 fixture 路径策略前需确认。
  - 将 `Sheet*` coverage 从 `IdentifiedOnly` 升级为 `PartiallyDecoded` 前需审查。
anti_regression_targets:
  - 现有 3 个 line-producing fixture 继续产出 inferred lines。
  - `PidGraphicKind::decoded_sheet_record_kind()` 与 provenance `record_kind` 保持一致。
  - probe-only / unsupported bytes 不丢失、不误渲染。
closure_claim_limit: 只能声明本计划覆盖的 geometry decoder wave 完成，不能声明 SPPID 全格式完整解析。
parallelism_budget:
  max_concurrent_plans: 1
  safe_parallelism: []
leverage:
  lost: 先保守扩展 record inventory，短期不会直接提升所有图元显示数量。
  kept: 复用现有 Probe/Decode 分层、NormalizedPidGeometry contract、fixture inventory 与 byte provenance 规则。
  gained: 将“全几何解析”拆成可验证的 decoder waves，避免一次性大改造成不可审查风险。
must_haves:
  truths:
    - 每个 decoded geometry entity 都有可追溯 Sheet record provenance。
    - 每个未 decoded 的主要几何类别都有可解释 inventory 或 probe 诊断。
    - 覆盖率升级由 fixture 和 byte evidence 驱动，而不是文档声明。
  artifacts:
    - path: tests/parse_real_files.rs
      provides: 真实 fixture geometry inventory 与 regression gates。
    - path: src/parsers/sheet_probe.rs
      provides: 当前 Sheet record shape evidence 与 f64 marker extraction。
    - path: src/parsers/sheet_records.rs
      provides: 新 typed Sheet record inventory / decoder 候选模块。
    - path: src/geometry.rs
      provides: NormalizedPidGeometry decoded entity projection。
    - path: src/inspect/coverage.rs
      provides: Sheet* coverage 分类与说明。
  key_links:
    - from: src/parsers/sheet_records.rs
      to: src/model.rs
      via: SheetGeometry DTO / SheetRecordKind。
    - from: src/model.rs
      to: src/geometry.rs
      via: SheetStream.geometry。
    - from: src/geometry.rs
      to: downstream renderers
      via: NormalizedPidGeometry JSON schema。
---

# Phase 14：SPPID Sheet 全几何元素解析执行计划

## Objective

把 `pid-parse` 从当前 “Sheet probe + object hint + endpoint inferred line” 推进到逐类解析 SPPID `Sheet*` 中的主要几何元素，并通过 `NormalizedPidGeometry` 输出可渲染、可审计、带来源证明的实体。

本计划不追求一次性“全格式完成”，而是建立一个可重复推进的 decoder wave：先量化缺口，再形成 record-shape inventory，然后逐类升级为 typed geometry。

## Current Baseline

- 公共几何契约已存在：`PidGraphicKind::{Line, Polyline, Arc, Circle, Point, Text, SymbolInstance, Unknown}`。
- Sheet schema 契约已存在：`SheetRecordKind::{PrimitiveLine, PrimitivePolyline, PrimitiveCircle, PrimitiveArc, SymbolPlacement, TextPlacementStyle, EndpointPair, CoordinatePageMetadata, Unknown}`。
- 当前稳定输出以 inferred/probe 为主：text probe、i32 coordinate hint、endpoint pair、object geometry hint、endpoint inferred line。
- 最新 focused 回归确认：
  - `DWG-0201GP06-01.pid`：`inferred_points=117`，`inferred_lines=49`。
  - 5 个 registry fixture 中 3 个 line-producing：`DWG-0201GP06-01.pid`、`DWG-0202GP06-01.pid`、publish DWG fixture。
  - fixture registry 当前 `registered=5`，目标仍是 `8+`。
- 当前 CI 风险：`cargo clippy --locked -j 1 --test parse_real_files -- -D warnings` 失败，原因是 `tests/parse_real_files.rs` 中有 1 个 unused variable、1 个 manual_map、1 个 len_zero。执行本计划前应先清掉这些 preflight blockers。

## Requirements Covered

- `GEOM-FIXTURE-BASELINE`：fixture inventory 必须能量化 decoded/inferred/probe-only geometry。
- `GEOM-RECORD-INVENTORY`：Sheet record shape 必须有 offset/range/type/field candidates/nearby evidence。
- `GEOM-TYPED-DECODERS`：主要 geometry primitive 逐类以 typed decoder 输出。
- `GEOM-PAGE-TRANSFORM`：units/page transform 解码后才能升级 coordinate context。
- `GEOM-COVERAGE-API`：coverage 与 schema 只能随真实 decoder 能力升级。

## Anti-Goals

- 不做语义级几何编辑或 writer round-trip。
- 不用 relationship endpoint 拓扑反推 CAD primitive。
- 不用 text run 误识别直接生成 positioned text。
- 不为追求数量降低 `Decoded` / `Inferred` / `ProbeOnly` 边界。

## Evidence Contract

每个 decoded entity 必须同时满足：

1. `PidGraphicKind` 有具体 payload。
2. `PidGraphicProvenance.stream_path` 指向 Sheet stream。
3. `PidGraphicProvenance.byte_range` 有界且覆盖 source record。
4. `PidGraphicProvenance.record_kind` 与 `PidGraphicKind::decoded_sheet_record_kind()` 一致。
5. `confidence == Decoded` 只在字段语义和坐标单位足够稳定时使用；否则保持 `Inferred` 或 `ProbeOnly`。

计划级验证至少包含：

- `cargo test --locked -j 1 --test parse_real_files geometry_fixture_inventory_reports_normalized_geometry_counts -- --nocapture`
- `cargo test --locked -j 1 --test parse_real_files dwg0201_produces_inferred_endpoint_lines -- --nocapture`
- `cargo test --locked --workspace --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo rustdoc --lib --locked -- -W missing-docs`

## Common Pitfalls

- 把 endpoint pair line 当成 primitive line decoded：endpoint pair 证明对象连接，不等价于 CAD line record。
- 把 Sheet text run 当成 positioned text：当前 text quality gate 仍显示多为二进制误识别。
- 过早升级 `Sheet*` coverage：必须等 typed decoder 消费真实 record bytes 后再升级。
- 混淆 source coordinates、page coordinates 和 renderer viewport coordinates。

## Stop-And-Challenge

- 如果新 decoder 只能通过 topology/object graph 推断位置，停止并保持 probe-only。
- 如果 record bytes 无法界定完整 range，停止并补 inventory/diagnostic。
- 如果新增 fixture 与当前 marker family 不兼容，停止并先写 investigation plan。

## Tasks

<task id="14-01" type="auto">
  <files>
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    清理当前 focused clippy blocker，并把 geometry inventory 输出扩展为 decoded/inferred/probe-only 的 per-kind 计数，包括 line、polyline、circle、arc、text、symbol、unknown。保持现有 5 fixture registry 与 8+ target gap 显式输出。
  </action>
  <verify>
    - Run `cargo clippy --locked -j 1 --test parse_real_files -- -D warnings`
    - Run `cargo test --locked -j 1 --test parse_real_files geometry_fixture_inventory_reports_normalized_geometry_counts -- --nocapture`
  </verify>
  <done>
    parse_real_files focused clippy 通过，fixture inventory 能稳定显示每类 geometry 的 decoded/inferred/probe-only 数量。
  </done>
</task>

<task id="14-02" type="auto">
  <files>
    - CREATE: src/parsers/sheet_records.rs
    - MODIFY: src/parsers/mod.rs
    - MODIFY: src/parsers/sheet_probe.rs
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    新增 Sheet record-shape inventory 模块，保守抽取 record offset/range、marker/type id、field_x candidates、nearby text/coordinate/f64 evidence。先作为 investigation surface，不输出 decoded geometry。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --lib parsers::sheet_records -- --nocapture`
    - Run `cargo test --locked -j 1 --test parse_real_files sheet_record_shape_inventory_reports_geometry_candidates -- --nocapture`
  </verify>
  <done>
    每个真实 Sheet fixture 都能输出 bounded record-shape inventory；未识别 shape 有原因和 byte range，不改变 `NormalizedPidGeometry` 行为。
  </done>
</task>

<task id="14-03" type="auto">
  <files>
    - MODIFY: src/parsers/sheet_records.rs
    - MODIFY: src/model.rs
    - MODIFY: src/geometry.rs
    - MODIFY: src/schema.rs
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    实现第一类 typed geometry decoder：优先选择 source-backed PrimitiveLine，而不是 endpoint inferred line。decoder 必须从 Sheet record bytes 解析 start/end，并填充 SheetGeometry DTO；`build_normalized_geometry()` 输出 `PidGraphicKind::Line` with `Decoded` only when record kind、byte range、坐标字段都闭环。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --test parse_real_files primitive_line_decoder_emits_decoded_lines_with_provenance -- --nocapture`
    - Run `cargo test --locked -j 1 --lib schema::tests::normalized_geometry_schema_exposes_graphic_contract`
  </verify>
  <done>
    至少一个 fixture 出现 decoded primitive line，且现有 endpoint inferred lines 保持不退化。
  </done>
</task>

<task id="14-04" type="auto">
  <files>
    - MODIFY: src/parsers/sheet_records.rs
    - MODIFY: src/model.rs
    - MODIFY: src/geometry.rs
    - MODIFY: tests/parse_real_files.rs
  </files>
  <action>
    在 record-shape inventory 证明稳定后，按 TextPlacementStyle、SymbolPlacement、PrimitivePolyline、PrimitiveCircle/PrimitiveArc 的顺序增加 decoder spike。每类先写 red test 和 panic-safety assertions，再决定是否可进入 decoded 输出；不满足 evidence contract 的类别保留 probe/unsupported 诊断。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --test parse_real_files text_symbol_and_curve_decoder_inventory_reports_supported_and_unsupported_shapes -- --nocapture`
    - Run `cargo test --locked -j 1 --test parser_panic_safety`
  </verify>
  <done>
    每个主要 geometry 类别都有 decoded 输出或明确 unsupported/probe-only 诊断；不会因为坏 bytes panic。
  </done>
</task>

<task id="14-05" type="auto">
  <files>
    - MODIFY: src/parsers/sheet_records.rs
    - MODIFY: src/model.rs
    - MODIFY: src/geometry.rs
    - MODIFY: src/inspect/coverage.rs
    - MODIFY: tests/parse_real_files.rs
    - MODIFY: docs/sppid/v0.10.x-status.md
    - MODIFY: README.md
    - MODIFY: CHANGELOG.md
  </files>
  <action>
    解码 CoordinatePageMetadata 的最小 units/page transform contract，升级 `PidCoordinateContext` 中可证明的部分；当 typed decoder 覆盖达到门槛后，将 coverage note 从 pure Sheet probe 更新为 supported/unsupported geometry category 清单。
  </action>
  <verify>
    - Run `cargo test --locked -j 1 --test parse_real_files coordinate_page_metadata_upgrades_context_without_mixing_spaces -- --nocapture`
    - Run `cargo test --locked -j 1 --test parse_real_files decoded_geometry_provenance_record_kind_matches_payload_kind -- --nocapture`
    - Run `cargo test --locked --workspace --all-targets`
    - Run `cargo clippy --locked --workspace --all-targets -- -D warnings`
    - Run `cargo fmt --all -- --check`
    - Run `cargo rustdoc --lib --locked -- -W missing-docs`
  </verify>
  <done>
    Sheet coverage 文档和 API contract 与真实 decoder 能力一致；全量门禁通过。
  </done>
</task>

## Success Criteria

- fixture inventory 能持续量化每类 geometry 的 decoded / inferred / probe-only / unsupported 计数。
- 至少第一类 non-endpoint primitive geometry 以 typed decoder 输出 decoded entity。
- 每个 decoded entity 的 `record_kind` 与 `PidGraphicKind::decoded_sheet_record_kind()` 一致。
- `Sheet*` coverage note 明确列出 supported/unsupported geometry categories。
- 全量 test、clippy、fmt、rustdoc 门禁通过。

## Planning Notes

- 本仓库没有 `.planning/SPEC.md`、`.planning/ROADMAP.md` 或 GSDD helper；本计划按 `gsdd-plan` 的目标倒推和任务 XML 结构落到项目现有 `docs/plans` 体系。
- 当前最紧急的执行前置不是新 decoder，而是先修复 `parse_real_files` focused clippy blocker，确保后续计划不会建立在红色门禁上。
- Phase 13 的 DA text association 调查显示 promoted field_x 能对上 DA trailer，但当前 `ItemTag` / `Name` 等 tag text 关联为 0；因此 text rendering 不应作为第一类 decoded geometry。
