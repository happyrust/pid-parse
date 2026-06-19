# 2026-06-19 IDA Evidence Baseline

Feature: `collect-ida-evidence-baseline`

Scope: read-only `ida-pro-mcp` evidence for the currently available IDB
instances. This note records current instance availability, focused positive
and negative searches for raw SmartPlant `.pid` persistence evidence, scoped
historical evidence, and confidence impact for atlas and roadmap rows.

## Read-only boundary

Only read-only IDA MCP tools were used:

- `list_instances`
- `select_instance`
- `server_health`
- `server_warmup`
- `survey_binary`
- `find_regex`
- `search_text`
- `func_query`
- `imports_query`

No IDB mutation tools were used. In particular, this pass did not call
`patch`, `patch_asm`, `put_int`, `rename`, `set_comments`,
`append_comments`, `set_type`, `type_apply_batch`, `define_func`,
`define_code`, `undefine`, `declare_type`, `enum_upsert`, `declare_stack`,
`delete_stack`, or `idb_save`.

## Current IDA instances

Initial `ida-pro-mcp___list_instances` returned:

| Instance | Host/port | IDB path | Reachable | Active | Role | Confidence impact |
|---|---:|---|---|---|---|---|
| `sppid.dll` | `127.0.0.1:13337` | `D:\work\plant-code\cad\pid-parse\dlls\sppid.dll.i64` | yes | yes | SmartPlant P&ID VB6/COM application glue | Negative raw-persistence evidence. Does not justify parser confidence promotion. |
| `core.dll` | `127.0.0.1:13338` | `D:\AVEVA\Everything3D3.1\core.dll.i64` | yes initially, temporarily slow/unresponsive during heavier queries, recovered for health and focused search | no initially | AVEVA E3D broad platform/core module | Broad platform semantics only. Not treated as raw SmartPlant `.pid` byte-layout evidence. |

`sppid.dll` health:

| Field | Value |
|---|---|
| `status` | `ok` |
| `module` | `sppid.dll` |
| `input_path` | `d:\work\plant-code\cad\pid-parse\dlls\sppid.dll` |
| `imagebase` | `0x54880000` |
| `auto_analysis_ready` | `true` |
| `hexrays_ready` | `true` |
| `strings_cache_ready` | `true` |
| `strings_cache_size` | `204` |

`core.dll` health after recovery:

| Field | Value |
|---|---|
| `status` | `ok` |
| `module` | `core.dll` |
| `input_path` | `D:\AVEVA\Everything3D3.1\core.dll` |
| `imagebase` | `0x5170000` |
| `auto_analysis_ready` | `true` |
| `hexrays_ready` | `true` |
| `strings_cache_ready` | `true` |
| `strings_cache_size` | `52255` |

## `sppid.dll` current evidence

`ida-pro-mcp___survey_binary` on `sppid.dll` reported:

| Field | Value |
|---|---|
| Arch | 32-bit |
| Image size | `0x14000` |
| Functions | 158 total, 129 named, 29 unnamed |
| Strings | 204 |
| Notable imports | dominated by `MSVBVM60` VB runtime imports |
| Notable strings | `sppiddll`, `PIDConstants`, `StartupClass`, `regRead`, `appObj`, `appObjOuter`, `RAD2D` |

Interpretation: the open `sppid.dll` instance is a VB6/COM application
layer. It exposes startup, registry, and RAD2D type-library glue, but not
the low-level persistence reader/writer layer for raw `.pid` storage.

### `sppid.dll` focused search trace

| Module | Tool | Search target | Result | Linked row | Confidence impact |
|---|---|---|---|---|---|
| `sppid.dll` | `find_regex` | `JSitesList|PSMspacemap|StyleCluster|JStyleOverride|GraphicGroup|P&IDAttributes|Dynamic Attributes|TaggedTxtData|AppObject|PSMroots|JTaggedTxtStgList|IOContext|DoIO|PersistCluster|GetPersistManager|RAD_OBJECT_TYPE|DocVersion2|DocVersion3|Sheet[0-9]*` | `n = 0`, no string matches | `ROADMAP-STYLECLUSTER`, `ROADMAP-0010`, `ROADMAP-GRAPHICGROUP`, `ROADMAP-DA-0089`, `ROADMAP-PSMSPACEMAP`, `ATLAS-JSITE-JSITESLIST` | Negative. No direct raw `.pid` stream names, persistence-manager terms, or unresolved-family names in this IDB. |
| `sppid.dll` | `search_text` | `DoIO` | `n = 0`, no rendered listing hits | `ROADMAP-0010`, `ROADMAP-GRAPHICGROUP`, `ROADMAP-PSMSPACEMAP` | Negative. No `IOContext::DoIO` reader/writer body was exposed. |
| `sppid.dll` | `search_text` | `IOContext` | `n = 0`, no rendered listing hits | `ROADMAP-0010`, `ROADMAP-GRAPHICGROUP`, `ROADMAP-PSMSPACEMAP` | Negative. No IOContext surface was exposed. |
| `sppid.dll` | `func_query` | function filter `*DoIO*` | `0` functions | `ROADMAP-0010`, `ROADMAP-GRAPHICGROUP`, `ROADMAP-PSMSPACEMAP` | Negative. No named DoIO function surface in this module. |
| `sppid.dll` | `imports_query` | import filter `*DoIO*` | `0` imports | `ROADMAP-0010`, `ROADMAP-GRAPHICGROUP`, `ROADMAP-PSMSPACEMAP` | Negative. No imported DoIO helper in this module. |

### `sppid.dll` conclusion

Current `sppid.dll` searches did not expose direct raw `.pid` persistence
strings, `IOContext::DoIO` reader/writer bodies, or unresolved-family
readers for `PSMspacemap`, `StyleCluster`, `0x0010`, `GraphicGroup 0x00FA`,
`JSitesList.trailing_slots`, `0x0089`/class 137 DA heads, or page transform
semantics. Therefore no parser confidence upgrade is justified from
`sppid.dll` alone.

## `core.dll` current evidence

`core.dll` is reachable at `127.0.0.1:13338`, but it is a very large AVEVA
E3D core/platform IDB. A heavier parallel query batch made the MCP endpoint
temporarily unresponsive before it recovered. To stay within the mission
boundary against broad sweeps, this pass used health, a minimal survey, and
one focused raw-persistence string search after recovery.

`ida-pro-mcp___survey_binary` with `detail_level = "minimal"` reported:

| Field | Value |
|---|---|
| Arch | 32-bit |
| Image size | `0x4113000` |
| Functions | 34,308 total, 10,147 named, 23,995 unnamed |
| Strings | 52,255 |
| Notable exported/platform symbols from first page | `MR_Message`, `DBE_ComparisonOperator`, `DBE_DirectionValue`, `DBE_OrientationValue`, `DBE_PositionValue` |

Interpretation: this is a broad platform/core module with drawing,
database, direction/orientation/position, and message semantics. It is not
automatically raw SmartPlant `.pid` byte-layout evidence.

### `core.dll` focused search trace

| Module | Tool | Search target | Result | Linked row | Confidence impact |
|---|---|---|---|---|---|
| `core.dll` | `server_health` | readiness probe | `status = ok`, auto-analysis, Hex-Rays, and string cache ready | `ATLAS-IDA-CURRENT-INSTANCES` | Positive for availability after recovery. |
| `core.dll` | `survey_binary` | minimal binary survey | 34,308 functions, 52,255 strings, broad E3D platform/core exports | `ATLAS-CORE-PLATFORM-SCOPE` | Positive only for broad platform semantics. No raw `.pid` layout conclusion. |
| `core.dll` | `find_regex` | `JSitesList|PSMspacemap|StyleCluster|JStyleOverride|GraphicGroup|P&IDAttributes|Dynamic Attributes|TaggedTxtData|AppObject|PSMroots|JTaggedTxtStgList|IOContext|DoIO|PersistCluster|GetPersistManager|RAD_OBJECT_TYPE|DocVersion2|DocVersion3` | `n = 0`, no string matches | `ROADMAP-STYLECLUSTER`, `ROADMAP-0010`, `ROADMAP-GRAPHICGROUP`, `ROADMAP-DA-0089`, `ROADMAP-PSMSPACEMAP` | Negative for direct SmartPlant raw persistence clues. Keeps `core.dll` scoped to broad platform semantics. |
| `core.dll` | `find_regex`, `search_text`, `func_query`, `imports_query` | broader `Drawing|Document|Storage|Stream|Persist|Route|Database|Symbol` and DoIO/IOContext follow-ups | timed out during heavier parallel batch | `ATLAS-CORE-PLATFORM-SCOPE` | No confidence claim is made from these timed-out calls. They are recorded as unavailable evidence, not positive or negative proof. |

### `core.dll` conclusion

`core.dll` should remain scoped to broad platform/drawing/database semantics.
It must not be used as raw SmartPlant `.pid` byte-layout evidence unless a
future focused path finds direct SmartPlant stream names, persistence-manager
clues, or concrete reader/writer bodies tied to `.pid` records.

## Historical IDA evidence preserved with scope

The following historical findings remain part of the evidence base. They are
useful because each is tied to a specific module and scope, not because they
make the whole vendor format decoded.

| Historical module/doc | Exact scope | Positive evidence | Still gated |
|---|---|---|---|
| `style.dll`, `docs/analysis/2026-06-12-phase30-style-dll-jstyleoverride-ida.md` | `JStyleOverride` only | `sub_1000F030` is the current persistence body with 13 `IOContext::DoIO` calls and a 64-byte disk payload. Supports `0x0030 = JStyleOverride`, not the retired `GArc2d` interpretation. | Individual field names, `StyleCluster` prefix layout, `0x0010`, `GraphicGroup`, and writer scope. |
| `OLESITE.dll`, `docs/analysis/2026-06-12-phase30-olesite-jsiteslist-ida.md` | `JSitesList.entries` only | `off_1005BBC8` points to UTF-16 `"JSitesList"`, `off_1005BBD0` points to `"JSite"`, and JOLEMembassy persistence iterates count-bounded `JSite` entries. | `JSitesList.trailing_slots` stale/delete/compact writer semantics. |
| `OLECRT.dll`, `docs/analysis/2026-06-13-phase31-olecrt-storage-entrypoints.md` | `DocVersion2` compatibility probe and embedded OLE/symbol bridge | `sub_1000A800` opens `DocVersion2` and reads two 16-bit values. `sub_100017C0` opens `SymbolInformationCluster` in external embedded OLE storage. | Main `.pid` stream graph, `Sheet*`, `JSitesList`, `PSMspacemap`, `StyleCluster`, `0x0010`, and `GraphicGroup` byte layouts. |
| `radsrvitem.dll`, `docs/analysis/2026-06-12-phase30-radsrvitem-record-spacemap-ida.md` | `0x0089` export boundary and `PSMspacemap` handle model | `0x0089` is a real runtime/persisted type filter but exports as `RAD_OBJECT_TYPE = "137"` through the generic path. `PSMspacemap` handle model is `handle = (segment_id << 13) | entry_index`, with entry range `0..0x1FFF` and segment capacity `0x2000`. | Semantic name/payload for `0x0089`, DA head field semantics, and raw on-disk `PSMspacemap` page layout. |

## Confidence impact summary

| Atlas or roadmap row | Current confidence impact |
|---|---|
| `ATLAS-IDA-CURRENT-INSTANCES` | Current IDA availability is documented for `sppid.dll` and `core.dll`. `core.dll` was reachable but slow under heavier searches. |
| `ATLAS-JSITE-JSITESLIST` | Historical `OLESITE.dll` evidence preserves `JSitesList.entries` as IDA-backed. No new `sppid.dll` or `core.dll` evidence changes trailing-slot scope. |
| `ATLAS-SHEET-JSTYLEOVERRIDE` | Historical `style.dll` evidence preserves `0x0030 = JStyleOverride` and the 64-byte payload scope. No current SPPID/core evidence broadens it. |
| `ATLAS-META-DOCVERSION2` | Historical `OLECRT.dll` evidence supports the small two-word `DocVersion2` probe only. |
| `ROADMAP-PSMSPACEMAP` | Historical `radsrvitem.dll` handle model is preserved, but raw page bytes stay not decoded. Current `sppid.dll` and `core.dll` searches are negative for direct page reader/writer evidence. |
| `ROADMAP-STYLECLUSTER` | Remains gated. Current `sppid.dll` and `core.dll` searches found no `StyleCluster` reader/writer evidence. |
| `ROADMAP-0010` | Remains gated. Current searches found no direct `0x0010` discriminator reader/writer path. |
| `ROADMAP-GRAPHICGROUP` | Remains `TypedAudit`/gated. Current searches found no direct `GraphicGroup` payload reader/writer path. |
| `ROADMAP-DA-0089` | Historical `radsrvitem.dll` confirms the runtime type filter but not semantic payload. Current searches found no new direct DA body reader. |
| `ROADMAP-PAGE-TRANSFORM` | No current IDA evidence establishes source coordinate space, units, direction, origin, scale, bounds, and byte provenance. Transform remains unavailable. |

## Negative closeout for this pass

This evidence pass does not justify parser or writer confidence promotion.
The current actionable conclusion is negative:

- `sppid.dll` is reachable and ready, but it behaves as VB6/COM application
  glue and lacks direct raw `.pid` persistence strings or `DoIO` bodies.
- `core.dll` is reachable and ready, but its evidence is scoped to broad E3D
  platform semantics. Focused raw SmartPlant persistence terms returned no
  matches.
- Historical IDA evidence remains valid only within its documented scope:
  `JStyleOverride`, `JSitesList.entries`, `DocVersion2`/embedded OLE, and the
  `PSMspacemap` handle model.

Future IDA work should target modules or functions that directly reference
SmartPlant CFBF stream names, `IOContext::DoIO`, persist-manager APIs, or
known unresolved family names. Broad sweeps of the current `sppid.dll` and
`core.dll` instances should not continue without a new concrete clue.
