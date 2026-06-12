# Phase 30: `OLESITE.dll` JSitesList / JOLEMembassy IDA Refresh

> Read-only IDA refresh on 2026-06-12 after opening
> `E:\reverse\pid\OLESITE.dll` as IDA MCP instance `127.0.0.1:13344`.

## Instance

| Binary | Port | Path |
|---|---:|---|
| `OLESITE.dll` | 13344 | `E:\reverse\pid\OLESITE.dll.i64` |

Survey:

- 32-bit PE, base `0x10000000`, image size `0x64000`.
- 1,563 functions, 498 strings.
- Exports include `GetSiteManager`, `JConvertSitesR1ToR2`,
  `JSite` constructors/operators, `JSite::GetRootStorage`,
  `JSite::SaveLinkedObjects`, and many `JSite` interface methods.

## Search Results

| Query | Result |
|---|---|
| `JSitesList` | initially 0 via simple string search, later resolved via pointer `off_1005BBC8` |
| `OLEM` | 9 RTTI / type-info hits |
| `JSite` | 58 hits |
| `IJPersist` | 8 hits |
| `Storage` | 6 hits |
| `PSMspacemap` | 0 hits |
| `StyleCluster` | 0 hits |
| `JStyleOverride` | 0 hits |
| `GraphicGroup` | 0 hits |

## JSitesList Storage Name

`off_1005BBC8` points to a UTF-16 storage/stream name:

```text
0x1005BBC8 -> 0x10050F7C -> "JSitesList"
```

`off_1005BBD0` points to:

```text
0x1005BBD0 -> 0x10050B24 -> "JSite"
```

The `JOLEMembassy` persistence bodies use these pointers while
serializing site metadata through `IStorage` / stream-like interfaces.

## JOLEMembassy Persistence Dispatch

`sub_1001DFC0` is the versioned dispatcher for `JOLEMembassy`
persistence:

```text
version 1 -> sub_1001D2C0(a1 - 20, io_context)
version 2 -> sub_1001D7F0(a1 - 20, io_context)
version 3 -> sub_1001DCC0(io_context)
```

It obtains the object version via:

```text
IOContext::GetObjectVersions(io_context, object_guid, &major, &minor)
```

## JSitesList Body Shape

`sub_1001D2C0` and `sub_1001D7F0` are the useful `JSitesList` persistence
bodies. Both:

- call `GetPersistManager(this + 12, ...)`;
- branch on `IOContext` flags;
- read/write a one-byte presence flag through `IOContext::DoIO`;
- open the `JSitesList` storage/stream through the persist manager using
  `off_1005BBC8[0]`;
- read/write a 4-byte count;
- iterate entries that correspond to `JSite` ids / objects.

The version-2 body (`sub_1001D7F0`) first reads:

```text
DoIO(io_context, 1, &present)
```

Then it opens `JSitesList`, reads two 4-byte values, treats the second as
an entry count, and iterates that count. The entry path uses the
`JSite` name pointer and delegates object persistence through jengine
interfaces.

## JSite Persistence Relationship

`JSite` is a persisted class in this module:

- constructors/operators are exported for both `IJPersistManager` and
  `IJPersistCluster` allocation paths;
- `JSite::operator new(... IJPersistManager ...)` calls
  `AllocPersistMemInDefault(a1 + 4, manager, clsid, version, flags, out)`;
- `JSite::operator new(... IJPersistCluster ...)` calls
  `AllocPersistMem(a1 + 4, cluster, clsid, version, flags, out)`.

`JSite::GetRootStorage` is not a parser for `/JSitesList`; it only asks
the persist manager for root storage and returns it through the vtable.

## Parser Implication

This pass upgrades the confidence of `/JSitesList`:

- the list really is a `JSitesList` storage/stream, not just an inferred
  list of integers from fixture bytes;
- its entries really correspond to `JSite` objects / ids;
- `JSitesListDecoded.entries` can be described as JSite ids with stronger
  evidence, while preserving `trailing_slots` for stale/unclaimed tail
  bytes until writer behavior is fully mapped.

No immediate Rust parser change is required unless we choose to rename or
document fields. The existing parser shape remains conservative:

- `entries`: observed count-bounded `JSite` entries;
- `trailing_slots`: post-count aligned tail that is not claimed as active
  entries.

Still gated:

- exact stale-tail writer semantics;
- whether all trailing slots are deleted/stale `JSite` ids or allocator
  residue;
- `PSMspacemap` raw page layout;
- `StyleCluster` prefix layout;
- `0x0010` discriminator and `GraphicGroup` payload.

## Next Actions

1. The docs-only terminology update is complete in
   `docs/specs/2026-06-08-pid-file-format-spec-kit/data-model.md`:
   `JSitesListDecoded.entries` are described as IDA-backed `JSite`
   entries / ids.
2. If code is changed later, keep backwards-compatible JSON field names unless
   there is a broader schema migration.
3. `OLECRT.dll` was attempted after this pass but did not register as an
   IDA MCP instance, so no survey/search was possible. For PID-specific
   PSM/style gaps, a true SmartPlant P&ID module remains the higher-value
   target.
