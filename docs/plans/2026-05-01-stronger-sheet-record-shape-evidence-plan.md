# Stronger Sheet Record-Shape Evidence Plan

> **Date:** 2026-05-01  
> **Goal:** 在不降低 promotion threshold 的前提下，为 `SheetObjectGeometryHint` 寻找更强的 Sheet source record evidence。  
> **Trigger:** `/Sheet6` 全量 endpoint `field_x` scoring 结果为 `max_score=65`, `promotable=0`，说明 `field_x + nearby coordinate + repeated delta` 仍不足以证明对象坐标归属。

## 1. Current Decision

当前不填充 `SheetObjectGeometryHint`。

原因：

- 全量 `/Sheet6` endpoint ids:
  - `fields=57`
  - `windows=6025`
  - `positive_non_endpoint=4297`
  - `endpoint_references=135`
  - `max_score=65`
  - `promotable=0`
- `max_score=65` 来自 non-endpoint + known object + coordinate candidate + repeated delta。
- 缺失的是“这个 window 是同一种 Sheet source record”的强证据，例如 marker、record length、GraphicOID-like value、稳定 header/trailer。

结论：

- 不降低阈值到 `65`。
- 不把 `nearby_coordinates` 当作 object position。
- 下一步寻找 stable record-shape evidence。

## 2. Evidence Categories To Add

### 2.1 Stable Marker Nearby

Search each non-endpoint positive window for repeated marker values at fixed offsets.

Candidate markers:

- 1-byte markers: `0x89`, `0x00`, `0x01`
- 2-byte record-type values observed in `record_type_counts`
- 4-byte discriminator-like values repeated across candidates

Acceptance:

- Same marker value appears at the same `delta` for at least 3 distinct `field_x`.
- Marker is not part of endpoint-record signature.

Score proposal:

```text
StableMarkerNearby +20
```

### 2.2 Record Length / Boundary Shape

Use existing `SheetProbeReport.candidate_boundaries` and `chunks` to see whether positive windows fall into repeatable chunk shapes.

Measurements:

- nearest chunk start/end
- field_x offset relative to chunk start
- candidate coordinate offset relative to chunk start
- chunk kind hint
- zero ratio / aligned u32 density

Acceptance:

- Same `(field_delta, coord_delta)` relative to chunk start across at least 2 distinct `field_x`.
- Chunk kind or density profile is similar.

Score proposal:

```text
StableChunkShape +25
```

### 2.3 GraphicOID-like Value

Search windows for values that match known object / drawing / graphic identifiers if any are available in decoded model fields.

Candidate sources:

- object graph `drawing_id`
- relationship DrawingID pairs
- possible GraphicOID / Representation ids in DA attributes

Acceptance:

- A nearby value links the window to the same object identity beyond plain `field_x`.
- The value appears at a stable delta across multiple objects.

Score proposal:

```text
GraphicIdentityNearby +30
```

### 2.4 Coordinate Quality Filter

Current coordinate candidates come from broad `i32, i32` scanning. Add stricter quality checks before scoring:

- reject coordinate pair if it overlaps `field_x`
- reject pairs whose neighboring dwords are endpoint-record constants
- prefer coordinates inside the same chunk as `field_x`
- prefer coordinates with bounded drawing-like range after excluding known outliers

Acceptance:

- Stricter filter reduces candidate noise but keeps existing inferred point baseline unchanged.

## 3. Implementation Phases

### Phase A: Window Feature Extraction

Add investigation-only DTO:

```rust
pub struct SheetFieldXWindowFeatures {
    pub field_x: u32,
    pub offset: usize,
    pub endpoint_record_start: Option<usize>,
    pub chunk_start: Option<usize>,
    pub chunk_end: Option<usize>,
    pub field_delta_from_chunk: Option<isize>,
    pub coordinate_delta_from_chunk: Option<isize>,
    pub stable_markers: Vec<SheetWindowMarker>,
}
```

Do not put this into `PidDocument`.

Status: implemented as `SheetFieldXWindowFeatures`, `SheetWindowMarker`, and `field_x_window_features`. Marker extraction is intentionally still empty; chunk-relative field/coordinate deltas are now available for the next evidence pass.

Tests:

- synthetic chunk-like window extracts field/coordinate deltas: done
- endpoint-record windows remain classified separately

Current real `/Sheet6` report:

```text
top chunk-shape groups:
[((10, 20), 12), ((68, 72), 11), ((26, 18), 8), ((84, 102), 6),
 ((8, 12), 5), ((14, 20), 5), ((20, 24), 5), ((20, 26), 5),
 ((2, -6), 4), ((2, 8), 4)]

top marker groups:
[((2, 393216), 28), ((2, 65536), 23), ((-10, 65536), 19),
 ((8, 65536), 18), ((-4, 65536), 17), ((20, 65536), 17),
 ((4, 6), 16), ((6, 524288), 16), ((-16, 65536), 14),
 ((2, 524288), 14)]
```

Interpretation:

- `(field_delta_from_chunk=10, coordinate_delta_from_chunk=20)` appears for 12 distinct endpoint `field_x` values.
- This is the first stronger shape signal beyond raw byte-window delta.
- Marker-like values also repeat at fixed deltas. Several values (`65536`, `524288`, `393216`, `6`) may be structural constants, but they still need filtering against endpoint signature noise.
- It is still investigation evidence only; scoring should require chunk shape plus marker/identity evidence before promotion.

### Phase B: Scoring Extension

Extend `SheetFieldXWindowScoreReason` with:

- `StableMarkerNearby`
- `StableChunkShape`
- `GraphicIdentityNearby`

Only add scores when evidence is repeated across distinct object `field_x`.

Tests:

- stable marker across 3 fields adds `+20`
- stable chunk shape across 2 fields adds `+25`
- endpoint windows never contribute support

### Phase C: Real Fixture Report

Extend existing all-endpoint scoring report:

```text
fields=...
windows=...
endpoint_references=...
positive_non_endpoint=...
max_score=...
promotable=...
top_reasons=...
top_candidate_offsets=...
```

Acceptance:

- If `promotable=0`, keep `SheetObjectGeometryHint` empty and document why.
- If `promotable>0`, inspect top candidates manually before population.

### Phase D: Population PR Gate

Populate `SheetObjectGeometryHint` only if all are true:

1. `score >= 70`
2. candidate has source-backed coordinate byte range
3. candidate carries at least one strong record-shape reason:
   - `StableMarkerNearby`
   - `StableChunkShape`
   - `GraphicIdentityNearby`
4. same shape is observed across multiple object `field_x`
5. real fixture test updates `object_geometry_hints` count from `0` to explicit number

## 4. Validation Commands

```powershell
cd D:\work\plant-code\cad\pid-parse
cargo test --lib field_x_window -- --nocapture
cargo test --test parse_real_files sheet6_all_endpoint_field_x_window_scoring_report -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
cargo test --test parse_real_files normalized_geometry_probe_baseline_on_real_fixture -- --nocapture
```

## 5. Immediate Checklist

- [x] Add window feature extraction DTOs in `sheet_probe`.
- [x] Map each field-x window to containing Sheet chunk.
- [x] Report top stable marker deltas across all positive non-endpoint windows.
- [x] Report top stable chunk-shape groups.
- [ ] Extend scoring only for repeated strong evidence.
- [ ] Keep `object_geometry_hints` empty until a real candidate crosses threshold with strong evidence.

