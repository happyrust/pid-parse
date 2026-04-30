# Coordinate Quality Filter Plan

> **Date:** 2026-05-01  
> **Goal:** 在 `SheetObjectGeometryHint` promotion 前过滤结构性 coordinate-like pairs，防止把 record constants 当成 CAD 坐标。  
> **Trigger:** feature scoring 已出现 `score=70` 的候选，但 top positions 多为 `(1536,0)`, `(6,8)`, `(0,65536)`, `(65537,-65535)` 等结构值。

## 1. Current Problem

当前 scoring 已能找到稳定 chunk-shape 与 marker 组合：

```text
feature scoring summary: max_score=70, promotable=70
```

但 top-candidate inspection 显示候选坐标不是可靠 CAD 几何：

```text
field_x=35  position=(1536, 0)
field_x=229 position=(1536, 0)
field_x=239 position=(6, 8)
field_x=440 position=(0, 65536)
field_x=433 position=(65537, -65535)
```

这些更像 record constants / flags / length fields，而不是图纸坐标。因此：

- 不填充 `SheetObjectGeometryHint`。
- 不降低阈值。
- 先实现 coordinate quality filters。

## 2. Filter Categories

### 2.1 Structural Value Filter

Reject coordinate-like pairs when either component is clearly structural:

```text
0
1
2
4
6
8
16
65535
65536
65537
-65535
-65536
```

Also reject if either component is a power of two or close to a known binary boundary:

```text
abs(value).is_power_of_two()
abs(value) in [32768, 65536, 131072, 262144, 524288, 1048576]
```

### 2.2 Tiny Pair Filter

Reject pairs where both components are tiny record-like values:

```text
abs(x) <= 16 && abs(y) <= 16
```

Examples:

- `(6, 8)`
- `(1, 8)`
- `(0, 6)`

### 2.3 Axis-Zero Structural Filter

Reject pairs where one axis is zero and the other is a structural value:

```text
x == 0 && structural(y)
y == 0 && structural(x)
```

Examples:

- `(1536, 0)`
- `(0, 65536)`

### 2.4 Chunk Boundary Filter

Reject candidate positions that cross the containing chunk boundary.

Required:

```text
chunk_start <= position.offset
position.offset + 8 <= chunk_end
```

### 2.5 Endpoint Signature Neighbor Filter

Reject coordinate-like pairs whose neighboring values include endpoint-record constants:

```text
0x00000006 discriminator
0x0002 type tag
0x0001 delimiter
```

This prevents endpoint reference records from leaking into position candidates.

## 3. Implementation Plan

### Phase A: Pure Predicate

Add:

```rust
pub fn is_high_quality_coordinate_candidate(hint: &SheetCoordinateHint) -> bool
```

Start with only value-based filters:

- structural value filter
- tiny pair filter
- axis-zero structural filter
- small-id pair filter (`abs(x) < 1000 && abs(y) < 1000`)

Tests:

- rejects `(1536,0)`: done
- rejects `(6,8)`: done
- rejects `(0,65536)`: done
- rejects `(65537,-65535)`: done
- accepts known existing broad coordinate hint like `(1200,-450)`: done

### Phase B: Apply To Candidate Selection

Update feature/scoring candidate selection:

- `nearest_coordinate`
- `repeated_delta_candidate`
- `field_x_window_features`

Rule:

- Only high-quality candidates can become `candidate_position`.
- Broad `coordinate_hints` baseline remains unchanged. This filter applies only to object-mapping/scoring candidates.

### Phase C: Real Fixture Re-evaluation

Re-run:

```powershell
cargo test --test parse_real_files sheet6_field_x_window_features_report_chunk_shapes -- --nocapture
cargo test --test parse_real_files sheet6_all_endpoint_field_x_window_scoring_report -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
```

Expected:

- feature scoring `promotable` dropped from `70` to `24`.
- adding small-id pair filtering drops feature scoring `promotable` further to `9`.
- adding packed-field / 256-aligned structural filtering drops feature scoring to `max_score=45`, `promotable=0`.
- broad all-endpoint scoring still has `promotable=0`.
- `object_geometry_hints` stays `0`.
- If any promotable candidates remain, inspect top positions again.

Current post-filter top positions still look structural:

```text
(35328,1536)
(53504,1536)
(196727,196712)
(131067,970)
(131067,1067)
```

The small-id pair filter removed obvious field-id pairs such as `(54,466)` and `(56,664)`.

Packed-field / 256-aligned filtering removed the remaining top structural candidates. Current decision: no Sheet6 object-coordinate mapping is proven yet.

## 4. Promotion Gate Update

After filters:

`SheetObjectGeometryHint` may only be populated when:

1. `candidate_position` passes `is_high_quality_coordinate_candidate`.
2. `score >= 70`.
3. Candidate has `StableChunkShape`.
4. Candidate has `StableMarkerNearby` or future `GraphicIdentityNearby`.
5. Candidate position byte range is inside the same chunk as `field_x`.
6. Real fixture baseline is intentionally updated from `0` to explicit count.

## 5. Immediate Checklist

- [ ] Implement `is_high_quality_coordinate_candidate`.
- [x] Implement `is_high_quality_coordinate_candidate`.
- [x] Add focused unit tests for rejected structural pairs.
- [x] Apply filter to feature/scoring candidate selection only.
- [x] Re-run real fixture scoring reports.
- [ ] Keep `SheetGeometry.object_geometry_hints` empty unless high-quality candidates remain after manual inspection.
- [x] Add object-id range filter for small id-like coordinate pairs.
- [x] Add coordinate distribution / packed-field filter for remaining structural-looking candidates.
- [x] Confirm post-filter scoring has `promotable=0`.

