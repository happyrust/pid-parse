# Field-X Window Scoring Implementation Plan

> **Date:** 2026-05-01  
> **Goal:** 将 `field_x_windows` 的调查结果转成可测试的 scoring pipeline，只评估 `endpoint_record_start = None` 的窗口，并决定是否有足够证据填充 `SheetObjectGeometryHint`。  
> **Scope:** `pid-parse/src/parsers/sheet_probe.rs`、real fixture tests、investigation docs。暂不生成 `Line + Inferred`。

## 1. Current Inputs

已完成能力：

- `SheetFieldXWindow`:
  - `field_x`
  - `offset`
  - `endpoint_record_start`
  - `window_start`
  - `window_end`
  - `nearby_coordinates`
- `field_x_windows(data, field_xs, window_radius)`:
  - 能找到 raw Sheet bytes 中的 `field_x` hits。
  - 能标记 endpoint-record signature hits。
  - 能收集窗口内 coordinate candidates。
- Real fixture smoke:
  - `229`: 3 hits, 1 endpoint-signature hit
  - `326`: 19 hits, 8 endpoint-signature hits
  - `740`: 5 hits, 3 endpoint-signature hits
  - `139`: 4 hits, 2 endpoint-signature hits

核心判断：

- `endpoint_record_start = Some(_)` 的 hit 是 relationship endpoint evidence，不能填 object position。
- 只有 `endpoint_record_start = None` 的 hit 才进入 object-geometry candidate scoring。

## 2. Proposed Public Surface

新增 investigation-only scoring DTO，仍放在 `parsers::sheet_probe`，不进入 stable `PidDocument` schema：

```rust
pub struct SheetFieldXWindowScore {
    pub field_x: u32,
    pub offset: usize,
    pub score: i32,
    pub reasons: Vec<SheetFieldXWindowScoreReason>,
    pub candidate_position: Option<SheetCoordinateHint>,
}

pub enum SheetFieldXWindowScoreReason {
    EndpointRecordReference,
    ObjectFieldResolves,
    NonEndpointHit,
    CoordinateCandidateAtDelta { delta: isize },
    RepeatedDeltaAcrossFields { delta: isize, support: usize },
    StableMarkerNearby { offset_delta: isize, marker: u32 },
}
```

Keep this separate from `SheetObjectGeometryHint` until a candidate crosses promotion threshold.

Status: `SheetFieldXWindowScore`, `SheetFieldXWindowScoreReason`, and Phase A `score_field_x_windows` are implemented. The helper is still investigation-only and does not mutate `SheetGeometry`.

## 3. Phase A: Scoring Function Without Promotion

**Function:**

```rust
pub fn score_field_x_windows(
    windows: &[SheetFieldXWindow],
    object_field_xs: &std::collections::HashSet<u32>,
) -> Vec<SheetFieldXWindowScore>
```

**Rules:**

- If `endpoint_record_start.is_some()`:
  - score `-100`
  - reason `EndpointRecordReference`
  - no `candidate_position`
- If hit is not endpoint-like:
  - `+10` for `NonEndpointHit`
  - `+10` if `field_x` exists in `object_field_xs`
  - choose nearest coordinate candidate only as `candidate_position`, not as proof
  - `+5` for coordinate candidate presence
- Do not implement repeated delta scoring in Phase A; leave reason enum ready for Phase B.

**Tests:**

- endpoint-like window scores `-100`
- non-endpoint window with object field and coordinate candidate scores positive but below promotion threshold
- no score result may directly mutate `SheetGeometry.object_geometry_hints`

## 4. Phase B: Repeated Delta Grouping

**Goal:** Look for a stable relation between `field_x` offset and coordinate offset.

For non-endpoint windows:

1. For each nearby coordinate, compute:

```text
delta = coordinate.offset - field_x.offset
```

2. Group by delta across distinct `field_x`.
3. A delta is interesting only if:
   - support >= 2 distinct `field_x`
   - all candidates are non-endpoint hits
   - candidate coordinate byte ranges remain inside each window

4. Add score:
   - `+40` for `RepeatedDeltaAcrossFields`
   - `candidate_position = coordinate at winning delta`

**Tests:**

- two field_x windows with coordinate at same delta produce repeated-delta reason
- one-off delta does not cross promotion threshold
- endpoint-like windows do not contribute support

## 5. Phase C: Real Fixture Report

Add a real fixture test that prints / validates scoring summary for sampled fields:

```rust
#[test]
fn sheet6_field_x_window_scoring_reports_non_endpoint_candidates()
```

Expected behavior for now:

- soft-skip if fixture missing
- score windows for `[229, 326, 740, 139]`
- assert there is at least one non-endpoint positive candidate
- assert no score is promoted to `SheetObjectGeometryHint`
- emit summary under `--nocapture`

Do not assert exact score totals until repeated-delta logic stabilizes.

## 6. Phase D: Promotion Decision

Only after Phase B:

```text
promotable if:
  score >= 70
  candidate_position.is_some()
  RepeatedDeltaAcrossFields support >= 2
  field_x is known object field_x
  hit is not endpoint-like
```

If no candidate crosses threshold, document that result and keep `object_geometry_hints` empty.

If candidates cross threshold, implement population in a separate PR:

- `SheetProbeReport` gains mapping candidates or `SheetGeometry` construction consumes scores.
- `SheetObjectGeometryHint.position` uses the winning coordinate with byte offset.
- Real fixture baseline updates from `0` to explicit count.

## 7. Current Real Fixture Scoring Summary

Sampled endpoint ids (`229`, `326`, `740`, `139`) currently report:

```text
field_x scoring summary: total=31, positive_non_endpoint=15, endpoint_references=14, max_score=25, promotable=0
```

All `/Sheet6` endpoint field ids currently report:

```text
all endpoint field_x scoring summary: fields=57, windows=6025, positive_non_endpoint=4297, endpoint_references=135, max_score=65, promotable=0
```

Interpretation:

- The scorer now separates endpoint-record references from non-endpoint candidates.
- Positive non-endpoint candidates exist, but no sampled or all-endpoint real candidate crosses the promotion threshold.
- Repeated-delta support is implemented, and the full `/Sheet6` endpoint-field scan reaches `max_score=65`, still below the `70` threshold.
- `object_geometry_hints` remains empty after scoring, as required.
- Current decision: do not populate `SheetObjectGeometryHint` from Sheet6 field-x windows yet.

## 8. Validation Commands

```powershell
cd D:\work\plant-code\cad\pid-parse
cargo test --lib field_x_window -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_probe_finds_sample_endpoint_ids -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_scoring_reports_non_endpoint_candidates -- --nocapture
cargo test --test parse_real_files sheet6_all_endpoint_field_x_window_scoring_report -- --nocapture
```

## 9. Immediate Checklist

- [x] Add `SheetFieldXWindowScore` and reason enum.
- [x] Add `score_field_x_windows` Phase A.
- [x] Test endpoint-like windows score `-100`.
- [x] Test non-endpoint candidate scores positive but does not promote.
- [x] Add real fixture scoring report test.
- [x] Implement repeated-delta grouping only after Phase A is green.
- [x] Decide whether sampled real candidates cross promotion threshold: current answer is no (`promotable=0`).
- [x] Expand sampling to all `/Sheet6` endpoint ids before deciding the entire fixture has no promotable mapping.
- [x] Decide whether all-endpoint `/Sheet6` candidates cross promotion threshold: current answer is no (`promotable=0`, `max_score=65`).
- [ ] Investigate whether threshold should stay at `70` or whether scoring needs a stronger positive evidence category before promotion.

