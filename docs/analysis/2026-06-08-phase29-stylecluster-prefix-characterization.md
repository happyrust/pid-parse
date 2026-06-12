# Phase 29: StyleCluster Prefix Characterization

> Generated from `cargo run --example probe_phase29_stylecluster_prefix`
> (run 2026-06-10). Investigation-only report; no parser semantics are
> promoted. Follow-up to
> `2026-06-08-phase29-stylecluster-leftover-triage.md`, which left the
> `/StyleCluster` prefix (12,300 bytes across 6 fixtures) as the only
> remaining leftover in that stream family.

## Question

The `/StyleCluster` region between the 16-byte cluster header and the
end-anchored body record chain looked GUID-table-like. Can it be
characterized tightly enough to justify a bounded parser, or should it
be closed out as documentation until IDA evidence arrives?

## Findings

### 1. Fixed 12-byte opener

All 6 fixtures open the prefix identically:

- `[16..26)` — ten `0x00` bytes;
- `[26..28)` — `u16` value **13**, constant across fixtures whose
  prefix lengths differ (1,529–2,360 bytes), so it is not a byte length.
  Whether it counts the embedded GUID-bearing entries is plausible but
  unproven.

### 2. 532-byte cross-fixture constant region

Byte-for-byte comparison of the prefix across fixtures (baseline `d06`):

| Fixture | Prefix len | Common prefix with baseline |
|---|---:|---:|
| `d06` | 2,360 | 2,360 |
| `nonascii-process-1` | 1,529 | **532** |
| `dwg0201` | 1,883 | **532** |
| `dwg0202` | 2,251 | **532** |
| `publish-a01` | 2,026 | **532** |
| `publish-dwg0202` | 2,251 | **532** |

`[16..548)` is identical boilerplate in every fixture — a writer
template, not per-drawing data.

### 3. The GUID region is not a uniform-stride array

Real COM-era GUIDs are embedded in the constant region
(`{93ADC030-0CB6-11D0-B29B-08003622D702}`,
`{EA1ACBD2-E20A-11CE-95CA-080036483902}`,
`{606FE421/606FE423-0025-11D0-A1E1-080036A1CF02}`,
`{571A3A00-3D33-11CE-BA54-080036019EE7}`, …), but no candidate stride
packs 13 clean entries:

| Stride | Plausible GUID entries (of 13) |
|---:|---:|
| 16 | 5 |
| 20 | 7 |
| 24 | 5 |
| 28 | 2 |
| 32 | 6 |

The region interleaves GUIDs with non-uniform payload (visible
`00 00 00 00 00 00 00 46` OLE-style tails and counter words), so a
"count × fixed-size entry" parser is **not** justified.

### 4. Fixture-specific style-slot region after 548

Beyond the constant region, every fixture carries UTF-16LE style names
at the same early offsets — `Normal` ×5 at 562/604/646/688/730
(42-byte slot stride, matching the 14/28/42 stride autocorrelation from
the body triage), then `ANSI`, `Viewport`, `Office Automation`, and
fixture-specific entries (`As Drawn`, `psOk`, `Nozzle - New`,
`PROJECTSTYLES.SP…` tails). Slot payloads differ per fixture
(ids / counters), so this is per-drawing style-slot data with a shared
template — structure known in texture, not in fields.

## Decision: Documentation-Only Closeout + IDA Target Request

- **No parser this round.** A uniform-stride GUID-table parser fails the
  evidence (Finding 3); template-matching 532 hardcoded bytes would be
  byte-comparison, not parsing; and tracing only the 12-byte opener
  would claim 72 bytes cross-fixture — not worth the surface.
- The prefix remains intentional leftover, now precisely mapped:
  12-byte opener + 520-byte constant GUID-bearing boilerplate +
  fixture-specific 42-byte-slot style region.
- **IDA target request** (extends the body-triage request): in a
  SmartPlant host IDB (`style.dll` first), locate the `stream_type
  0x005A` writer/reader; resolve the meaning of count 13, the GUID
  entry layout, and the 42-byte style-slot record; then promote the
  prefix in one slice together with body record-type naming.

## Guardrails

- Embedded GUID byte patterns are layout evidence only; no registry or
  IDA identity is claimed for any GUID.
- The 42-byte slot observation is texture evidence; no field names.
- Constant-region equality holds for the 6 local fixtures only; private
  customer files may diverge.
