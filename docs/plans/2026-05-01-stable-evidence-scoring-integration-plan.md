# Stable Evidence Scoring Integration Plan

> **Date:** 2026-05-01  
> **Goal:** 将 stable chunk-shape 与 stable marker 信号纳入 `score_field_x_windows`，但继续阻止无强证据的 `SheetObjectGeometryHint` promotion。  
> **Trigger:** `/Sheet6` 已出现重复 chunk-shape 与 marker groups，但这些信号仍可能包含结构常量或 endpoint signature 噪声。

## 1. Current Evidence

已完成：

- `field_x_windows`:
  - raw Sheet bytes 中定位 `field_x`
  - 标记 `endpoint_record_start`
  - 收集 `nearby_coordinates`
- `score_field_x_windows`:
  - endpoint reference: `-100`
  - non-endpoint + object field + coordinate candidate: positive score
  - repeated raw delta: `+40`
- `field_x_window_features`:
  - 映射到 containing `SheetChunk`
  - 输出 `field_delta_from_chunk`
  - 输出 `coordinate_delta_from_chunk`
  - 抽取 marker-like aligned `u32`

真实 `/Sheet6` report：

```text
all endpoint field_x scoring:
fields=57, windows=6025, positive_non_endpoint=4297,
endpoint_references=135, max_score=65, promotable=0

top chunk-shape groups:
((10, 20), 12), ((68, 72), 11), ((26, 18), 8), ((84, 102), 6)

top marker groups:
((2, 393216), 28), ((2, 65536), 23), ((-10, 65536), 19),
((8, 65536), 18), ((4, 6), 16)
```

Current decision:

- 不降低 `score >= 70` threshold。
- 不填充 `SheetObjectGeometryHint`。
- 先过滤强信号里的噪声，再集成 scoring。

## 2. Noise Filters Before Scoring

### 2.1 Endpoint Signature Filter

Already available:

- `SheetFieldXWindow.endpoint_record_start`

Rule:

- Any feature/window with `endpoint_record_start.is_some()` must not contribute support to stable shape or marker groups.

### 2.2 Structural Constant Filter

Do not score marker groups if the value is too generic:

```text
value <= 16
value.is_power_of_two() && value <= 1_048_576
65536      // 0x00010000
393216     // 0x00060000
524288     // 0x00080000
```

Rationale:

- These values look like shifted/sliced record constants or low-cardinality structure markers.
- They can help investigation reports, but should not count as strong identity evidence by themselves.

### 2.3 Coordinate Overlap Filter

Already partly available:

- `nearest_coordinate` ignores coordinate pairs overlapping `field_x`.

Next:

- stable shape support should only count features whose `candidate_position` is inside the same chunk as `field_x`.
- reject candidate coordinate if it crosses chunk boundary.

## 3. Scoring Extensions

### 3.1 StableChunkShape

Add reason:

```rust
StableChunkShape {
    field_delta: isize,
    coordinate_delta: isize,
    support: usize,
}
```

Score:

```text
+25 when support >= 3 distinct field_x
```

Why support >= 3:

- current report has many groups; requiring 3 reduces one-off coincidences.
- top groups have support 12/11/8, so useful candidates survive.

### 3.2 StableMarkerNearby

Add reason:

```rust
StableMarkerNearby {
    delta: isize,
    value_u32: u32,
    support: usize,
}
```

Score:

```text
+20 when support >= 3 distinct field_x
and marker value is not in structural constant denylist
```

Do not score marker alone. Marker is only strong if paired with:

- `StableChunkShape`, or
- future `GraphicIdentityNearby`.

### 3.3 Promotion Gate

Keep `SheetObjectGeometryHint` empty unless:

```text
score >= 70
candidate_position.is_some()
has StableChunkShape
has StableMarkerNearby or GraphicIdentityNearby
endpoint_record_start is None
```

This prevents `65`-point repeated-delta candidates from promoting without additional record identity evidence.

## 4. Implementation Phases

### Phase A: Pure Grouping Helpers

Add helpers that return group support only:

```rust
stable_chunk_shape_support(features) -> BTreeMap<(isize, isize), usize>
stable_marker_support(features) -> BTreeMap<(isize, u32), usize>
```

Tests:

- endpoint features excluded
- generic marker values are present in report but excluded from score support
- chunk shape counts distinct `field_x`, not windows

### Phase B: Score Reasons

Extend `SheetFieldXWindowScoreReason`:

- `StableChunkShape`
- `StableMarkerNearby`

Tests:

- stable chunk shape adds `+25`
- generic marker does not add `+20`
- non-generic marker with support adds `+20`
- marker alone cannot mark candidate promotable

### Phase C: Real Fixture Report

Extend report output:

```text
strong_chunk_shape_candidates=...
strong_marker_candidates=...
max_score_after_strong_evidence=...
promotable_after_strong_evidence=...
```

Expected initial result:

- `StableChunkShape` may increase `max_score`.
- `StableMarkerNearby` may remain `0` if top markers are denied as generic constants.
- `promotable` should likely remain `0` unless a non-generic marker emerges.

### Phase D: Decide Population

If `promotable=0`:

- keep `SheetObjectGeometryHint` empty
- document no object coordinate mapping proven yet
- next investigation moves to GraphicOID / identity evidence

If `promotable>0`:

- manually inspect top candidates
- add synthetic promotion test
- update real fixture baseline from `0` to explicit count
- only then populate `SheetObjectGeometryHint`

## 5. Validation Commands

```powershell
cd D:\work\plant-code\cad\pid-parse
cargo test --lib stable_chunk_shape -- --nocapture
cargo test --lib stable_marker -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_features_report_chunk_shapes -- --nocapture
cargo test --test parse_real_files sheet6_all_endpoint_field_x_window_scoring_report -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
```

## 6. Immediate Checklist

- [x] Add stable chunk-shape support helper.
- [x] Add stable marker support helper with structural constant denylist.
- [x] Add `StableChunkShape` score reason.
- [x] Add `StableMarkerNearby` score reason.
- [x] Extend real fixture scoring report with strong evidence totals.
- [ ] Keep `object_geometry_hints` baseline at `0` unless promotion gate is fully satisfied.
- [x] Add manual/top-candidate inspection before any DTO population because `promotable=70` is too broad.
- [ ] Add coordinate quality filters for structural coordinate-like pairs before any DTO population.

## 7. Current Filtered Support Report

After introducing `stable_chunk_shape_support` and `stable_marker_support`, the real fixture report is:

```text
top chunk-shape groups:
[((10, 20), 12), ((68, 72), 11), ((26, 18), 8), ((84, 102), 6),
 ((8, 12), 5), ((14, 20), 5), ((20, 24), 5), ((20, 26), 5),
 ((2, -6), 4), ((2, 8), 4)]

top marker groups:
[((-28, 3194542878), 21), ((10, 76), 13),
 ((-10, 3670148), 12), ((-6, 10092794), 12),
 ((8, 4980752), 12), ((16, 33619968), 12),
 ((-30, 3676202437), 11), ((-26, 1070382696), 11),
 ((-14, 3676202437), 11), ((-10, 1070382696), 11)]

feature scoring summary:
max_score=45, promotable=0 after coordinate quality filters

top feature scores include:
(none at score >= 70)
```

Interpretation:

- The tightened denylist removes low-value constants and shifted powers of two.
- Remaining top markers are higher-entropy values and are better candidates for record identity evidence.
- Feature scoring no longer reaches the nominal promotion threshold after coordinate quality filters.
- Current decision: keep `SheetObjectGeometryHint` empty for `/Sheet6`; no object-coordinate mapping is proven yet.

