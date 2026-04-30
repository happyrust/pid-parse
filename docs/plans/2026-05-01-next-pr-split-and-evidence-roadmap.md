# PID Geometry Next PR Split And Evidence Roadmap

> **Date:** 2026-05-01  
> **Goal:** 将当前 PID real-geometry 工作拆成可 review 的 PR，并明确下一条证据路线：继续保持 `SheetObjectGeometryHint` 为空，直到 object-coordinate mapping 被 source-proven。  
> **Current decision:** `/Sheet6` 经过 field-x window、stable chunk/marker、coordinate-quality filters 与 `GraphicIdentityNearby` 后仍无 promotable object-coordinate mapping (`max_score=45`, `over_threshold=0`)。

## 1. Current State

已完成并验证：

- H7CAD 能显示 PID 中的 inferred Sheet coordinate points。
- `pid-parse` 有 `NormalizedPidGeometry` contract。
- `SheetGeometry.object_geometry_hints` contract 已存在，但默认空。
- `/Sheet6` endpoint/object field investigation 已完成：
  - endpoint ids 是 object DA `field_x`
  - endpoint records 是 semantic relationship evidence，不是坐标
  - current field-x scoring after filters: `max_score=45`, `promotable=0`
- `GraphicIdentityNearby` 已完成 Phase A-C：
  - identity report: `same_object=11`, `wrong_object=414`
  - identity scoring: `identity_supported=0`, `max_score=45`, `over_threshold=0`
  - all-Sheet identity scoring: `sheets=1`, `identity_supported=0`, `over_threshold=0`
- H7CAD 仍不渲染 `Line + Inferred`，这是正确的。
- 中文执行清单：`docs/plans/2026-05-01-h7cad-pid-pr-execution-checklist-cn.md`
- 路线图：`docs/diagrams/h7cad-pid-real-geometry-roadmap.svg` / `.png`

验证命令已通过：

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_features_report_chunk_shapes -- --nocapture
cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
cargo test --test parse_real_files all_sheets_graphic_identity_scoring_report_keeps_object_hints_empty -- --nocapture

cd D:\work\plant-code\cad\H7CAD-pid-real-geometry-display
cargo test -p H7CAD pid_bundle -- --nocapture
```

## 2. Recommended PR Split

### PR 1: Normalized Geometry Contract

Purpose:

- Ship stable `NormalizedPidGeometry` and CLI/schema/report contract.

Contents:

- `src/geometry.rs`
- schema exposure
- `pid_inspect --geometry-json`
- normalized geometry real-fixture baseline
- tests that keep text/endpoint evidence as `ProbeOnly Unknown`

Acceptance:

- `pid_inspect --geometry-json` emits normalized probe entities.
- Coordinate hints become inferred positioned points.
- No line rendering yet.

### PR 2: H7CAD Inferred Point Rendering

Purpose:

- Consume normalized geometry in H7CAD and render inferred points only.

Contents:

- H7CAD `PID_GEOMETRY_POINTS` rendering
- H7CAD point screenshot fix
- PID open message geometry stats
- PID tab fit targeting main PID layers

Acceptance:

- H7CAD opens sample PID and reports inferred geometry points.
- Unknown/probe-only evidence remains non-rendered.
- No endpoint-line rendering.

### PR 3: Sheet6 Evidence Inventory And Guardrails

Purpose:

- Document why endpoint records cannot be promoted to lines yet.

Contents:

- `/Sheet6` evidence inventory docs
- endpoint/object graph cross-check
- `object_geometry_hints` empty baseline test
- `SheetObjectGeometryHint` DTO slot

Acceptance:

- Real fixture test asserts `/Sheet6.object_geometry_hints == 0`.
- Docs record coordinate hints vs endpoint records byte ranges.
- No H7CAD behavior change beyond PR 2.

### PR 4: Field-X Window Investigation Tools

Purpose:

- Add investigation-only parser helpers for future geometry reverse engineering.

Contents:

- `field_x_windows`
- endpoint-record signature marking
- `score_field_x_windows`
- repeated-delta scoring
- real fixture scoring reports

Acceptance:

- All helper output is investigation-only.
- No DTO population.
- No `Line + Inferred`.

### PR 5: GraphicIdentityNearby Investigation

Purpose:

- Add source-backed identity evidence near Sheet field-x windows.

Contents:

- `SheetIdentityIndex`
- `field_x_window_identities`
- `GraphicIdentityNearby` scoring reason
- identity report and identity scoring report

Acceptance:

- `/Sheet6` identity report observes `same_object=11`, `wrong_object=414`.
- `/Sheet6` identity scoring remains `identity_supported=0`, `max_score=45`, `over_threshold=0`.
- `object_geometry_hints` stays empty.

### Folded Into PR 4: Strong Evidence And Coordinate Quality Filters

Purpose:

- Prevent false promotion from stable-but-structural record patterns.

Contents:

- `field_x_window_features`
- stable chunk-shape support
- stable marker support with denylist
- feature-based scoring
- coordinate quality filters
- docs proving `promotable=0` after filters

Acceptance:

- `sheet6_field_x_window_features_report_chunk_shapes` reports `max_score=45`, `promotable=0`.
- `object_geometry_hints` stays empty.
- Broad `coordinate_hints` baseline remains unchanged.

## 3. Next Evidence Route

### Route A: Other Fixtures

The current real fixture only contributes one endpoint-linked Sheet to the all-Sheet identity report:

- `sheets=1`
- same result as `/Sheet6`: `identity_supported=0`, `over_threshold=0`

Next step is another real PID fixture if available.

Goal:

- find a Sheet where object-coordinate records are less ambiguous
- check whether same-object identities intersect non-endpoint coordinate candidates elsewhere

### Route B: Text Placement Before Lines

Text may be easier than line endpoints:

- current text probes are `Unknown + ProbeOnly`
- look for text run + nearby coordinate + stable text record marker
- if proven, promote to `Text + Inferred` before `Line + Inferred`

This may give H7CAD more visible real geometry without needing endpoint coordinate mapping.

Detailed plan:

- `docs/plans/2026-05-01-text-placement-evidence-plan-cn.md`

Current Phase A report:

- `Sheet6 text window report: text_runs=9, coordinates=64, candidates=121, same_chunk=25, coordinate_quality_passed=2`
- after text-quality scoring: `text_quality_passed=0`, `max_score=-50`, `over_threshold=0`
- normalized geometry still has no `PidGraphicKind::Text`
- top text runs look like likely binary false positives, so current `/Sheet6` has no promotable Text placement candidate

### Route C: Graphic Identity Refinement

Only continue identity promotion work if:

- same-object identity intersects a non-endpoint feature window
- candidate coordinate passes `is_high_quality_coordinate_candidate`
- coordinate and field_x are inside the same chunk
- repeated support exists across distinct field_x values

Current `/Sheet6` does not satisfy this.

## 4. Do Not Do Yet

- Do not render endpoint records as lines.
- Do not use topology layout positions as real CAD geometry.
- Do not lower promotion threshold to make tests pass.
- Do not populate `SheetObjectGeometryHint` from `field_x + nearby coordinate` alone.

## 5. Immediate Checklist

- [x] Decide PR split order: PR1 contract, PR2 H7CAD points, PR3 guardrails, PR4 field-x evidence, PR5 identity evidence.
- [x] Generate Chinese execution checklist.
- [x] Generate roadmap SVG/PNG.
- [ ] Execute manual hunk staging or temporary branch split after explicit user authorization.
- [ ] Run focused validation for each PR before commit.
- [x] Start next evidence route with all-Sheet scoring reports.
- [x] Draft Text placement evidence route plan.
- [x] Implement Text placement Phase A report helper.
- [x] Add Text placement Phase B text-quality filter and scoring.
- [ ] Continue Text placement only with better text extraction or another fixture.

