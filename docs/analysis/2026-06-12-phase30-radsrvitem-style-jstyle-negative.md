# Phase 30: `radsrvitem.dll` Style / JStyle Negative Evidence

> Read-only IDA refresh on 2026-06-12 using the reachable
> `radsrvitem.dll` instance (`127.0.0.1:13338`). This note records the
> final low-cost searches after the JSite, `0x0089`, and PSMspacemap
> refreshes.

## Instance Check

Available IDA instances remained unchanged:

| Binary | Port | Status |
|---|---:|---|
| `core.dll` | 13337 | reachable |
| `radsrvitem.dll` | 13338 | reachable / active |

No `style.dll`, `J2DSrv.dll`, `sppid.dll`, `XCeedRAD.dll`, or
`smartplantpid.exe` IDB was available.

## Searches

| Search | Result |
|---|---|
| `StyleCluster` | 0 hits |
| `JStyleOverride` | 0 hits |
| `JStyle` | hits in `JStyleBase`, `IJPersistImp`, `IJManageStyle2Imp`, `IJStyleCopyImp`, `IJStyleUserImp` RTTI/vtables |

The hits show that `radsrvitem.dll` carries generic JStyle base/interface
infrastructure, but not the `JStyleOverride` class name or
`StyleCluster` storage name.

## `JStyleBase` Findings

### Constructor

`sub_5655D200` constructs `JStyleBase` and installs these vtables:

```text
JStyleBase
JStyleBase::IJPersistImp
JStyleBase::IJManageStyle2Imp
JStyleBase::IJStyleCopyImp
JStyleBase::IJStyleUserImp
```

It initializes a small base object state, but does not expose record
layout or persisted style fields.

### `IJPersistImp` thunk

`sub_5655DB60` is the first visible `JStyleBase::IJPersistImp` vtable
entry:

```text
return (*(this[-12]->vtable + 52))(
  this - 12,
  a2,
  a3,
  "JStyleBase::IJPersistImp"
)
```

`sub_5655DBE0` similarly forwards:

```text
return (*(this[-12]->vtable + 48))(
  this - 12,
  "JStyleBase::IJPersistImp"
)
```

These are interface thunks / name-forwarding helpers, not direct
Load/Save implementations.

## Practical Conclusion

This current IDB cannot answer the remaining StyleCluster/JStyle
questions:

- no `StyleCluster` storage literal;
- no `JStyleOverride` class literal;
- no direct `0x0030` persistence body found from the low-cost searches;
- `JStyleBase::IJPersistImp` hits are generic interface thunks.

The Phase 16/17 conclusion remains unchanged: `0x0030` is
`JStyleOverride`, not arc geometry, but deeper persistence fields and
StyleCluster prefix layout are still gated on `style.dll` / `J2DSrv.dll`
or another writer/reader-side IDB.
