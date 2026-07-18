# Phase 34 0x0020 Ownership Review

> Date: 2026-06-26  
> Scope: ownership review for `0x0020 igRectangle2d` candidates after the
> normalized 6-fixture probe. No parser, schema, writer, bundle confidence, or
> geometry emission change is made by this note.

## Result

`0x0020` is not currently an immediate top-level Sheet geometry decoder slice.
The normalized probe found 4 records, but none are in the primary top-level
`/Sheet6` streams:

- 2 records are the same `/Sheet6615` mini-sheet shape in `dwg0202` and its
  publish fixture duplicate.
- 2 records are inside nested `/JSite204\Sheet6` under the A01 publish fixture.

This means the next question is ownership, not field decoding. A decoder that
blindly emits these records into normalized document geometry would mix
top-level drawing geometry, mini Sheet-like support streams, and nested
symbol-local geometry.

## Probe Evidence

Command:

```powershell
cargo run --quiet --example probe_psm_undecoded_shapes
```

| Family | Samples | Location | Record position | Neighbor context | Initial ownership judgment |
|---|---:|---|---|---|---|
| `/Sheet6615` mini-sheet | 2 | `DWG-0202GP06-01.pid` and `publish-DWG-0202GP06-01.pid` | first PSM record after stream prefix, `0x000008..0x00005C`, `flags=2`, `btf=78` | no previous record, next `igLine2d flags=2 btf=50` | top-level Sheet-like support stream; not primary `/Sheet6` |
| nested `/JSite204\Sheet6` | 2 | `A01.pid` under `/JSite204` | `0x000594..0x0005E8` and `0x000A04..0x000A58`, `flags=0`, `btf=78` | surrounded by `igLine2d btf=50` records | nested JSite / symbol-local geometry package |

The `btf=78` invariant is stable across all 4 records, but flags and ownership
context differ:

- `/Sheet6615`: `flags=2`, first record, mini Sheet stream.
- nested `/JSite204\Sheet6`: `flags=0`, embedded among decoded `igLine2d`
  records inside a JSite package.

## Field-Layout Readiness

The probe reports nonzero finite f64 candidates:

| Family | Candidate f64 offsets |
|---|---|
| `/Sheet6615` | `+18=0.126271288517`, `+26=0.092378904764`, `+34=0.127478376761`, `+50=0.406862745098` |
| nested `/JSite204\Sheet6` sample 1 | `+34=0.594000000000`, `+46=-0.001617739908`, `+50=0.707070707071` |
| nested `/JSite204\Sheet6` sample 2 | `+18=0.025000000000`, `+26=0.010000000000`, `+34=0.559000000000`, `+50=0.715563506261` |

These candidates are plausible normalized coordinate-like values, but they do
not yet prove rectangle semantics:

- offsets are not stable across all samples (`+18` / `+26` absent from one
  nested sample);
- `+50` appears in all samples but could be scale, extent, style, or unrelated
  payload data;
- no corner-pair or width/height invariant is established;
- no byte-audit movement is planned because ownership is unresolved.

## Decoder Decision

Do not implement `SheetIgRectangle2dDecoded` yet.

Before a decoder is allowed, a follow-up must prove one of these scoped
contracts:

1. `/Sheet6615` is a top-level drawable Sheet stream and its `0x0020` record
   has a stable rectangle layout.
2. nested `/JSite204\Sheet6` geometry should be decoded as symbol-local geometry
   and exposed under a nested-geometry contract, not merged into top-level
   document geometry.
3. both families share a true layout but require separate ownership projection.

If none of these contracts can be proven, Phase 34-B should close as
`0x0020 = identified drawable type name, ownership-gated / no decoder`.

Follow-up neighbor-correlation evidence is recorded in
`docs/analysis/2026-06-26-phase34-0020-neighbor-correlation.md`. That probe
found no strict decoded `igLine2d` neighbor in the `0x0020` streams, so the
candidate f64 offsets still cannot be tied to decoded endpoints, bbox fields,
extents, or length.

## Next Evidence Step

The completed neighbor correlation probe did not admit a DTO:

- `/Sheet6615` and nested `/JSite204\Sheet6` remain separate ownership
  families;
- immediate `0x0018` neighbors are not enough because the strict `igLine2d`
  decoder does not produce endpoints for these streams;
- keep `0x0020` probe-only until a controlled fixture or native reader proves
  both ownership projection and bounded rectangle fields.
