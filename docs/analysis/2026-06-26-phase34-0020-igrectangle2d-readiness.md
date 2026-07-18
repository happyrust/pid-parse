# Phase 34 0x0020 igRectangle2d Readiness Note

> Date: 2026-06-26  
> Scope: pre-decoder readiness for Slice 34-B. No parser, schema, writer,
> bundle confidence, or geometry emission change is made by this note.

## Result

`0x0020 igRectangle2d` remains the preferred first drawable geometry candidate
for Phase 34-B, but it is **not ready for decoder implementation yet**.

The current evidence proves:

- `0x0020` maps to the IDA / IGDS type name `igRectangle2d`.
- The normalized 6-fixture probe now records 4 hits across 3 fixture paths.
- All 4 accepted records have `bytes_to_follow = 78`.
- It is more directly drawable than `0x0013 igBoundary2d` or
  `0x003D igSmartFrame2d`.

The current evidence does **not** yet prove:

- corner, width/height, transform, or vertex fields;
- whether the record should emit a closed polyline or stay probe-only;
- byte-audit movement if a decoder claims the ranges.

Therefore, Phase 34-B must stay in byte-layout / ownership investigation until
field offsets and emission semantics are proven. The ownership review is
recorded in
`docs/analysis/2026-06-26-phase34-0020-ownership-review.md`.

## Evidence Already Available

| Evidence | Status | Meaning |
|---|---|---|
| `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md` type table | Available | `0x20` is named `igRectangle2d` in the IGDS / IDA type table. |
| Phase 34 plan candidate table | Available | Initial cross-fixture probe summary said `0x0020` had 3 hits across 2 fixtures. |
| `examples/probe_psm_undecoded_shapes.rs` | Enhanced and fixture-normalized | Dumps count, `bytes_to_follow`, neighbor types, ranges, u32 words, f64 candidates, and payload hex for `0x0020`. |
| Current session probe output | Trusted | Rerun succeeded after command execution recovered; updated hit count is 4 across the normalized fixture set. |

## Important Caveat

Older Phase 14 documentation says standard IGDS `0x0020` rectangle had zero hits
in the fixture set at that time. The normalized Phase 34 probe now reports 4
hits across 3 fixture paths. Treat the Phase 14 statement as an older snapshot,
not a current negative proof.

## Trusted Probe Output

Command:

```powershell
cargo run --quiet --example probe_psm_undecoded_shapes
```

Summary:

| Sample | Fixture | Stream | Range | Flags | BTF | Context | Nonzero f64 candidates |
|---:|---|---|---|---:|---:|---|---|
| 0 | `DWG-0202GP06-01.pid` | `/Sheet6615` | `0x000008..0x00005C` (`8..92`) | 2 | 78 | `<none>` -> `igLine2d` | `+18=0.126271288517`, `+26=0.092378904764`, `+34=0.127478376761`, `+50=0.406862745098` |
| 1 | `A01.pid` | `/JSite204\Sheet6` | `0x000594..0x0005E8` (`1428..1512`) | 0 | 78 | `igLine2d` -> `igLine2d` | `+34=0.594000000000`, `+46=-0.001617739908`, `+50=0.707070707071` |
| 2 | `A01.pid` | `/JSite204\Sheet6` | `0x000A04..0x000A58` (`2564..2648`) | 0 | 78 | `igLine2d` -> `igLine2d` | `+18=0.025000000000`, `+26=0.010000000000`, `+34=0.559000000000`, `+50=0.715563506261` |
| 3 | `publish-DWG-0202GP06-01.pid` | `/Sheet6615` | `0x000008..0x00005C` (`8..92`) | 2 | 78 | `<none>` -> `igLine2d` | same as sample 0 |

Raw payload prefixes are captured in the probe output. Two samples are exact
publish/source duplicates for `/Sheet6615`, so the effective independent shape
families are the `/Sheet6615` top-level mini-sheet case and the nested
`/JSite204\Sheet6` case.

## Decoder Admission Criteria

A `SheetIgRectangle2dDecoded` DTO is only allowed if all criteria hold:

1. All 3 current records share a stable layout or a clearly versioned layout.
2. The candidate rectangle geometry is backed by bounded byte ranges.
3. Numeric fields are finite and plausible in the same coordinate domain as
   existing decoded Sheet geometry.
4. The decoder rejects malformed/truncated variants without partial claims.
5. Byte-audit can explain the exact consumed-range movement.
6. Existing guardrails remain intact: no `0x0010` geometry, no `0x00FA`
   child-list naming, no `0x0030` arc revival.

If any criterion fails, the correct Phase 34-B output is a negative closeout.

Current status against these criteria:

| Criterion | Current status |
|---|---|
| Stable record size | Pass for current samples: all `btf=78`. |
| Stable field layout | Not proven. Candidate f64 offsets differ by family; one nested sample lacks the `+18` / `+26` candidates seen in the other records. |
| Drawable rectangle semantics | Not proven. Current hits are `/Sheet6615` mini-sheet records or nested `/JSite204\Sheet6` records, not primary top-level `/Sheet6` records. |
| Byte-audit movement | Not planned yet. Need identify whether these ranges are leftover/probe today and whether claiming them changes top-level vs nested ownership. |
| Emission representation | Deferred. Closed polyline remains preferred if geometry is proven. |

## Emission Shape

If the byte layout is proven, the default output should be a decoded closed
polyline rather than a new public rectangle enum. A new public kind should only
be introduced if downstream consumers require rectangle-specific semantics that
cannot be represented by a closed polyline plus provenance.

## Current Decision

Do not implement `decode_igrectangle_at` yet. The current ownership review shows
that `0x0020` is ownership-gated:

- decide whether `/Sheet6615` should be decoded as top-level drawable geometry
  or treated as a small Sheet-like support stream;
- decide whether nested `/JSite204\Sheet6` belongs to symbol-local geometry and
  whether Phase 34 should decode it at all;
- test candidate f64 offsets (`+18`, `+26`, `+34`, `+50`, and nested-only
  `+46`) against nearby decoded `igLine2d` records before any DTO is added.

Follow-up result:
`docs/analysis/2026-06-26-phase34-0020-neighbor-correlation.md` completed that
neighbor probe and found no strict decoded `igLine2d` neighbor in the `0x0020`
streams. Phase 34-B therefore closes as `0x0020 = identified drawable type
name, ownership-gated / no decoder`.
