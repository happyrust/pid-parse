# PrimitiveLine Record Evidence Check

> Date: 2026-05-09
> Scope: Phase 14 Task 14-03 preflight for source-backed `SheetRecordKind::PrimitiveLine`.

## Conclusion

Task 14-03 is blocked by evidence, not implementation mechanics.

The current `DWG-0201GP06-01.pid /Sheet6` diagnostics prove:

- object `field_x` windows can be associated with coordinate/f64 marker evidence;
- promoted object positions can produce endpoint-pair inferred lines;
- the line-producing path is still `EndpointPair + Inferred`, not a decoded primitive line record.

No independent Sheet record has been proven to carry a primitive line's own
`start/end` payload with bounded source bytes. Therefore existing endpoint
inferred lines must not be relabeled as `SheetRecordKind::PrimitiveLine` or
`PidGeometryConfidence::Decoded`.

## Evidence Run

Commands:

```powershell
cargo test --locked -j 1 --test parse_real_files sheet6_field_x_window_features_report_chunk_shapes -- --nocapture
cargo test --locked -j 1 --test parse_real_files sheet6_top_candidate_record_dump_stays_investigation_only -- --nocapture
```

Observed signal:

- top record shape classes are object `field_x` windows, for example `(14, 38)` and `(46, 70)`;
- top feature scores are explained by `ObjectFieldResolves`, `StableChunkShape`, `StableMarkerNearby`, `RepeatedF64PairBeforeField`, and `GraphicIdentityNearby`;
- top candidate dumps remain investigation-only and are centered on object positioning evidence;
- text dumps remain rejected as binary-like text, not positioned labels.

## Boundary

A future primitive-line decoder needs a separate proof chain:

1. identify a bounded Sheet record range that is not an endpoint-pair record;
2. prove the record contains two coordinate pairs or equivalent start/end fields;
3. show the same record shape repeats across fixtures or multiple independent records;
4. emit `PidGraphicKind::Line` with `confidence = Decoded`;
5. set provenance `record_kind = SheetRecordKind::PrimitiveLine`;
6. keep endpoint-pair inferred lines under `SheetRecordKind::EndpointPair`.

## Recommended Next Step

Extend `sheet_records` with a dedicated primitive-line investigation report
instead of adding a decoded geometry DTO immediately. The report should group
non-endpoint marker records by bounded range shape and list candidate numeric
fields that could be start/end coordinates.
