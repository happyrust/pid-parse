# Phase 29-B Sheet Byte-Audit Trace Integration

> Date: 2026-06-08  
> Scope: claim existing Sheet typed / audit-only decoder byte ranges in
> byte-audit traces. This is a coverage accounting change, not a new parser
> semantics change.

## Change

`src/byte_audit/aggregate.rs` now augments top-level Sheet byte-audit traces
with existing Sheet decoder byte ranges:

Decoded confidence:

- `decode_primitive_lines`
- `decode_iglines`
- `decode_iglinestrings`
- `decode_igpoints`
- `decode_igtextboxes`
- `decode_igsymbols`
- `decode_jstyle_overrides`

Probed confidence for audit-only families:

- `decode_graphic_groups`
- `decode_sub_records_0x0010`

No decoder field semantics were changed.

## Aggregation Fix

Adding decoded ranges exposed a pre-existing aggregation issue:
`ParserTrace::consumed_bytes()` summed `consumed_ranges` directly, so overlapping
ranges with different confidence buckets could be double-counted. This produced
impossible Sheet coverage ratios above `1.0`.

`consumed_bytes()` now returns:

```text
total_bytes - leftover_bytes
```

That counts the union of consumed ranges because `leftover_ranges` is already
computed from the flattened union across confidence buckets.

## Focused Tests

Added:

- `byte_audit::parser_trace_tests::consumed_bytes_counts_union_across_confidence_overlap`
- `byte_audit::aggregate::tests::sheet_typed_decoders_claim_known_record_ranges`

Validation:

```text
cargo test --lib byte_audit -- --nocapture
```

Result: `41 passed`.

## Snapshot Effect

After regenerating all 6 local fixture snapshots:

| Fixture id | Overall ratio | `/Sheet6` consumed | `/Sheet6` leftover | `/Sheet6` ratio |
|---|---:|---:|---:|---:|
| `d06` | 0.13231003 | 3,532 | 590 | 0.8568656 |
| `nonascii-process-1` | 0.21170664 | 38,137 | 8,403 | 0.8194456 |
| `dwg0201` | 0.22253118 | 27,403 | 2,191 | 0.9259647 |
| `dwg0202` | 0.18653207 | 21,311 | 2,420 | 0.89802366 |
| `publish-a01` | 0.08193194 | 1,523 | 257 | 0.855618 |
| `publish-dwg0202` | 0.18639842 | 21,311 | 2,428 | 0.89772105 |

This confirms that much of the earlier Sheet leftover was already covered by
existing typed / audit-only decoders or the common Sheet header, but had not
been represented in byte-audit accounting.

## Remaining Sheet Leftover After Trace Integration

After rerunning:

```text
cargo run --example probe_phase29_sheet_leftover_windows \
  > docs/analysis/2026-06-08-phase29-sheet-leftover-windows-after-trace.md
```

the top remaining groups are no longer the broad `0x0001` / `0x0002` local
shapes. The remaining groups include:

- nested `/JSite204/Sheet6` with cluster-family magic `44 F5 90 6C`;
- `0x00CE` variants that the current conservative `igSymbol2d` decoder does
  not accept;
- top-level Sheet header / cluster-family prefix leftovers;
- small residual groups that need bounded record-shape review.

## Guardrails

- This phase does not add new record decoders.
- `0x0010` remains audit-only.
- `GraphicGroup` raw tail remains unnamed.
- `PidPageTransform::Available` remains unavailable.
- Text placement remains no-promotion.

## Next Step

Use `phase29-sheet-leftover-windows-after-trace.md` as the new input for any
future Sheet unknown-record work. Do not use the pre-integration window report
as the priority source except for historical comparison.
