# Phase 34 0x0020 Neighbor Correlation Evidence

> Date: 2026-06-26
> Scope: Phase 34-B evidence probe for `0x0020 igRectangle2d`. No parser,
> schema, writer, bundle confidence, or normalized geometry output changed.

## Result

`0x0020` remains ownership-gated and is not admitted for a decoder slice.

The enhanced `probe_psm_undecoded_shapes` run confirmed four `0x0020` records
with stable `bytes_to_follow = 78`, but none of their containing streams has a
nearby `igLine2d` record accepted by the strict `decode_igline_at` decoder.
Immediate neighbor records may carry the `0x0018` type code, but they do not
produce decoded endpoints through the current validated decoder.

Therefore the candidate f64 values cannot be correlated with decoded
`igLine2d` endpoints, bbox min/max, extents, or length. Partial numeric-looking
values are not enough to claim rectangle fields.

## Probe Command

```powershell
cargo run --quiet --example probe_psm_undecoded_shapes
```

The probe now reports:

- immediate previous and next record summaries;
- nearest strictly decoded previous and next `igLine2d` records, when present;
- candidate finite f64 payload offsets in `[-10, 10]`;
- nearest candidate-to-line endpoint/bbox/extent/length deltas.

## 0x0020 Evidence

| Family | Records | Location | Immediate context | Strict decoded `igLine2d` correlation |
|---|---:|---|---|---|
| `/Sheet6615` mini-sheet | 2 | `DWG-0202GP06-01.pid` and publish duplicate | first record, next type-code `igLine2d flags=2 btf=50` | none |
| nested `/JSite204\Sheet6` | 2 | `A01.pid` | surrounded by type-code `igLine2d btf=50` records | none |

Candidate f64 offsets remain the same shape as the readiness note:

| Family | Candidate offsets |
|---|---|
| `/Sheet6615` | `+18`, `+26`, `+34`, `+50` |
| nested `/JSite204\Sheet6` sample 1 | `+34`, `+46`, `+50` |
| nested `/JSite204\Sheet6` sample 2 | `+18`, `+26`, `+34`, `+50` |

The probe prints `no decoded igLine2d neighbor` for all `0x0020` candidate
values. That is a negative correlation result, not missing instrumentation.

## Decision

Close Phase 34-B as `0x0020 = identified drawable type name,
ownership-gated / no decoder`.

Do not add:

- `SheetIgRectangle2dDecoded`;
- `PidGraphicKind::Rectangle`;
- closed-polyline rectangle emission;
- byte-audit consumed-range movement;
- writer or bundle confidence changes.

Reopen only if a future controlled fixture or native reader proves both:

1. whether `/Sheet6615` and nested `/JSite204\Sheet6` should be projected into
   top-level or symbol-local geometry; and
2. stable bounded fields for rectangle corners, transform, or extents.
