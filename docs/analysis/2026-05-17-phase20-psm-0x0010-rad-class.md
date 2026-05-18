# PSM type `0x0010` RAD identity partial analysis

> Date: 2026-05-17  
> Scope: Phase 20 Slice B partial AC  
> IDA instances: `radsrvitem.dll` (port 13346) / `style.dll` (port 13348) plus negative spot-checks in existing reachable instances  
> Status: **partial** — GUID/type-table identity confirmed; human class name and concrete Read/IO function not yet recovered

## TL;DR

PSM type `0x0010` is no longer an anonymous byte pattern. In
`radsrvitem.dll`, the runtime `PersistTypeTable<PersistComTypeEntry>`
maps type index `0x0010` to:

```text
PSM type index: 0x0010
GUID:           1D1928C0-0000-0000-C000-000000000046
tail16:         0x40
tail17:         0x06
parent index:   0x0115
```

Entry `0x0115` uses the same GUID with `tail16=0xC0`, `tail17=0x06`,
and `parent=0`. This means `0x0010` is a child/alias entry for the same
persisted GUID, not a random sibling in the `47FCC33x` style family.

What is **not** confirmed yet:

- no human RAD/C++ class name has been recovered for GUID
  `1D1928C0-0000-0000-C000-000000000046`;
- no concrete `Read` / `Load` / `DoIO` function has been tied to this
  GUID;
- no sub-kind discriminator offset has been IDA-confirmed.

## Evidence Chain

```text
radsrvitem.dll!PSMSerializeIn sub_564915E0
        |
        | extracts packed type: ([record+8] >> 6) & 0x3FFF
        v
radsrvitem.dll!dword_567DDC90 PersistTypeTable
        |
        | initialized by sub_56445C90
        v
sub_56455D10(281, dword_5667B068, sub_56471660, 0)
        |
        | static entries: 281 x 20B
        |   16B GUID + tail16 + tail17 + parent_index
        v
entry[0x0010] @ 0x5667B1A8
        |
        | GUID 1D1928C0-0000-0000-C000-000000000046
        | parent 0x0115
        v
entry[0x0115] @ 0x5667C60C
        |
        | same GUID, root alias
        v
style.dll .rdata @ 0x10068F44 contains the same GUID
        |
        | but no direct class factory / IID xref found
        v
status: persisted type identity confirmed; concrete class name deferred
```

## Type Table Layout

`sub_56445C90` initializes the persist type table:

```text
sub_56455D10(281, dword_5667B068, sub_56471660, 0)
```

`sub_56455D10` walks `281` static entries at 20-byte stride. The
insertion routine `sub_5647C1E0` copies each entry into runtime
`PersistComTypeEntry` objects:

| Runtime field | Source | Meaning observed |
|---|---|---|
| `entry+0x00..0x0F` | static `+0..15` | GUID |
| `entry+0x10` | sequential index | type index |
| `entry+0x12` | static `+18..19` | parent type index |
| `entry+0x14` | static `+16` | tail16 / flags byte |
| `entry+0x16` | static `+17` | tail17 / category byte |

The relevant entries are:

| Index | Address | GUID | tail16 | tail17 | Parent |
|---:|---|---|---:|---:|---:|
| `0x0010` | `0x5667B1A8` | `1D1928C0-0000-0000-C000-000000000046` | `0x40` | `0x06` | `0x0115` |
| `0x0115` | `0x5667C60C` | `1D1928C0-0000-0000-C000-000000000046` | `0xC0` | `0x06` | `0x0000` |

## PSMSerializeIn Dispatch

The relevant `PSMSerializeIn` function is
`radsrvitem.dll!sub_564915E0`.

It extracts the existing packed type from the record:

```text
0x564918DE  mov eax, [ecx+8]
0x564918E7  shr eax, 6
0x564918EA  and eax, 3FFFh
0x564918EF  cmp dx, ax
```

When the packed type does not match the incoming `nType`, the function
uses `sub_564689C0(dword_567DDC90, type, &entry)` and walks the
`entry+0x12` parent chain. This explains why `0x0010` can be a
child/alias of root entry `0x0115`.

## Factory / VTable Findings

The direct factory search did **not** produce a normal COM class
factory.

Observed negative evidence:

- `sub_5647CE40` is a 19-byte default stub:
  `*out = 0; return 0x80004001`.
- `sub_5647CA50` is also an `E_NOTIMPL` default stub.
- `style.dll!DllGetClassObject` contains the positive Phase 16 control
  path for `47FCC338` / `JStyleOverride`, but not the `1D1928C0...`
  GUID.
- `style.dll` contains the `1D1928C0...` GUID at `0x10068F44`, but the
  loaded IDB has no static xrefs or pointer-to-GUID references to that
  address.

Observed positive bridge evidence:

- `dword_567DDC60` is initialized to `sub_56471660`.
- `sub_56471660` accepts `tail17` categories `1-6,8-12`.
- `0x0010` has `tail17=0x06`, so it is eligible for that callback path.
- `sub_56469100` calls `dword_567DDC60` after resolving a
  record/type entry through `sub_56468B30`.
- `sub_56468B30` resolves packed OIDs to existing record slots via
  `sub_56479970`, then either calls the resolved object's vtable or
  falls back to `SerialCluster` lazy-load logic.

Current interpretation: the visible direct factory stubs are not the
concrete class factory. The operational path is mediated by packed OID,
existing record slots, and `SerialCluster` storage access. The remaining
identity gap is the persisted type name behind GUID `1D1928C0...`.

## style.dll Probe

`style.dll` has one raw GUID hit:

| GUID | Address | Result |
|---|---|---|
| `1D1928C0-0000-0000-C000-000000000046` | `0x10068F44` | raw `.rdata` hit only |

Negative checks:

- no `DllGetClassObject` branch for `0x10068F44`;
- no static xrefs to `0x10068F44`;
- no pointer-to-GUID match for `0x10068F44`;
- executable-segment scan for `10068` operands only found references to
  `0x10068AD4` / `0x10068B84`;
- local registry queries for both `HKCR\CLSID\{1D1928C0...}` and
  `HKCR\Interface\{1D1928C0...}` returned key-not-found.

This weakens the hypothesis that the loaded `style.dll` IDB contains a
direct concrete factory for the `0x0010` persisted type.

## Cross-Fixture Relationship

Phase 18/19 fixture facts remain unchanged:

| Fact | Value |
|---|---:|
| total decoded `0x0010` audit records | `582` |
| `leading_word == 0x0002` | `164` |
| `leading_word == 0x0003` | `21` |
| `leading_word == 0x0001` | `18` |
| `leading_word == None` | `0` |

No IDA-confirmed discriminator offset has been found yet. The existing
`leading_word` field therefore remains audit-only and byte-position
named. It must not be renamed to `sub_kind`.

## Address Index

| Symbol / location | Address | Notes |
|---|---|---|
| `radsrvitem.dll!PSMSerializeIn` | `0x564915E0` | Main deserializer |
| type extraction in `PSMSerializeIn` | `0x564918DE..0x564918EF` | `([record+8] >> 6) & 0x3FFF` |
| runtime type table | `dword_567DDC90` | `PersistTypeTable<PersistComTypeEntry>` |
| type table init callsite | `0x56445D73` | calls `sub_56455D10` |
| static type table base | `0x5667B068` | 281 entries x 20B |
| static entry `0x0010` | `0x5667B1A8` | GUID `1D1928C0...` |
| static entry `0x0115` | `0x5667C60C` | same GUID, root alias |
| callback global | `dword_567DDC60` | initialized to `sub_56471660` |
| callback function | `0x56471660` | accepts category `0x06` |
| OID/slot resolver | `0x56479970` | packed OID lookup |
| factory bridge candidate | `0x56468B30` | existing object/vtable + lazy load |
| `SerialCluster` ctor | `0x56493840` | storage path |
| style.dll raw GUID hit | `0x10068F44` | no direct xrefs |
| JStyleOverride positive control | `0x10066B64` | `DllGetClassObject` branch, `sub_10001600` |

## Known Unknowns

| ID | Status | Next path |
|---|---|---|
| human persisted type name | unresolved | search external metadata / RTTI / local type names |
| concrete factory/vtable for GUID `1D1928C0...` | unresolved | continue from `SerialCluster` storage objects only if needed |
| `Read` / `Load` / `DoIO` function | unresolved | blocked on class/factory identity |
| sub-kind discriminator offset | unresolved | blocked on `Read` / `DoIO` |
| mapping from `leading_word` buckets to sub-kind names | unresolved | keep audit-only |

## Follow-up Metadata / RTTI Recon (2026-05-18)

A focused follow-up checked the recommended low-risk path for recovering a
human type name before doing more blind factory tracing.

Negative evidence:

- `style.dll` local type queries for `1D1928`, `Persist`, `Style`, and
  `Override` returned no IDA local type entries tied to the `0x0010` GUID.
- Rendered-text search for `1D1928` / `1D1928C0` returned no listing hits.
- Repository search found no external `.tlb`, `.idl`, `.drx`, `.pdb`, `.lib`,
  `.exp`, `.map`, or `.def` metadata files for the loaded SmartPlant DLLs.
- Local registry queries for `HKCR\CLSID`, `HKCR\Interface`, and `HKCR\TypeLib`
  returned key-not-found for `1D1928C0-0000-0000-C000-000000000046` and the
  adjacent `style.dll` GUID constants `09D6BBB0-0000-0000-C000-000000000046`
  / `8EC51800-0000-0000-C000-000000000046`.
- Public web/GUID database searches also did not identify
  `1D1928C0-0000-0000-C000-000000000046`, the two adjacent GUID constants, or
  the `JStyleBase` control GUID `19F333B0-4F81-11D0-A223-080036A1CF02`.

Additional IDA evidence:

- `style.dll` keeps `1D1928C0...` at `.rdata:0x10068F44` followed immediately by
  `09D6BBB0...` and `8EC51800...`; all three have the COM-style
  `C000-000000000046` tail.
- `0x10068F44` still has **zero xrefs**.
- Nearby GUID-array addresses `0x10068AD4` and `0x10068B84` are referenced by
  default style / exception-style creation functions such as
  `HGetDimExceptionStyle`, `sub_1003E0D0`, `sub_10059060`, and `sub_10059D00`.
  Those paths call `JCoCreateInstance` for known style helpers and do not create
  an xref to `1D1928C0...`.
- The GUID block is followed by named `ClassFactory<VJStyle...>` vtables and
  style strings (`styles.drx`, `JStyleOverride::...`, `JStyleBase::IJPersistImp`),
  but there is no ordering evidence strong enough to assign a class name to
  `1D1928C0...`.

Conclusion: the metadata / RTTI / registry path did **not** recover a human
persisted type name. The safest current interpretation remains:
`1D1928C0...` is a persisted/interface GUID constant present in `style.dll`, but
not an activatable or named class in the loaded IDB. Typed `0x0010` DTO work
remains blocked on stronger Read/DoIO evidence.

## Follow-up Read/DoIO Tracing (2026-05-18)

After the metadata path failed, a second focused follow-up traced the two
least-risky Read/DoIO candidates: the `radsrvitem.dll` `SerialCluster` path and
the `style.dll` `IJPersist` path.

`radsrvitem.dll` findings:

- `sub_56468B30` resolves packed OIDs to record slots, checks type-entry flags,
  and can lazy-load a `SerialCluster`.
- `sub_5648BBA0` constructs/uses `SerialCluster` and then calls
  `sub_56490B30` or `sub_56491090` to open named persisted children.
- `sub_56490B30` / `sub_56491090` are storage accessor wrappers: they validate a
  short wide name, then call a storage-like vtable at `+12` / `+16` with
  constants `4114` / `18`.
- The resulting objects are handed to `SerialCluster` vtable slots `+64` /
  `+68`, but this path still does not identify the concrete class or
  `Read` / `DoIO` sequence for GUID `1D1928C0...`.

`style.dll` findings:

- `JStyleBase::IJPersistImp` has full RTTI and an inspectable vtable at
  `.rdata:0x1006E9AC`.
- Its versioned read helper `sub_10057B30` calls
  `IOContext::GetObjectVersions` with GUID
  `19F333B0-4F81-11D0-A223-080036A1CF02`, not `1D1928C0...`.
- When the object version is 2 or 3, `sub_10057B30` calls `sub_10057350`, which
  performs two `IOContext::DoIO(..., 2, ...)` reads/writes against
  `JStyleBase` fields.

Conclusion: the `style.dll` route proves the method for recovering a
version-gated `IJPersist` `DoIO` sequence, but the recovered sequence belongs to
`JStyleBase` (`19F333B0...`) rather than the PSM `0x0010` persisted GUID
`1D1928C0...`. This is useful control evidence, not sufficient DTO evidence for
`0x0010`.

## Phase 21 Implications

Do not implement typed `0x0010` sub-kind DTOs from this evidence alone.
The stable facts that can be used safely are:

- `0x0010` has a confirmed persisted GUID identity;
- `0x0010` has a confirmed root alias `0x0115` with the same GUID;
- `tail17=0x06` is an accepted factory category;
- fixture `leading_word` distribution remains audit-only.

Typed DTO work still needs either:

1. the human persisted type name plus `Read` / `DoIO` sequence, or
2. an explicit partial-AC decision accepting GUID/table identity as the
   Phase 20 stopping point.
