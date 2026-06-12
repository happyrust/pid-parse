# Phase 30 IDA-Gated Next Actions

> Current status after the 2026-06-12 `radsrvitem.dll` refresh. This is
> a handoff checklist for the next time additional IDBs are available.

## Current IDB Availability

Only these IDBs were reachable:

| Binary | Port | Useful result |
|---|---:|---|
| `core.dll` | 13337 | Not the SmartPlant writer target for current PID format questions. |
| `radsrvitem.dll` | 13338 | Exhausted for low-cost JSite / `0x0089` / PSMspacemap / JStyle checks. |

Not available:

- `style.dll`
- `J2DSrv.dll`
- `sppid.dll`
- `XCeedRAD.dll`
- `smartplantpid.exe`

## Confirmed From `radsrvitem.dll`

### JSite / JSitesList Evidence

Confirmed:

- `sub_56448A10` formats `JSite<id>` from `*sub_564472F0()`.
- `sub_56448A70` formats `JSite<id>` from `sub_56448970()`.
- `sub_5646FF60` receives an integer id, constructs `JSite<id>`, and
  opens that storage.
- `sub_5645FF00` / `sub_56460330` call this storage-open helper after
  deriving the id from record/runtime context.

Still gated:

- no `JSitesList` literal in `radsrvitem.dll`;
- no `"OLEM"` literal in `radsrvitem.dll`;
- no writer/reader evidence for stale tail slots.

Parser implication:

- keep `JSitesListDecoded.entries` / `trailing_slots`;
- do not rename `entries` to `jsite_ids` yet.

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

Confirmed negative:

- `StyleCluster` 0 hits.
- `JStyleOverride` 0 hits.
- `JStyleBase::IJPersistImp` hits are interface thunks / RTTI, not direct
  persistence bodies.

Still gated:

- `0x0030` persistence fields;
- StyleCluster prefix layout;
- JStyleOverride class-specific load/save behavior.

Parser implication:

- Phase 16/17 conclusion remains: `0x0030` is `JStyleOverride`, not arc
  geometry;
- deeper fields still need `style.dll` / `J2DSrv.dll` or another
  writer/reader-side IDB.

## Next IDB Search Checklist

When a new relevant IDB is opened, run these first:

| Question | Search terms |
|---|---|
| `/JSitesList` writer | `JSitesList`, `OLEM`, `JSite`, `OpenStream`, `CreateStream` |
| stale tail semantics | `JSitesList`, slot/count nearby code, array resize/write paths |
| DA / `0x0089` semantic name | `137`, `0x89`, `P&IDAttributes`, `Dynamic Attributes`, `RAD_OBJECT_TYPE` |
| PSMspacemap raw page layout | `PSMspacemap`, `GetSpaceMapSegment`, `Segment::`, `0x2000`, `<< 13`, `0x1FFF` |
| StyleCluster prefix | `StyleCluster`, `JStyleBase`, `JStyleOverride`, `IJPersistImp`, `0x30` |
| `0x0010` discriminator | `0x10`, `16`, `GraphicGroup`, `JStyleOverride`, sub-record read loops |

## Current Recommendation

Do not continue broad searches in the currently open `radsrvitem.dll`
unless a new concrete string/function clue appears. The next productive
step is either:

- open one of the gated IDBs and run the checklist above; or
- keep parser behavior unchanged and submit/review the accumulated
  Phase 29/30 documentation + parser work as-is.
