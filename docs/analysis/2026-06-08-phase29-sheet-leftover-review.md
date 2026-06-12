# Phase 29-A Sheet Leftover Manual Review

> Date: 2026-06-08  
> Inputs:
> - `docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md`
> - `docs/analysis/2026-06-08-phase29-sheet-leftover-windows.md`
> - `src/byte_audit/aggregate.rs`
> - `src/parsers/sheet_records.rs`

## Key Finding

The current `/Sheet*` byte-audit trace is **not equivalent to Sheet decoder
coverage**.

`src/byte_audit/aggregate.rs` registers top-level Sheet streams with
`probe_sheet_stream`, then traces:

- text run evidence;
- 26-byte endpoint records.

It does **not** currently call the Phase 14+ typed / audit-only Sheet decoders
when building byte-audit traces:

- `decode_primitive_lines`
- `decode_iglines`
- `decode_iglinestrings`
- `decode_igpoints`
- `decode_igtextboxes`
- `decode_igsymbols`
- `decode_jstyle_overrides`
- `decode_graphic_groups`
- `decode_sub_records_0x0010`

Therefore, a Sheet byte-audit leftover range can mean at least three different
things:

1. truly unknown Sheet bytes;
2. known typed / audit-only record bytes that are not yet claimed by
   byte-audit;
3. nested cluster-family streams whose ownership is not top-level Sheet
   semantics.

## Review Of Top Window Groups

| Group | Local byte shape | Current interpretation | Classification |
|---|---|---|---|
| #1 | `0x0001 unknown / btf 1024+ / prefix 01 00 FB FF` | Repeated Sheet-local shape across 4 fixtures. The candidate type code is only a local PSM-like header found inside a leftover range, not a confirmed record type. | `NeedsShapeReview + NeedsIDA` |
| #2 | `0x0002 unknown / btf 1024+ / prefix 01 00 01 00` | Repeated across 3 fixtures. Appears in regions that include list/control-like small integers and `0x0030`-looking bytes. | `NeedsShapeReview + NeedsIDA` |
| #3 | `0x0001 unknown / btf 0512-1023 / prefix 01 00 01 00` | Repeated across DWG and D06 fixtures. Same caution as #1. | `NeedsShapeReview + NeedsIDA` |
| #4 | `0x0005 unknown / btf 1024+ / prefix 01 00 01 00` | Repeated across 3 fixtures. The byte shape may be section/list structure rather than a top-level PSM record. | `NeedsShapeReview + NeedsIDA` |
| #5 | nested `/JSite204/Sheet6`, prefix `44 F5 90 6C` | Starts with the cluster-family magic, so it is not equivalent to top-level `/Sheet6` raw geometry. | `NeedsRegistration + OwnershipDecision` |
| #18 | `0x00CE igSymbol2d` | `igSymbol2d` has a typed decoder, but Sheet byte-audit currently does not call typed decoders. | `NeedsByteAuditIntegration`, not a new decoder |

## Consequence For Next Work

The next safe improvement is **not** to immediately write new Sheet record
decoders for groups #1-#4.

The safer next slice is to integrate existing Sheet typed / audit-only decoders
into byte-audit tracing, so that byte-audit can distinguish:

- already decoded records;
- audit-only records;
- probe-only text / endpoint evidence;
- truly unknown leftover windows.

## Proposed Next Slice: Phase 29-B Sheet Byte-Audit Trace Integration

### Goal

Teach `byte_audit_report` to claim known Sheet typed / audit-only record byte
ranges without changing parser semantics.

### Scope

Add trace consumption for existing decoder families only:

- `0x3FE6 GLine2d`
- `0x0018 igLine2d`
- `0x0084 igLineString2d`
- `0x005E igPoint2d`
- `0x004D igTextBox`
- `0x00CE igSymbol2d`
- `0x0030 JStyleOverride`
- `0x00FA GraphicGroup` as `AuditOnly` / probed confidence
- `0x0010` as `AuditOnly` / probed confidence

### Non-Goals

- Do not add new semantic fields.
- Do not rename `0x0010.leading_word`.
- Do not interpret `GraphicGroup` raw tail.
- Do not make page transform available.
- Do not promote text placement.

### Acceptance Criteria

- Sheet byte-audit leftover decreases for fixtures with known decoded records.
- Existing decoded / audit-only record counts remain unchanged.
- Byte-audit confidence clearly distinguishes decoded record bytes from
  audit-only / probed bytes.
- A focused test demonstrates that a synthetic Sheet stream with one known typed
  record is claimed by byte-audit.

## Re-open For New Decoders

After byte-audit trace integration, rerun:

```bash
cargo run --example probe_phase29_sheet_leftover_windows > docs/analysis/2026-06-08-phase29-sheet-leftover-windows-after-trace.md
```

Only groups still left over after existing decoders are claimed should be used
as candidates for new parser / IDA work.
