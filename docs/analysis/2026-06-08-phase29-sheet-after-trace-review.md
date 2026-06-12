# Phase 29-C Sheet After-Trace Remaining Groups Review

> Date: 2026-06-08  
> Input: `docs/analysis/2026-06-08-phase29-sheet-leftover-windows-after-trace.md`  
> Scope: classify remaining Sheet byte-audit leftovers after Phase 29-B trace
> integration. This review does not change parser semantics.

## Starting Point

Phase 29-B changed the meaning of Sheet leftovers:

- common Sheet cluster-family header is claimed;
- existing typed Sheet decoders are claimed as `Decoded`;
- audit-only `GraphicGroup` and `0x0010` are claimed as `Probed`;
- `ParserTrace::consumed_bytes()` now uses union accounting.

After that, `/Sheet6` coverage is high (`0.81–0.93` across local fixtures), so
remaining Sheet leftovers are much narrower and more actionable than the
pre-integration report.

## Top Remaining Groups

| Rank | Shape | Current classification | Reason |
|---:|---|---|---|
| 1 | nested `/JSite204/Sheet6`, prefix `44 F5 90 6C` | `NeedsRegistration + OwnershipDecision` | Starts with cluster-family magic and lives under `JSite204`, so it should not be treated as a top-level Sheet record family until ownership semantics are clear. |
| 2 | `0x00CE igSymbol2d`, prefix `CE 00 71 00` | `NeedsSymbolRejectProbe` | Type code is known, but current conservative `decode_igsymbols` did not claim these ranges. Need rejection reason before changing validation. |
| 3 | `0x00CE igSymbol2d`, prefix `CE 00 79 00` | `NeedsSymbolRejectProbe` | Same as above; appears across more fixtures. |
| 4 | `0x00CE igSymbol2d`, prefix `CE 80 79 00` | `NeedsIDAOrVariantEvidence` | Non-zero type flags (`0x8000` high bits) may be meaningful or may indicate a variant / wrapper; do not simply relax validation. |
| 5 | `0x00CE igSymbol2d`, prefix `CE 80 71 00` | `NeedsIDAOrVariantEvidence` | Same high-bit concern as rank 4. |
| 6 | local `0x0B00 unknown`, prefix `00 00 00 08` | `NeedsWindowAlignmentReview` | Likely starts inside a payload or tail; not enough evidence for a record header. |
| 7 | top-level cluster-family prefix `44 F5 90 6C` | `NeedsHeaderResidualReview` | Some top-level header-adjacent bytes still appear as local shapes; likely not a new record decoder target. |
| 9 | `0x0084 igLineString2d`, prefix `84 00 38 00` | `NeedsLineStringRejectProbe` | Type code is known; current decoder did not claim these ranges. Need rejection reason before changing validation. |

## Interpretation

The remaining high-value work is **not** a broad new Sheet decoder. It is a set
of narrow rejection probes:

1. Why are some `0x00CE` records not accepted by `decode_igsymbols`?
2. Why are some `0x0084` records not accepted by `decode_iglinestrings`?
3. Which nested `JSite*/Sheet*` streams should be registered, and what owner
   model should they map to?

## Recommended Next Slice: Phase 29-C1 Symbol Reject Probe

### Goal

Explain the `0x00CE` after-trace leftovers by rejection reason:

- type flags rejected,
- `bytes_to_follow` out of accepted range,
- payload truncated,
- f64 transform / insertion values non-finite or out of domain,
- decoder accepted but byte-audit range still split by overlapping trace,
- other.

### Output

`docs/analysis/2026-06-08-phase29-igsymbol-reject-probe.md`

### Guardrails

- Do not relax `decode_igsymbols` validation in the same slice.
- Do not introduce a new DTO.
- Do not claim all `0x00CE` bytes are symbols; some may be variants or wrapper
  records.
- If non-zero type flags dominate, require IDA or controlled fixture evidence
  before accepting them.

## Secondary Slice: Phase 29-C2 Nested Sheet Registration Review

### Goal

Classify `/JSite204/Sheet*` streams:

- Are they true Sheet streams?
- Are they embedded symbol-local cluster streams?
- Should byte-audit register them with cluster-header tracing only?
- Should they stay unregistered until JSite ownership is modeled?

### Guardrail

Nested `JSite*/Sheet*` streams must not be merged into top-level `/Sheet*`
geometry coverage until ownership and downstream semantics are clear.

## Decision

Proceed with **Phase 29-C1 Symbol Reject Probe** first. It is narrower than
general Sheet reverse engineering and directly addresses the largest remaining
top-level Sheet leftover groups after trace integration.
