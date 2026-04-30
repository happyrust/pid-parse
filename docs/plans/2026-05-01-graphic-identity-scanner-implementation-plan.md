# Graphic Identity Scanner Implementation Plan

> **Date:** 2026-05-01  
> **Parent plan:** `docs/plans/2026-05-01-graphic-identity-nearby-evidence-plan.md`  
> **Goal:** implement Phase B/C for `GraphicIdentityNearby`: scan `SheetFieldXWindow`s for source-backed identity values, score them only when they resolve to the same object, and keep `object_geometry_hints` empty unless the full promotion gate is satisfied.

## 1. Current Starting Point

Completed:

- `SheetObjectIdentity`
- `SheetIdentityIndex`
- `SheetIdentityIndex::field_x_for_drawing_id`
- `sheet_identity_index_from_trailers`
- TDD validation:
  - RED: missing `sheet_identity_index_from_trailers`
  - GREEN: `identity_index_maps_field_x_to_source_ids`
  - module suite: `cargo test --lib parsers::sheet_probe -- --nocapture` -> 21 passed

Current behavior:

- indexes only DA trailers that carry `DrawingID`
- skips relationship trailers without `DrawingID`
- resolves DrawingID case-insensitively

## 2. Phase B Target API

Add investigation-only identity evidence near field-x windows.

Proposed structs:

```rust
pub struct SheetFieldXWindowIdentity {
    pub field_x: u32,
    pub offset: usize,
    pub delta_from_field: isize,
    pub kind: SheetFieldXWindowIdentityKind,
    pub value: SheetFieldXWindowIdentityValue,
    pub resolves_to_field_x: Option<u32>,
    pub resolves_to_same_object: bool,
}

pub enum SheetFieldXWindowIdentityKind {
    DrawingIdAscii,
    DrawingIdUtf16Le,
    TrailerRecordId,
    TrailerClassId,
    UnknownMarker,
}

pub enum SheetFieldXWindowIdentityValue {
    Text(String),
    U32(u32),
}
```

Add function:

```rust
pub fn field_x_window_identities(
    data: &[u8],
    windows: &[SheetFieldXWindow],
    identity_index: &SheetIdentityIndex,
) -> Vec<SheetFieldXWindowIdentity>
```

Do **not** attach identities to `SheetFieldXWindowFeatures` in the first implementation. Keep this report-side helper separate to avoid broad churn in the current PR split.

## 3. Detection Rules

### 3.1 DrawingID ASCII

Inside each window range:

- find 32-byte ASCII hex strings
- normalize to lowercase for lookup
- resolve via `SheetIdentityIndex::field_x_for_drawing_id`
- `resolves_to_same_object = resolved == window.field_x`

### 3.2 DrawingID UTF-16LE

Inside each window range:

- find 32 UTF-16LE hex code units
- normalize to lowercase for lookup
- resolve via `SheetIdentityIndex::field_x_for_drawing_id`

### 3.3 Trailer Record ID

Inside each window range:

- scan aligned and unaligned little-endian `u32`
- compare to every indexed `record_id`
- mark same-object only when matched identity's `field_x == window.field_x`

### 3.4 Trailer Class ID

Class IDs alone are weak. Include them as report evidence, but do not score them as same-object identity by themselves.

Initial rule:

- scan `u32` values that equal indexed `class_id`
- report as `TrailerClassId`
- `resolves_to_field_x = None`
- `resolves_to_same_object = false`

### 3.5 Unknown Marker

Reuse `SheetWindowMarker` only after Phase B if needed. Initial scanner should avoid treating high-entropy markers as identity.

## 4. TDD Sequence

### Test 1 - Same Object Record ID

Name:

```rust
field_x_window_identities_find_same_object_record_id
```

Fixture:

- window for `field_x=35`
- identity index contains `field_x=35, record_id=0x6009`
- bytes near the field contain `0x6009`

Expected:

- one `TrailerRecordId`
- `resolves_to_field_x == Some(35)`
- `resolves_to_same_object == true`

### Test 2 - Wrong Object Record ID

Name:

```rust
field_x_window_identities_reject_wrong_object_record_id
```

Fixture:

- window for `field_x=35`
- identity index contains another object `field_x=99, record_id=0x7001`
- bytes near the field contain `0x7001`

Expected:

- identity is reported
- `resolves_to_field_x == Some(99)`
- `resolves_to_same_object == false`

### Test 3 - DrawingID ASCII

Name:

```rust
field_x_window_identities_resolve_ascii_drawing_id_case_insensitively
```

Expected:

- 32-hex ASCII DrawingID resolves to same object regardless of case

### Test 4 - DrawingID UTF-16LE

Name:

```rust
field_x_window_identities_resolve_utf16_drawing_id
```

Expected:

- UTF-16LE 32-hex DrawingID resolves to same object

## 5. Phase C Scoring

Add scoring reason only after Phase B tests are green:

```rust
GraphicIdentityNearby {
    kind: SheetFieldXWindowIdentityKind,
    delta: isize,
}
```

Scoring:

- `+35` for same-object `DrawingIdAscii`, `DrawingIdUtf16Le`, or `TrailerRecordId`
- `0` for wrong-object identities
- `0` for class-id-only identities
- endpoint-record hits remain `-100`

Keep this as report/scoring evidence only. It does not populate `SheetObjectGeometryHint`.

## 6. Real Fixture Report

Add test:

```rust
sheet6_field_x_window_identity_report
```

Report:

- total windows
- total identities
- same-object identities
- wrong-object identities
- class-id-only identities
- top identity kinds

Expected first assertion:

- fixture may find zero identities
- `object_geometry_hints` remains `0`

Add follow-up test:

```rust
sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven
```

Expected:

- no `SheetObjectGeometryHint` population
- no `Line + Inferred`

## 7. Validation Commands

```powershell
cargo test --lib field_x_window_identities_find_same_object_record_id -- --nocapture
cargo test --lib field_x_window_identities_reject_wrong_object_record_id -- --nocapture
cargo test --lib field_x_window_identities_resolve_ascii_drawing_id_case_insensitively -- --nocapture
cargo test --lib field_x_window_identities_resolve_utf16_drawing_id -- --nocapture
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
```

## 8. Non-Goals

- No H7CAD changes.
- No endpoint line rendering.
- No `object_geometry_hints` population from identity evidence alone.
- No promotion based on class ID alone.
- No use of high-entropy unknown markers as identity proof.

## 9. Immediate Checklist

- [x] Add `SheetFieldXWindowIdentityKind` / `SheetFieldXWindowIdentityValue` / `SheetFieldXWindowIdentity`.
- [x] Add RED test for same-object record ID.
- [x] Implement minimal record-id scanner.
- [x] Add wrong-object record ID test.
- [x] Add ASCII DrawingID scanner.
- [x] Add UTF-16LE DrawingID scanner.
- [x] Add real `/Sheet6` report with `object_geometry_hints=0`.
- [x] Add scoring integration for same-object `GraphicIdentityNearby`.
- [x] Add real `/Sheet6` identity scoring report with `object_geometry_hints=0`.

## 10. Phase B Synthetic Scanner Notes

Implemented:

- `SheetFieldXWindowIdentityKind`
- `SheetFieldXWindowIdentityValue`
- `SheetFieldXWindowIdentity`
- `field_x_window_identities`
- ASCII 32-hex DrawingID scanner
- UTF-16LE 32-hex DrawingID scanner
- little-endian DA `record_id` scanner

TDD validation:

```powershell
cargo test --lib field_x_window_identities_find_same_object_record_id -- --nocapture
cargo test --lib field_x_window_identities_resolve_ascii_drawing_id_case_insensitively -- --nocapture
cargo test --lib field_x_window_identities_resolve_utf16_drawing_id -- --nocapture
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_object_geometry_hints_baseline_is_empty_until_mapping_is_proven -- --nocapture
```

Result:

- same-object `record_id`: RED then GREEN
- ASCII DrawingID: RED then GREEN
- UTF-16LE DrawingID: RED then GREEN
- wrong-object `record_id`: covered and green
- module suite: 25 passed
- `/Sheet6.object_geometry_hints == 0` guardrail: passed

Current non-goal still holds: the scanner reports identity evidence only; it does not populate `SheetObjectGeometryHint`.

## 11. Real `/Sheet6` Identity Report

Implemented:

- `sheet6_field_x_window_identity_report`

Validation:

```powershell
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
```

Observed output:

```text
field_x identity summary: fields=57, windows=6025, identities=425, same_object=11, wrong_object=414, kinds={"TrailerRecordId": 425}
```

Interpretation:

- The identity route has a real signal: 11 same-object `TrailerRecordId` hits.
- The signal is noisy: 414 wrong-object `TrailerRecordId` hits.
- Current evidence is not sufficient for promotion by itself.
- `object_geometry_hints` remains `0`.

Next scoring implication:

- `GraphicIdentityNearby` should only add score for same-object identities.
- Wrong-object identities should remain report evidence and may become a negative/diagnostic reason later.
- Promotion still requires same-object identity plus stable chunk shape, high-quality coordinate, same-chunk coordinate, and repeated support.

## 12. Phase C Scoring Notes

Implemented:

- `SheetFieldXWindowScoreReason::GraphicIdentityNearby`
- `score_field_x_window_features_with_identities`

Behavior:

- starts from existing `score_field_x_window_features`
- adds `+35` only when identity evidence:
  - resolves to the same object
  - has the same `field_x`
  - belongs to the same field-x hit offset
- wrong-object identities do not add score
- endpoint-record hits remain downranked by the base scoring path

Synthetic validation:

```powershell
cargo test --lib score_field_x_window_features_adds_graphic_identity_nearby_only_when_resolved -- --nocapture
```

Result:

- same-object `TrailerRecordId` raises score from `25` to `60`
- wrong-object `TrailerRecordId` stays at `25`

Real fixture validation:

```powershell
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
```

Observed output:

```text
graphic identity scoring summary: scores=6025, identity_supported=0, max_score=45, over_threshold=0
```

Interpretation:

- The 11 same-object identities observed in the identity report do not intersect a non-endpoint feature scoring candidate.
- `GraphicIdentityNearby` therefore does not raise any real `/Sheet6` feature score yet.
- `object_geometry_hints` remains `0`.
- This strengthens the current conclusion: `/Sheet6` still lacks source-proven object-coordinate mapping under the current promotion gate.

Post-format validation:

```powershell
rustfmt --edition 2021 src/parsers/sheet_probe.rs tests/parse_real_files.rs
cargo test --lib parsers::sheet_probe -- --nocapture
cargo test --test parse_real_files sheet6_field_x_window_identity_report -- --nocapture
cargo test --test parse_real_files sheet6_graphic_identity_scoring_keeps_object_hints_empty_until_proven -- --nocapture
```

Result:

- `sheet_probe`: 26 passed.
- identity report: passed (`same_object=11`, `wrong_object=414`).
- identity scoring report: passed (`identity_supported=0`, `max_score=45`, `over_threshold=0`).

