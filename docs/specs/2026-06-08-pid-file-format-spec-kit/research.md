# Research: PID File Format Evidence

## Summary

The parser has a mature container and metadata foundation, a broad but still
evidence-graded Sheet/PSM decoding surface, and an active IDA-backed type
matrix effort. The current highest-value work is not another blind byte-pattern
decoder; it is completing the IDA and fixture cross-checks needed to safely
promote partial or audit-only families.

## Parser Facts

### Stable / Decoded Areas

- `.pid` outer structure is OLE / CFBF.
- The parser preserves stream tree metadata and raw stream bytes through
  `PidPackage`.
- Summary / DocumentSummary property sets are decoded.
- `/TaggedTxtData/Drawing` and `/TaggedTxtData/General` XML metadata are
  decoded.
- `DocVersion2`, `DocVersion3`, `AppObject`, and `JTaggedTxtStgList` are
  decoded.
- `PSMroots` is decoded.
- Writer is passthrough-first and supports metadata updates, stream
  replacement, round-trip verification, and experimental Sheet byte patches.

### Partial Areas

- `PSMclustertable` and `PSMsegmenttable` expose useful structure but still
  contain unnamed SmartPlant semantics.
- Dynamic Attributes provide object inventory and relationship evidence, but
  record body fields still have leftover / partial semantics.
- `ObjectGraph`, `CrossReferenceGraph`, `layout`, and `PidImportView` are useful
  consumer views, but not yet one fully unified canonical semantic graph.
- `NormalizedPidGeometry` can emit source-backed / inferred geometry, but the
  source-to-page transform remains unavailable.

### Sheet / PSM Decoder Facts

Current typed or audit-only Sheet record families:

- `0x3FE6 GLine2d`: decoded SmartPlant extension line wrapper.
- `0x0018 igLine2d`: decoded.
- `0x0084 igLineString2d`: decoded.
- `0x005E igPoint2d`: decoded.
- `0x004D igTextBox`: decoded, with IDA support for type identity and text
  semantics.
- `0x00CE igSymbol2d`: decoded, with IDA type-name confirmation.
- `0x0030 JStyleOverride`: decoded by Phase 16 IDA evidence; previous
  `PrimitiveArc` interpretation was retired.
- `0x00FA GraphicGroup`: audit-only; header and raw variable tail are collected,
  but child/reference semantics are not named.
- `0x0010`: audit-only polymorphic family; `leading_word` is only
  `payload[0..2]` as little-endian `u16`, not a semantic sub-kind.

## IDA Facts

### `radsrvitem.dll` Type-Code Mapper

`sub_56448F70(_WORD *a1)` maps `u16` PSM / IGDS type codes to type names.

Confirmed names include:

- `0x0018 igLine2d`
- `0x0020 igRectangle2d`
- `0x004D igTextBox`
- `0x0059 igCircle2d`
- `0x005E igPoint2d`
- `0x0061 igArc2d`
- `0x0063 igEllipse2d`
- `0x007E igEllipticalArc2d`
- `0x0084 igLineString2d`
- `0x00CE igSymbol2d`
- `0x0115 igDimension`
- `0x0117 igBalloon`
- `0x0118 igLeader`

This proves type identity, not field layout.

### `igTextBox` Reader Candidate

`sub_56445F40` dispatches `0x004D` to `sub_564468B0`.

`sub_564468B0` confirms:

- `*a2 == 77`, so `0x004D = igTextBox`.
- text payload is UTF-16LE.
- extracted text is written to property `"TEXT"`.
- runtime text layout has modes controlled by `a2[12] == 1 / 2 / 3`.
- relation IDs are written to `"RELEATIONS"` with the binary's spelling.

Important boundary: this function reads runtime record layout, not directly the
raw `Sheet*` stream bytes consumed by `decode_igtextbox_at`. Rust raw offsets
such as `payload+30` and `payload+32` remain fixture/probe-backed until the
runtime-to-raw mapping is proven.

### Default IGDS Path Negative Evidence

`sub_564462F0`, the default path for many IGDS types, appears to:

1. map type code to type name,
2. create or update RAD object properties,
3. write `"RELEATIONS"`,
4. register the object.

It does not visibly read ordinary geometry fields such as line endpoints, point
coordinates, polyline point lists, symbol transforms, or circle / arc / ellipse
parameters.

Therefore `radsrvitem.dll` currently proves type names and part of runtime
property behavior, but not ordinary geometry field layout.

### Runtime Record Pointer Chain

Phase 27 traced the `sub_56445F40` record pointer back through the
`ImpIJPersistManager` / `SerialCluster` chain:

```text
record_id
  -> ImpIJPersistManager::vtable+0xA4
  -> descriptor lookup
  -> SerialCluster::vtable+0x70
  -> runtime_record_ptr = serial_cluster_base + record_descriptor[0]
```

The stream binding seen in `radsrvitem.dll` opens CFB root-level PSM streams:

- `PSMclustertable`
- `PSMroots`
- `PSMspacemap`
- `PSMcluster0`

This is not yet a direct proof of raw `Sheet*` stream offsets.

### Latest `0x0010` / JStyle Blocker

Saved progress records that `radsrvitem.dll` contains JStyleBase / IJPersist
control-path samples, but the visible paths are wrappers or control logic, not
the `0x0010` attribute fragment implementation.

Current blocker:

- only `radsrvitem.dll` and unrelated `core.dll` were reachable,
- no `style.dll` / JStyle host module was open,
- local assets did not expose the needed style-related binary,
- `1D1928C0-0000-0000-C000-000000000046` field semantics remain unresolved.

Next evidence source must be a module containing the real JStyle/RAD host
implementation.

## Fixture Facts

- Historical fixture registry reached 5 PID fixtures, below the 8-12 target.
- Later Phase 26 found no `.pid` fixtures available in the current working tree
  at that time, so fresh coverage / byte-audit snapshots were blocked.
- D06 is a compact fixture used for relationship fallback, Sheet audit, and
  text-placement regression.
- Fixture availability is a hard constraint for any new coverage or byte-audit
  claims.

## Guardrail Findings

- `0x0030` must stay `JStyleOverride`.
- `0x0010.leading_word` must remain byte-position evidence.
- `0x00FA GraphicGroup` must remain audit-only.
- f64 geometry evidence can produce useful inferred points/lines, but does not
  prove page transform.
- `page_dimensions_mm` is page-size evidence, not a decoded transform.
- Text probes remain no-promotion until text extraction and placement are
  source-proven.

## Open Research Questions

1. Which module contains the ordinary geometry reader for `igLine2d`,
   `igPoint2d`, `igLineString2d`, `igSymbol2d`, and P1 geometry types?
2. How does the PSM runtime `SerialCluster` record map to raw `Sheet*` stream
   records, if at all?
3. What is the real discriminator for `0x0010` sub-record variants?
4. Does `GraphicGroup` raw tail contain child OIDs, reference IDs, or another
   RAD persistence payload?
5. Which record, if any, encodes the source-to-page transform needed for
   `PidPageTransform::Available`?
