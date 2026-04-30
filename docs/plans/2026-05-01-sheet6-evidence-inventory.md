# Sheet6 Evidence Inventory for Real Geometry Decode

> **Date:** 2026-05-01  
> **Fixture:** `test-file/DWG-0201GP06-01.pid`  
> **Sheet:** `/Sheet6`  
> **Purpose:** 为下一阶段 `Line + Inferred` 解码建立 evidence inventory，避免把 probe evidence 过早渲染成真实图形。

## Commands Run

```powershell
cd D:\work\plant-code\cad\pid-parse
cargo run --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --geometry-json
cargo run --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --probe-sheet-chunks Sheet6 --json
```

## Normalized Geometry Summary

`--geometry-json` 当前全部 evidence 都来自 `/Sheet6`：

| Kind | Confidence | Count | Current render policy |
|---|---:|---:|---|
| `Point` | `Inferred` | 64 | H7CAD renders on `PID_GEOMETRY_POINTS` |
| `Unknown` text probe | `ProbeOnly` | 9 | Do not render |
| `Unknown` endpoint probe | `ProbeOnly` | 59 | Do not render |
| **Total** |  | **132** | 64 rendered, 68 skipped |

## Sheet Probe Summary

`--probe-sheet-chunks Sheet6 --json` reports:

| Field | Value |
|---|---:|
| `sheet_name` | `Sheet6` |
| `path` | `/Sheet6` |
| `size` | `29594` bytes |
| `candidate_boundaries` | `1869` |
| `chunks` | `308` |
| `text_runs` | `9` |
| `coordinate_hints` | `64` |

Observed `record_type_counts`:

| Record type | Count |
|---|---:|
| `0x1985` | 1 |
| `0x315B` | 1 |
| `0x4046` | 6 |
| `0x683C` | 1 |
| `0x6A21` | 1 |
| `0x9469` | 2 |
| `0xC03F` | 3 |
| `0xDA45` | 1 |
| `0xFD5B` | 1 |

## Coordinate Hints

The 64 coordinate hints currently become `PidGraphicKind::Point + Inferred`.

Important parser detail: current `coordinate_hints` are not a decoded coordinate table. They are the first 64 aligned `i32, i32` pairs that pass a plausibility filter:

```text
offset % 4 == 0
(x != 0 || y != 0)
abs(x) <= 1_000_000
abs(y) <= 1_000_000
stop after 64 hints
```

This makes them useful proof that Sheet source coordinates are reaching H7CAD, but not yet strong enough to map endpoint ids to line endpoints.

Offset range:

- first point byte range: `28..36`
- last point byte range: `1060..1068`

Coordinate bounds from current heuristic:

| Axis | Min | Max |
|---|---:|---:|
| X | `0` | `983056` |
| Y | `-327679` | `983056` |

Early samples:

| Entity | Byte range | X | Y |
|---|---:|---:|---:|
| `/Sheet6:coordinate-hint:0` | `28..36` | `16` | `19484` |
| `/Sheet6:coordinate-hint:1` | `136..144` | `18944` | `13824` |
| `/Sheet6:coordinate-hint:2` | `140..148` | `13824` | `16896` |
| `/Sheet6:coordinate-hint:3` | `144..152` | `16896` | `3072` |
| `/Sheet6:coordinate-hint:4` | `216..224` | `26624` | `16896` |
| `/Sheet6:coordinate-hint:5` | `220..228` | `16896` | `1536` |
| `/Sheet6:coordinate-hint:6` | `224..232` | `1536` | `0` |
| `/Sheet6:coordinate-hint:7` | `256..264` | `0` | `13824` |

## Text Probe Evidence

The 9 text probes are still `Unknown + ProbeOnly` because insertion point, height and rotation are not decoded.

Early samples:

| Entity | Byte range | Decoded text |
|---|---:|---|
| `/Sheet6:text-probe:0` | `81..89` | `봽렎卆툦` |
| `/Sheet6:text-probe:1` | `184..196` | `휱爿낳큷툪?` |
| `/Sheet6:text-probe:2` | `290..300` | `휱렿띪亭퇿` |
| `/Sheet6:text-probe:3` | `306..318` | `휱렿띪亭퇿?` |
| `/Sheet6:text-probe:4` | `409..417` | `諵硒킊?` |

The current text output looks like noisy UTF-16 probe evidence, not user-facing P&ID labels. Do not promote it to `Text` until record semantics provide a real insertion point and a reliable string payload.

## Endpoint Probe Evidence

The 59 endpoint probes are still `Unknown + ProbeOnly`.

Offset range:

- first endpoint record byte range: `17314..17388`
- last endpoint record byte range: `24370..24444`

Early samples:

| Entity | Byte range | `rel_field_x` | Endpoint ids |
|---|---:|---:|---|
| `/Sheet6:endpoint-probe:0` | `17314..17388` | `949` | `229 -> 326` |
| `/Sheet6:endpoint-probe:1` | `17426..17500` | `951` | `740 -> 139` |
| `/Sheet6:endpoint-probe:2` | `17538..17612` | `953` | `139 -> 326` |
| `/Sheet6:endpoint-probe:3` | `17650..17724` | `955` | `433 -> 646` |
| `/Sheet6:endpoint-probe:4` | `17762..17836` | `957` | `452 -> 440` |
| `/Sheet6:endpoint-probe:5` | `17874..17948` | `959` | `661 -> 630` |
| `/Sheet6:endpoint-probe:6` | `17986..18060` | `961` | `490 -> 740` |
| `/Sheet6:endpoint-probe:7` | `18098..18172` | `963` | `602 -> 169` |

## Key Finding

Coordinate hints and endpoint records are not adjacent:

- coordinate hints: `28..1068`
- endpoint records: `17314..24444`

Therefore, the next line-decoding step must not pair endpoint records with the nearest coordinate hint by byte proximity. The likely missing layer is an endpoint-id or `field_x` mapping table that connects:

```text
endpoint_a / endpoint_b
  -> endpoint definition or node record
  -> coordinate record / coordinate hint
  -> Line or Polyline geometry
```

## Object Graph Cross-Check

`pid_inspect --json` confirms the endpoint ids are object `field_x` values, not coordinate indexes.

Current object graph / crossref summary:

| Field | Count |
|---|---:|
| `object_graph.objects` | 68 |
| `object_graph.relationships` | 64 |
| objects with `field_x` | 68 |
| `/Sheet6` relationship endpoint links | 59 |
| unique endpoint ids in `/Sheet6` links | 57 |
| endpoint ids resolving to object records | 51 |
| endpoint ids not found in object records | 6 |
| `/Sheet6` links with source and target DrawingID | 55 |

Endpoint ids missing from object records:

```text
0, 68, 326, 467, 659, 671
```

Endpoint object type distribution:

| Object type | Count |
|---|---:|
| `PipeRun` | 35 |
| missing object record | 6 |
| `Nozzle` | 6 |
| `PipingComp` | 4 |
| `Instrument` | 2 |
| `ItemNote` | 2 |
| `EquipmentOther` | 1 |
| `SignalRun` | 1 |

Early linked examples:

| `rel_field_x` | Endpoint ids | Source DrawingID | Target DrawingID | Sheet offset |
|---:|---:|---|---|---:|
| `949` | `229 -> 326` | `5B8554AB800840539DA97EDD629888D3` | `DAC7AD038E65479C8CC11380690E27EE` | `17314` |
| `951` | `740 -> 139` | `8C9C23D8D02143B8A02708B9485CDF99` | `D8FAB6ED48684E799CDFF0396E213773` | `17426` |
| `953` | `139 -> 326` | `D8FAB6ED48684E799CDFF0396E213773` | `DAC7AD038E65479C8CC11380690E27EE` | `17538` |
| `955` | `433 -> 646` | `7A76004E229A468E893EDCEF6AC5B778` | `3EED312E093A45A19FA036319BB3FA61` | `17650` |
| `957` | `452 -> 440` | `2976224908C243BE828E73680D1BEAA3` | `1622B669BC9C4DA68652380D06BC8497` | `17762` |
| `959` | `661 -> 630` | `E78F86E76A4A4028A5A4A1CB637BF800` | `B6DFAFEF77B84B018484619CABA228F1` | `17874` |
| `961` | `490 -> 740` | `A74866784CEA40CD888EED8BA901A747` | `8C9C23D8D02143B8A02708B9485CDF99` | `17986` |
| `963` | `602 -> 169` | `D744ECCC3AD5415097A66FDC1AF6CC70` | `83D514A215F14515835AF4D5799EF050` | `18098` |

This cross-check is useful for semantic linking, but it does not provide true CAD coordinates. Object graph / layout coordinates must not be used to promote endpoint records into `Line + Inferred`, because those coordinates are topology-derived fallback positions rather than Sheet source geometry.

## Promotion Rules for Next Implementation

Safe to keep:

- `coordinate_hints` as `Point + Inferred`, because they have direct byte range and numeric coordinates.
- endpoint records as `Unknown + ProbeOnly`, because they currently carry ids but no proven endpoint coordinates.
- text probes as `Unknown + ProbeOnly`, because current decoded text appears noisy and lacks placement.

Do not promote to `Line + Inferred` until all of these are true:

1. Both endpoint ids resolve to coordinate-bearing records.
2. The coordinate-bearing records have byte ranges.
3. The relationship between `rel_field_x` and the endpoint record is preserved in provenance.
4. A synthetic test and a real fixture baseline both prove the mapping is deterministic.

## Immediate Next Step

`SheetObjectGeometryHint` has now been added to the public `SheetGeometry` DTO as an empty-by-default contract slot. Population is intentionally still pending.

Add a focused parser-side investigation for endpoint-id mapping. The next probe should be separate from the current broad `coordinate_hints` heuristic and should look for a stronger record shape:

1. Search `Sheet6` bytes for endpoint/object `field_x` values such as `229`, `326`, `740`, `139`.
2. For each hit, inspect a bounded byte window for adjacent plausible coordinates and DrawingID-like / GraphicOID-like payloads.
3. Score candidate mapping records only when the same `field_x` appears with stable surrounding structure across multiple objects.
4. Populate `SheetObjectGeometryHint { field_x, offset, position?, graphic_oid?, note? }` only when the record shape is repeatable.
5. Keep endpoint records at `Unknown + ProbeOnly` until both endpoint ids resolve to mapping DTOs with byte provenance.
6. Add a test that asserts endpoint probe count remains 59 until coordinates are proven.

