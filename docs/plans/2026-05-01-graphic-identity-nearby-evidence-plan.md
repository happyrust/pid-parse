# Graphic Identity Nearby Evidence Plan

> **Date:** 2026-05-01  
> **Goal:** find stronger object-to-geometry evidence than `field_x + nearby coordinate` by requiring a nearby identity value that resolves to the same object.  
> **Constraint:** this is investigation-only until the identity link is source-proven. `object_geometry_hints` stays empty unless the promotion gate below is satisfied.

## 1. Why This Route

Current `/Sheet6` evidence is intentionally insufficient:

- endpoint records connect relationship `field_x` values to object `field_x` values
- field-x windows can find nearby coordinate-like pairs
- stable chunk-shape / marker signals exist
- coordinate quality filters remove structural false positives
- final feature scoring result: `max_score=45`, `promotable=0`

The missing evidence is identity:

- a nearby value that ties a Sheet byte window to the same object beyond the raw `field_x`
- examples: `DrawingID`, trailer `record_id`, `class_id`, GraphicOID-like values, representation ids

## 2. Candidate Identity Sources

### 2.1 Object Graph / DA Trailer

Use existing parsed provenance:

- object `field_x`
- object `drawing_id`
- DA trailer `record_id`
- DA trailer `class_id`
- relationship endpoint links

These are source-backed and can be indexed by `field_x`.

### 2.2 Sheet Window Markers

Use existing `SheetWindowMarker` candidates from `field_x_window_features`.

Marker filtering already removes obvious structural constants:

- zero
- tiny constants
- low powers of two
- known generic values like `65_536`, `393_216`, `524_288`

The remaining high-entropy markers are candidates, not proof.

### 2.3 GraphicOID-Like Values

`NormalizedPidGeometry` has an optional `graphic_oid`, but current code does not yet source-prove it from Sheet records.

Treat a GraphicOID-like value as valid only if:

- it is found near the same `field_x` window
- it repeats in a stable delta or stable chunk shape
- it resolves to an object, relationship, representation, or DrawingID-backed record

Do not promote a value just because it looks high entropy.

## 3. Proposed DTOs

Add investigation-only structs in `src/parsers/sheet_probe.rs`:

```rust
pub struct SheetFieldXWindowIdentity {
    pub field_x: u32,
    pub offset: usize,
    pub delta_from_field_x: isize,
    pub kind: SheetFieldXWindowIdentityKind,
    pub value: String,
    pub resolves_to_same_object: bool,
}

pub enum SheetFieldXWindowIdentityKind {
    DrawingIdAscii,
    DrawingIdUtf16Le,
    TrailerRecordId,
    TrailerClassId,
    GraphicOidLike,
    UnknownMarker,
}
```

Then extend `SheetFieldXWindowFeatures` with:

```rust
pub nearby_identities: Vec<SheetFieldXWindowIdentity>,
```

If this creates too much churn for PR 4, keep it as a separate PR 5 after current PR split lands.

## 4. Implementation Phases

### Phase A - Identity Index

Build a helper from parsed object graph / DA attributes:

- `field_x -> drawing_id`
- `field_x -> record_id`
- `field_x -> class_id`
- `drawing_id -> field_x`

Acceptance:

- pure helper unit test with synthetic objects
- no Sheet byte scanning yet

### Phase B - Sheet Window Identity Scanner

For each `SheetFieldXWindow`:

- scan the bounded window for:
  - little-endian `record_id`
  - little-endian `class_id`
  - ASCII 32-hex DrawingID
  - UTF-16LE 32-hex DrawingID
  - high-entropy marker values already surfaced by `marker_candidates`
- mark whether each identity resolves to the same `field_x`

Acceptance:

- synthetic test proves same-object identity is reported
- synthetic test proves wrong-object identity is not treated as support

### Phase C - Scoring Integration

Add scoring reason:

```rust
GraphicIdentityNearby {
    kind: SheetFieldXWindowIdentityKind,
    delta: isize,
}
```

Suggested scoring:

- `+35` when identity resolves to the same object
- `+15` when the same identity shape repeats across at least 3 distinct `field_x` values
- `0` for unresolved high-entropy markers
- never override `EndpointRecordReference = -100`

Acceptance:

- real fixture report prints:
  - total identities
  - same-object identities
  - repeated identity shapes
  - max score
  - promotable count
- `object_geometry_hints` remains `0` unless every promotion condition below is met

### Phase D - Promotion Gate

Only populate `SheetObjectGeometryHint` when all are true:

1. `field_x` resolves to a known object.
2. candidate coordinate passes `is_high_quality_coordinate_candidate`.
3. candidate coordinate is inside the same chunk as the `field_x` hit.
4. window is not an endpoint-record reference.
5. stable chunk shape support exists for at least 3 distinct `field_x` values.
6. `GraphicIdentityNearby` resolves to the same object.
7. same promotion shape appears across at least 2 distinct objects, or the same object appears in multiple independently bounded Sheet records.

Until then, reports only.

## 5. Tests To Add

Unit tests:

- `identity_index_maps_field_x_to_source_ids`
- `field_x_window_identities_find_same_object_record_id`
- `field_x_window_identities_reject_wrong_object_record_id`
- `score_field_x_window_features_adds_graphic_identity_nearby_only_when_resolved`

Real fixture report tests:

- `sheet6_field_x_window_identity_report`
- `sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven`

Expected first outcome:

- reports may find identity-like markers
- `object_geometry_hints` should still stay empty unless same-object identity is proven

## 6. Validation Commands

```powershell
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
```

## 7. Non-Goals

- Do not parse or render line geometry from endpoint records.
- Do not use topology layout as CAD geometry.
- Do not treat high-entropy marker repetition as identity unless it resolves to object provenance.
- Do not populate `SheetObjectGeometryHint` from unresolved markers.

## 8. Immediate Checklist

- [x] Locate existing object provenance helpers that expose `field_x`, `drawing_id`, `record_id`, and `class_id`.
- [x] Add a pure identity-index helper.
- [ ] Add synthetic scanner tests before touching real fixture reports.
- [ ] Add `/Sheet6` identity report with `promotable=0` as the default expectation.
- [ ] Revisit promotion only if same-object identity is source-proven.

## 9. Phase A Implementation Notes

Implemented:

- `SheetObjectIdentity`
- `SheetIdentityIndex`
- `SheetIdentityIndex::field_x_for_drawing_id`
- `sheet_identity_index_from_trailers`

Behavior:

- indexes DA trailers that carry `DrawingID`
- keeps `field_x`, `record_id`, `class_id`, and `drawing_id`
- resolves DrawingID case-insensitively
- skips trailers without `DrawingID` so relationship trailers do not masquerade as object identities

TDD validation:

```powershell
cargo test --lib identity_index_maps_field_x_to_source_ids -- --nocapture
cargo test --lib parsers::sheet_probe -- --nocapture
```

Result:

- RED first: missing `sheet_identity_index_from_trailers`
- GREEN: focused test passed
- module suite: 21 passed

