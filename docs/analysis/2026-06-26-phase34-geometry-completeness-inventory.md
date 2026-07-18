# Phase 34 Geometry Completeness Inventory

> Date: 2026-06-26  
> Scope: Slice 34-A, read-only inventory. No parser, schema, writer, bundle
> confidence, or geometry emission change is made by this note.

## Summary

The current implementation has solid coverage for the main decoded Sheet
geometry families, but it is not yet a complete vendor geometry decoder.

Covered decoded / typed-audit Sheet families:

| Type code | Current identity | Current status |
|---:|---|---|
| `0x3FE6` | `GLine2d` SmartPlant wrapper | Decoded |
| `0x0018` | `igLine2d` | Decoded |
| `0x0084` | `igLineString2d` | Decoded |
| `0x005E` | `igPoint2d` | Decoded |
| `0x004D` | `igTextBox` | Decoded |
| `0x00CE` | `igSymbol2d` | Decoded, with conservative reject handling |
| `0x0030` | `JStyleOverride` annotation | Decoded; not an arc |
| `0x00FA` | `GraphicGroup` | TypedAudit / audit-only |
| `0x0010` | sub-record / attribute fragment family | TypedAudit |

Current remaining cross-fixture PSM candidates from the Phase 34 plan are:

| Type code | IDA / matrix identity | Hits | Fixtures | Phase 34-A classification |
|---:|---|---:|---:|---|
| `0x0013` | `igBoundary2d` | 20 | 3 | StructuralCandidate / NeedsReader |
| `0x003D` | `igSmartFrame2d` | 12 | 6 | StructuralCandidate / NeedsReader |
| `0x0020` | `igRectangle2d` | 4 | 3 | GeometryCandidate / NeedsByteLayoutProof |

## Fixture Scope Gap

The first completeness gap found by this inventory was the fixture list itself.
Before the follow-up normalization, existing sources used three slightly
different fixture sets:

| Source | Fixture count | Includes | Excludes / mismatch |
|---|---:|---|---|
| `data-model.md` fixture snapshot matrix | 6 | `d06`, `nonascii-process-1`, `dwg0201`, `dwg0202`, `publish-a01`, `publish-dwg0202` | Current byte-audit/spec snapshot scope |
| `tests/parse_real_files.rs::geometry_fixture_cases()` | 5 | `dwg0201`, `dwg0202`, non-ASCII process fixture, `publish-a01`, `publish-dwg0202` | Excludes `D06.pid` |
| `examples/probe_psm_type_code_histogram.rs` and `probe_psm_undecoded_shapes.rs` | 5 | `dwg0201`, `dwg0202`, non-ASCII process fixture, `D06.pid`, `publish-a01` | Excludes `publish-dwg0202` |

Phase 34 should normalize these scopes before claiming a quantified full-Sheet
geometry status. Otherwise a decoder slice could appear complete in one probe
set while missing a fixture used by byte-audit or normalized geometry tests.

Recommended Phase 34 fixture baseline:

- `test-file/DWG-0201GP06-01.pid`
- `test-file/DWG-0202GP06-01.pid`
- `test-file/工艺管道及仪表流程-1.pid`
- `test-file/D06.pid`
- `test-file/export-test/publish-data/A01/A01.pid`
- `test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid`

The geometry fixture target remains 8 available fixtures, so the current local
set is still at least two fixtures short for broad geometry confidence.

Follow-up normalization on 2026-06-26 updated the two Phase 34 PSM probe
examples and `geometry_fixture_cases()` to use the same 6-fixture baseline
listed above. The normalized histogram now reports `0x0013` 20 hits / 3
fixtures, `0x003D` 12 hits / 6 fixtures, and `0x0020` 4 hits / 3 fixtures. The
remaining fixture gap is now sample size, not inconsistent scope.

## Quantified Current Coverage

The 6-fixture byte-audit matrix last recorded in the spec-kit data model shows
whole-file coverage ratios from `0.66524816` to `0.88816800`. These are not
Sheet-only ratios, but they show that the parser has moved beyond early
single-digit coverage and still preserves significant raw/audit/probe regions.

| Fixture id | Total bytes | Consumed bytes | Leftover bytes | Coverage ratio |
|---|---:|---:|---:|---:|
| `d06` | 69,579 | 54,676 | 14,903 | 0.78581180 |
| `nonascii-process-1` | 211,094 | 174,374 | 36,720 | 0.82604903 |
| `dwg0201` | 223,326 | 198,351 | 24,975 | 0.88816800 |
| `dwg0202` | 206,431 | 175,856 | 30,575 | 0.85188760 |
| `publish-a01` | 63,211 | 42,051 | 21,160 | 0.66524816 |
| `publish-dwg0202` | 206,579 | 175,982 | 30,597 | 0.85188717 |

The D06 real-fixture ratchet gives one concrete normalized-geometry baseline:

| D06 normalized geometry bucket | Count |
|---|---:|
| Total entities | 97 |
| Decoded polylines | 6 |
| Decoded points | 10 |
| Decoded texts | 4 |
| Decoded symbols | 2 |
| Decoded annotations (`JStyleOverride`) | 3 |
| Inferred points | 64 |
| Inferred lines | 0 |
| ProbeOnly unknowns | 8 |

This supports the current classification: decoded primitives exist and are
useful, but inferred / probe-only evidence remains part of the product surface.

## Remaining Sheet Byte Hotspots

The after-trace Sheet leftover inventory shows that broad unknown Sheet bytes
were already reduced by wiring existing typed/audit decoders into byte-audit.
Remaining hotspots are narrower:

- Nested `JSite204/Sheet6` cluster-family stream remains ownership-gated rather
  than a top-level Sheet decoder target.
- Several `0x00CE igSymbol2d` leftovers are conservative rejects, not automatic
  evidence for loosening `decode_igsymbols`.
- `0x0084 igLineString2d` rejects and small numeric/no-header byte windows need
  bounded review before parser changes.
- `0x0013` appears in after-trace leftovers, but remains boundary/structural
  until reader or controlled fixture evidence proves drawable semantics.

## Candidate Classification

| Candidate | Classification | Reason | Next allowed action |
|---|---|---|---|
| `0x0020 igRectangle2d` | GeometryCandidate | Known IGDS drawable type, appears in 2 fixture families, only 3 current hits | Inspect payload byte layout and neighbor context; implement only if corner/transform fields are proven |
| `0x0013 igBoundary2d` | StructuralCandidate | Boundary / constraint semantics, not necessarily drawable output | Evidence closeout or reader search; do not emit geometry |
| `0x003D igSmartFrame2d` | StructuralCandidate | SmartFrame / OLE frame path, candidate reader `sub_564464D0` but not ordinary geometry | Reader / xref investigation; do not emit geometry |
| `0x0059 igCircle2d` | NeedsFixture | IDA type name exists, no current effective fixture hit in Phase 34 plan | Add fixture or native reader evidence |
| `0x0061 igArc2d` | NeedsFixture | High-value geometry, absent from current fixture evidence | Add fixture or native reader evidence; do not reuse retired `0x0030` arc interpretation |
| `0x0063 igEllipse2d` | NeedsFixture | High-value geometry, absent from current fixture evidence | Add fixture or native reader evidence |
| `0x007E igEllipticalArc2d` | NeedsFixture | High-value geometry, absent from current fixture evidence | Add fixture or native reader evidence |
| `0x005D igBSplineCurve2d` | NeedsFixture | High-value geometry, absent from current fixture evidence | Add fixture or native reader evidence |
| `0x0010` | Blocked / TypedAudit | No discriminator or bounded field layout evidence | Keep positional `leading_word`; no geometry emission |
| `0x00FA GraphicGroup` | Blocked / TypedAudit | Child/reference payload semantics unproven | Keep header + raw tail only |

## 34-B Readiness

`0x0020 igRectangle2d` is still the best first implementation candidate, but it
is not ready to code from this inventory alone. The next slice should first
produce a rectangle-specific byte-layout note with:

- all 3 record offsets and half-open byte ranges;
- `bytes_to_follow`, type flags, preceding and following record types;
- candidate corner / transform fields with finite numeric checks;
- malformed/truncated rejection expectations;
- expected byte-audit movement if the decoder is accepted.

If those three records do not prove stable corners or a stable transform, the
correct Phase 34-B result is a negative closeout, not a DTO.

## Verification Notes

The existing verification commands remain:

```powershell
cargo run --quiet --example probe_psm_type_code_histogram
cargo run --quiet --example probe_psm_undecoded_shapes
cargo test --locked --test parse_real_files geometry_fixture_inventory_reports_normalized_geometry_counts -- --nocapture
```

During this session the shell tool stopped returning exit status, so these
commands were not trusted as newly rerun evidence. This document is therefore a
consolidation of already-landed source/test/docs evidence, not a regenerated
snapshot.

## Decision

Phase 34-A is complete as a read-only inventory. The geometry gap is now
quantified as:

- fixture scope mismatch found and normalized to the 6 local PID baseline;
- current local fixture count below the 8-fixture target;
- three cross-fixture undecoded candidates, only one of which is an immediate
  drawable geometry candidate;
- five high-value drawable geometry families blocked by missing fixture/native
  reader evidence;
- remaining probe/audit families that must not be promoted by shape alone.

Next recommended action: normalize the Phase 34 fixture set and produce the
`0x0020 igRectangle2d` byte-layout note before any parser implementation.
