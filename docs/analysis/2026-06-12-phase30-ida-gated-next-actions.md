# Phase 30 IDA-Gated Next Actions

> Current status after the 2026-06-12 `radsrvitem.dll` refresh. This is
> a handoff checklist for the next time additional IDBs are available.

## Current IDB Availability

Initial `radsrvitem.dll` refresh only had these IDBs:

| Binary | Port | Useful result |
|---|---:|---|
| `core.dll` | 13337 | Not the SmartPlant writer target for current PID format questions. |
| `radsrvitem.dll` | 13338 | Exhausted for low-cost JSite / `0x0089` / PSMspacemap / JStyle checks. |

Later in the same phase, `E:\reverse\pid\style.dll` was opened and
registered as an IDA MCP instance. See
`2026-06-12-phase30-style-dll-jstyleoverride-ida.md`.

Later sweeps also opened SmartSketch / RAD runtime modules under
`E:\reverse\pid`. `OLESITE.dll` provided direct `JSitesList` evidence.
See `2026-06-12-phase30-secondary-idb-sweep.md` and
`2026-06-12-phase30-olesite-jsiteslist-ida.md`.

`OLECRT.dll` was launched in IDA during the final local OLE follow-up and
initially did not register as an IDA MCP instance. It later became reachable
as instance `127.0.0.1:13353` and was checked in Phase 31. See
`2026-06-13-phase31-olecrt-storage-entrypoints.md`.

`OLECRT.dll` confirms a generic OLE / embedded symbol-object layer:
`StgOpenStorageEx`, `DocVersion2`, `SymbolInformationCluster`,
`GetPersistManager`, `GetPersistCluster`, and `UnBindSheetWrappers2`.
It still does not expose `JSitesList`, `PSMspacemap`, `StyleCluster`,
`GraphicGroup`, `IOContext`, or `DoIO` reader bodies.

`smartplantpid.exe` was later provided under the repository `dlls`
directory and opened as IDA MCP instance `127.0.0.1:13345`. It appears to
be a VB6 front-end / launcher (`MSVBVM60` imports, strings such as
`SmartPlantPID`, `Smart Plant P&ID`, `sppid`, `Registry`, and
`ErrorLogging`), not the low-level `.pid` persistence module. See
`2026-06-12-phase30-smartplantpid-exe-ida.md`.

Additional SPPID application / automation modules were opened afterward:
`sppid.dll` (13346), `sppidautomation.dll` (13347),
`sppiddwgprocess.dll` (13348), `ipidobjectmanagerinf.dll` (13349),
`sppidautomation.exe` (13350), `sppidautomationwrap.dll` (13351), and
`llama.dll` (13352). They add application, archive/workshare, automation,
interface, and logical-model context, but still do not expose the raw CFBF
stream / `IOContext` persistence reader names needed for the remaining
byte-layout questions. See
`2026-06-12-phase30-sppid-backend-idb-sweep.md`.

Still not available from a true SmartPlant P&ID install:

- lower-level backend DLL / COM module that contains PID storage readers

## Confirmed From `radsrvitem.dll`

### JSite / JSitesList Evidence

Confirmed from `radsrvitem.dll`:

- `sub_56448A10` formats `JSite<id>` from `*sub_564472F0()`.
- `sub_56448A70` formats `JSite<id>` from `sub_56448970()`.
- `sub_5646FF60` receives an integer id, constructs `JSite<id>`, and
  opens that storage.
- `sub_5645FF00` / `sub_56460330` call this storage-open helper after
  deriving the id from record/runtime context.

Confirmed from `OLESITE.dll`:

- `off_1005BBC8 -> "JSitesList"`.
- `off_1005BBD0 -> "JSite"`.
- `sub_1001DFC0` dispatches versioned `JOLEMembassy` persistence:
  version 1 to `sub_1001D2C0`, version 2 to `sub_1001D7F0`, version 3
  to `sub_1001DCC0`.
- `sub_1001D2C0` / `sub_1001D7F0` open `JSitesList`, read/write count
  fields, and iterate `JSite` entries through jengine persistence
  interfaces.

Still gated:

- exact writer semantics for stale/trailing slots.

Parser implication:

- `JSitesListDecoded.entries` are now IDA-backed as `JSite` ids /
  entries;
- keep the serialized field names `entries` / `trailing_slots` unless a
  broader schema migration is intentionally made;
- keep `trailing_slots` conservative until stale-tail writer behavior is
  mapped.

### `0x0089` Evidence

Confirmed:

- `sub_5644B640(a3=0)` enumerates record ids and filters runtime records
  by `*record == 137`.
- `sub_5644B640(a3=1)` exports one record via `sub_56445F40`.
- `0x0089` falls through `sub_56445F40` default path to `sub_564462F0`.
- `sub_56448F70` does not map `0x0089`; the export helper writes
  `RAD_OBJECT_TYPE = "137"`.

Still gated:

- semantic class/family name for the `0x0089` DA/PSMcluster0 records;
- persisted head fields (`record_id`, `field_x`, `class_id`, class name)
  beyond the on-disk byte-pattern evidence.

Parser implication:

- keep DA/PSMcluster0 head-field surfacing conservative;
- do not promote non-signature records' head fields to decoded semantics
  from this IDB alone.

### PSMspacemap Evidence

Confirmed:

- `sub_5648C370` is `ClusterTable::GetSpaceMapSegment()`.
- cluster table entries maintain a segment-id array.
- segments are reused unless exhausted or flagged with `0x08`.
- `sub_56479040(segment_id, entry_index)` encodes:

```text
handle = (segment_id << 13) | entry_index
```

- `sub_56479C20(handle, out)` decodes:

```text
segment_id = handle >> 13
entry_index = handle & 0x1FFF
```

- segment capacity is `0x2000` entries.

Still gated:

- raw `/PSMspacemap` stream/page byte layout;
- direct mapping from in-memory segment object offsets to persisted page
  fields.

Parser implication:

- use handle encoding as analysis evidence only;
- do not mark PSMspacemap page bytes as decoded until a direct
  stream-reader/writer path or controlled fixture ties the model to bytes.

### Style / JStyle Evidence

Confirmed from the original `radsrvitem.dll` pass:

- `StyleCluster` 0 hits.
- `JStyleOverride` 0 hits.
- `JStyleBase::IJPersistImp` hits are interface thunks / RTTI, not direct
  persistence bodies.

Confirmed from the later `style.dll` pass:

- `JStyleOverride` strings and vtables are present.
- `stru_10066B64` is `47fcc338-2d0f-11d0-a1ff-080036a1cf02`.
- `sub_1000F030` is the current `JStyleOverride` persistence body:
  after common style setup it performs 13 `IOContext::DoIO` calls for a
  total 64-byte payload.
- `sub_1000F210` is the versioned persistence path for the same CLSID.
- `sub_10010640` clones a wider runtime byte region with
  `qmemcpy(v5 + 22, this + 22, 0x58)` and clears transient pointer-like
  slots.

Still gated:

- semantic names for the individual `0x0030` persistence fields;
- StyleCluster prefix layout;
- JStyleOverride fields beyond the 64-byte disk sequence / runtime slot
  relationship.

Parser implication:

- Phase 16/17 conclusion remains: `0x0030` is `JStyleOverride`, not arc
  geometry;
- the existing 64-byte Rust decoder is now directly backed by
  `style.dll` IDA evidence;
- field naming, StyleCluster prefix, and storage writer semantics still
  need `J2DSrv.dll`, `sppid.dll`, or another lower-level writer/reader-side
  backend IDB.

## Next IDB Search Checklist

When a new relevant IDB is opened, run these first:

| Question | Search terms |
|---|---|
| `/JSitesList` writer | `OLESITE.dll`: `sub_1001D2C0`, `sub_1001D7F0`, `sub_1001DFC0`, `off_1005BBC8` |
| stale tail semantics | `JSitesList`, count nearby code, deleted/stale `JSite` slot write paths |
| DA / `0x0089` semantic name | `137`, `0x89`, `P&IDAttributes`, `Dynamic Attributes`, `RAD_OBJECT_TYPE` |
| PSMspacemap raw page layout | `PSMspacemap`, `GetSpaceMapSegment`, `Segment::`, `0x2000`, `<< 13`, `0x1FFF` |
| StyleCluster prefix | `StyleCluster`, style storage open/create paths, style catalog save/load code |
| `0x0010` discriminator | `0x10`, `16`, `GraphicGroup`, `JStyleOverride`, sub-record read loops |

## Current Recommendation

Do not continue broad searches in the currently open local runtime IDBs
unless a new concrete string/function clue appears. The next productive
step is either:

- open a lower-level product DLL / COM module that directly references raw
  CFBF stream names, jengine `IOContext`, or persist-manager APIs; or
- keep parser behavior unchanged and submit/review the accumulated
  Phase 29/30 documentation + parser work as-is.
