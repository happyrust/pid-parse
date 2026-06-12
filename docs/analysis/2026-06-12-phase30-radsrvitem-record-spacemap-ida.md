# Phase 30: `radsrvitem.dll` `0x0089` / PSMspacemap IDA Refresh

> Read-only IDA refresh on 2026-06-12 using the reachable
> `radsrvitem.dll` instance (`127.0.0.1:13338`). This note follows the
> JSite refresh and records two additional gated findings: the RAD export
> behavior for `0x0089` records and the PSMspacemap segment/entry handle
> layout.

## `0x0089` Record Export Path

### `sub_5644B640`

`sub_5644B640` has no normal code callers in IDA and is referenced from
`.rdata:5665F280`, so it is likely an interface/vtable method.

When called with `a3 == 0`, it enumerates a record-id list:

```text
v12 = record list from sub_56455240(...)
for each record id in v12:
  record = manager->vtable[0xA4](record_id, 64)
  if (*record == 137):
    optionally append record pointer to output array
    count += 1
```

This confirms `0x0089` is a real runtime/persisted record type filter.

When called with `a3 == 1`, it calls `sub_56445F40` to export a single
record.

### `sub_56445F40`

`sub_56445F40` dispatches by `*record`:

| Type | Helper | Meaning in current IDB |
|---:|---|---|
| `0x00FA` / 250 | `sub_56446020` | `igSimpleDependencyObject` |
| `0x004D` / 77 | `sub_564468B0` | `igTextBox` |
| `0x003D` / 61 | `sub_564464D0` | `igSmartFrame2d` / OLE frame |
| default | `sub_564462F0` | generic RAD export path |

`0x0089` is not one of the special cases, so it flows through
`sub_564462F0`.

### `sub_564462F0`

Default export path:

```text
type_name = sub_56448F70(record)
if type_name == "":
  type_name = decimal string of *record
sub_564459D0(..., type_name, ...)
```

`sub_564459D0` writes:

```text
RADObjectProperties
RAD_OBJECT_TYPE = type_name
```

Then, for related records that pass `sub_56449950`, it writes their
record ids as `"RELEATIONS"` values.

### `sub_56448F70`

`sub_56448F70` is a type-code to RAD type-name table. It includes the
known IGDS / SmartPlant geometry classes:

| Code | Name |
|---:|---|
| `0x0018` | `igLine2d` |
| `0x004D` | `igTextBox` |
| `0x005E` | `igPoint2d` |
| `0x0084` | `igLineString2d` |
| `0x00CE` | `igSymbol2d` |
| `0x00FA` | `igGraphicGroup` / dependency-special path |

`0x0089` is not mapped. Therefore this RAD export helper labels a
`0x0089` record as:

```text
RAD_OBJECT_TYPE = "137"
```

and does not parse the DA/PSMcluster0 head fields or ASCII class-name
payload.

### Practical Conclusion

This is a useful boundary:

- `0x0089` is confirmed as a runtime/persisted record type.
- The reachable `radsrvitem.dll` export path does **not** reveal its
  semantic name or payload layout.
- DA/PSMcluster0 head-field surfacing should remain gated on another
  writer/reader path, not upgraded from this evidence alone.

## PSMspacemap / Segment Handle Evidence

### `PSMspacemap` storage paths

String search found `PSMspacemap` references in:

| Function | Role |
|---|---|
| `sub_56469950` | load/open path for `PSMclustertable`, `PSMroots`, `PSMspacemap`, `PSMcluster0` |
| `sub_56469BF0` | alternate load path; also opens `PSMspacemap` |
| `sub_5646AE30` | save path; logs `ImpIPersistStorage::Save` and uses `m_pSpaceMapStorageDuringSave` |
| `sub_5646B3A0` | storage attach/switch path for `PSMclustertable`, `PSMroots`, `PSMspacemap` |
| `sub_5648C370` | `ClusterTable::GetSpaceMapSegment()` |

No literal `tseg` string was present in the current IDB.

### `sub_5648C370`: segment selection

`sub_5648C370` selects or allocates a spacemap segment for a cluster
table entry.

Observed in-memory cluster/segment fields:

| Expression | Meaning |
|---|---|
| `*(v7 + 20)` | count of segment ids attached to a cluster table entry |
| `*(v7 + 24)` | array of `u32` segment ids |
| `v9[0]` | segment manager count |
| `v9[1]` | segment pointer array |
| `segment + 12` | `m_iNext` / next available entry index |
| `segment + 22` | segment flags |

Selection behavior:

- With `a4 & 4`, it scans existing segment ids for a reusable segment.
- It skips segments with flag bit `0x08`.
- It calls `sub_56479EE0(segment)` to decide whether a segment remains
  usable.
- If no existing segment can be used, it calls `sub_56479210(&segment_id)`
  to allocate/register a new segment and appends the id to the cluster
  entry's segment-id array.

### Segment exhaustion and free-list behavior

`sub_56479AC0(segment)` returns `*(u16 *)(segment + 12)`, logged as
`m_iNext`.

`sub_56479EE0(segment)`:

```text
free_count_or_cursor = (segment[20] == 0xFFFF) ? segment[10] : segment[20]
if m_iNext < 0x2000 || free_count_or_cursor != 0:
  return usable
segment.flags |= 0x08
return unusable
```

So `0x2000` is the segment entry capacity threshold. A segment can still
be reusable past `m_iNext >= 0x2000` only when its free/reuse list is
non-empty.

`sub_56479DD0(segment)` pops a reusable entry index from the free-list
state at `segment + 10`, `segment + 16`, and `segment + 20`.

### Handle encoding

`sub_56479040(segment_id, entry_index)` is the key layout proof:

```text
if entry_index >= 0x2000:
  return 0
return (segment_id << 13) | entry_index
```

`sub_56479C20(handle, out)` reverses it:

```text
segment_id = handle >> 13
entry_index = handle & 0x1FFF
```

If `entry_index` cannot be used, it advances through linked/flagged
segments with `sub_56479D10` and returns a newly encoded handle via
`sub_56479040`.

### Practical Conclusion

This confirms the high-level `PSMspacemap` handle model:

```text
handle = (segment_id << 13) | entry_index
entry_index range = 0..0x1FFF
segment capacity = 0x2000 entries
```

It does **not** yet prove the raw on-disk byte layout of a
`PSMspacemap` stream/page. The observed in-memory segment object has
fields at offsets `+10`, `+12`, `+16`, `+20`, and `+22`, and entry
pointers are addressed as `entry_base + 4 * entry_index`, but the current
IDA evidence stops short of a direct stream-page struct definition.

Recommended parser action remains conservative:

- use the `(segment << 13) | entry` model only as analysis evidence;
- do not claim `PSMspacemap` page bytes as decoded until a direct stream
  reader/writer function or controlled fixture ties these in-memory
  fields to the persisted page layout.
