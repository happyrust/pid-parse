# Phase 30: `radsrvitem.dll` JSite / JSitesList IDA Refresh

> Read-only IDA refresh on 2026-06-12 using the reachable
> `radsrvitem.dll` instance (`127.0.0.1:13338`). This document records
> what can be confirmed with the currently available IDB and what remains
> gated on other modules.

## Availability

`ida-pro-mcp list_instances` currently exposes:

| Binary | Port | Relevance |
|---|---:|---|
| `core.dll` | 13337 | E3D/core side; not the SmartPlant writer target for this slice. |
| `radsrvitem.dll` | 13338 | Reachable and selected for this refresh. |

Still not open / not reachable: `style.dll`, `J2DSrv.dll`, `sppid.dll`,
`XCeedRAD.dll`, `smartplantpid.exe`.

## Search Results

| Search | Result |
|---|---|
| `JSitesList` | 0 hits |
| `OLEM` | 0 hits |
| `JSite` | many hits; key code hits at `sub_56448A10`, `sub_56448A70`, `sub_5646FF60` |

Interpretation: the current IDB can confirm `JSite<N>` storage naming
and opening paths, but does not contain the `/JSitesList` `"OLEM"` stream
writer/reader literal.

## Confirmed JSite Storage Naming Paths

### `sub_56448A10`

Prototype recovered by IDA:

```text
int __stdcall sub_56448A10(wchar_t *Buffer, size_t BufferCount)
```

Core behavior:

```text
v2 = sub_564472F0();
_snwprintf_s(Buffer, BufferCount, -1, L"%s%d", L"JSite", *v2);
```

This function formats `JSite<id>` where the id is read from the pointer
returned by `sub_564472F0`.

### `sub_56448A70`

Prototype recovered by IDA:

```text
int __stdcall sub_56448A70(wchar_t *Buffer, size_t BufferCount)
```

Core behavior:

```text
v2 = sub_56448970();
_snwprintf_s(Buffer, BufferCount, -1, L"%s%d", L"JSite", v2);
```

This is a second storage-name builder: it also formats `JSite<id>`, but
the id is recovered through `sub_56448970`.

### `sub_5646FF60`

Core behavior:

```text
_snwprintf_s(Buffer, 0xB, -1, L"%d", a1);
wcsncpy_s(Destination, 0x20, L"JSite", -1);
wcsncat_s(Destination, 0x20, Buffer, -1);
return (*(a3->vtable + 24))(a3, Destination, 0, 18, 0, 0, a2);
```

This function takes an integer `a1`, constructs `JSite<a1>`, and passes
that storage name to a COM storage method. Callers include
`sub_5645FF00`, `sub_56460330`, `sub_56460760`, and `sub_56460960`.

## Caller Context

### `sub_5645FF00`

Relevant chain:

```text
v8 = sub_564472F0(this);
v37 = *v8;
...
sub_5646FF60(v37, &pStg, v42);
pStg->OpenStream(pStg, off_5667B048, ...);
```

This uses the recovered id to open a `JSite<id>` storage, then opens a
child stream inside that storage. This supports the Phase 29-M parser's
decision to treat `/JSitesList` values as storage ids by evidence, while
still leaving the model field named `entries`.

### `sub_56460330`

Relevant chain:

```text
v5 = sub_564472F0(this);
v3 = *v5;
...
sub_5646FF60(v3, &pStg, v16);
ReadClassStg(pStg, &pclsid);
```

This again derives a numeric id, opens `JSite<id>`, and inspects the
storage class. The function then loads `raslink.dll`, suggesting
symbol/link storage handling around JSite packages.

## Relationship To Phase 29-M

Phase 29-M proved from fixtures that `/JSitesList` is:

```text
"OLEM" + u32 count + u32 slot table
```

and that the first `count` slot values match `JSite<id>` storages on
6/6 local fixtures.

This IDA refresh strengthens that conclusion:

- `radsrvitem.dll` contains multiple storage-name builders for
  `JSite<id>`.
- At least one path (`sub_5646FF60`) takes a numeric id and opens that
  exact storage name.
- Callers derive the id from persisted/runtime record context and then
  open `JSite<id>` storage.

However, the current IDB does **not** contain `JSitesList` or `"OLEM"`;
therefore the Phase 29-M DTO should remain:

```text
entries: Vec<u32>
trailing_slots: Vec<u32>
```

and should **not** be renamed to `jsite_ids` yet.

## `0x0089` Record Evidence Reminder

The previously analyzed `sub_5644B640` remains relevant:

```text
lookup record pointer via manager vtable +0xA4;
if (*record == 137) { ... }
```

This confirms that `0x0089` is a real persisted/runtime record type
filter in `radsrvitem.dll`. It does not yet name the record family or
explain the DA / PSMcluster0 payload layouts.

## Still Gated

The following remain blocked until additional IDBs are opened:

- `"OLEM"` writer / reader and count-vs-slot stale-tail semantics.
- Formal rename of `/JSitesList.entries` to `jsite_ids`.
- `0x0089` record type semantic name and payload layout.
- `PSMspacemap` `tseg` page layout.
- `StyleCluster` prefix layout.
- `0x0010` discriminator and Read/DoIO semantics.

Recommended next external action: open at least one of `style.dll`,
`J2DSrv.dll`, `sppid.dll`, `XCeedRAD.dll`, or `smartplantpid.exe`, then
rerun the targeted searches for `OLEM`, `JSitesList`, `tseg`, style
prefix symbols, and RAD/JStyle persistence paths.
