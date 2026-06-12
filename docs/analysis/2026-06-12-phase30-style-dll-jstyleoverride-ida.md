# Phase 30: `style.dll` JStyleOverride Persistence Refresh

> Read-only IDA refresh on 2026-06-12 after opening
> `E:\reverse\pid\style.dll` as a new IDA MCP instance.

## Instance

| Binary | Port | Path |
|---|---:|---|
| `style.dll` | 13339 | `E:\reverse\pid\style.dll.i64` |

Survey:

- 32-bit PE, base `0x10000000`, image size `0x84000`.
- 2,590 functions, 666 strings.
- Exports include `HCreateOverrideStyle`,
  `HGetOverrideStyle`, `HCreateStyle`, `HCreateStyleFromUF`, and many
  style-library helpers.

## Search Results

| Query | Result |
|---|---|
| `JStyleOverride` | present, including `JStyleOverride::IJStyleOverrideImp` and `JStyleOverride::IJStylePrerenderImp` |
| `JStyleBase` / `IJPersist` | present |
| `StyleCluster` | 0 hits |
| `JSitesList` / `OLEM` | 0 hits |
| `PSMspacemap` | 0 hits |
| `GraphicGroup` | 0 hits |

This makes `style.dll` useful for JStyle persistence, but not for
`JSitesList`, `PSMspacemap`, or `StyleCluster` storage-name evidence.

## JStyleOverride Identity

`stru_10066B64` decodes as:

```text
47fcc338-2d0f-11d0-a1ff-080036a1cf02
```

This is the same CLSID family identity previously used for the
`0x0030 = JStyleOverride` conclusion.

Nearby version GUIDs observed during style persistence analysis:

```text
stru_10066AF4 = 47fcc333-2d0f-11d0-a1ff-080036a1cf02
stru_10066B14 = 47fcc335-2d0f-11d0-a1ff-080036a1cf02
stru_10066B64 = 47fcc338-2d0f-11d0-a1ff-080036a1cf02
```

## JStyleOverride Vtable

The main `JStyleOverride` vtable starts around `0x1006A560`.
Notable entries:

| Vtable slot address | Function | Finding |
|---:|---|---|
| `0x1006A564` | `sub_1000EF40` | returns `"JStyleOverride"` |
| `0x1006A570` | `sub_1000F210` | versioned persistence path for CLSID `47fcc338...` |
| `0x1006A574` | `sub_10010640` | clone/copy helper |
| `0x1006A5FC` | `sub_1000F030` | current persistence body with 13 `DoIO` calls |
| `0x1006A604` | `sub_1000DFE0` | `IJStyleOverrideImp` thunk |
| `0x1006A698` | `sub_1000E6E0` | `IJStylePrerenderImp` thunk |

The `IJStyleOverrideImp` / `IJStylePrerenderImp` functions are interface
thunks that forward to the owning base object and carry the interface
name. They are not the persistence body.

## Current Persistence Body

`sub_1000F030` is the key current persistence body for `JStyleOverride`.
After the common style helper `sub_10055F30`, it calls
`IOContext::DoIO` 13 times:

| Object byte offset | Size |
|---:|---:|
| `+88` | 4 |
| `+96` | 4 |
| `+100` | 4 |
| `+152` | 4 |
| `+104` | 8 |
| `+112` | 8 |
| `+120` | 8 |
| `+136` | 8 |
| `+128` | 4 |
| `+188` | 4 |
| `+192` | 4 |
| `+144` | 2 |
| `+146` | 2 |

Total persisted payload size: `64` bytes.

This directly matches the existing Rust `JStyleOverride` decoder shape
from Phase 16: 13 fields / 64-byte payload. The current parser should
stay on the `JStyleOverride` interpretation and must not regress to the
retired `GArc2d` interpretation for `0x0030`.

## Versioned Persistence Body

`sub_1000F210` is the versioned path using `stru_10066B64`. For version
2 it reads a similar JStyleOverride field group via `DoIO`; for version
3 it delegates to the vtable slot at `+208`, which resolves back to the
current persistence path.

## Clone Evidence

`sub_10010640` allocates a new 204-byte persisted object, then copies a
wide byte region:

```text
qmemcpy(v5 + 22, this + 22, 0x58)
```

It then clears transient pointer-like fields:

```text
v5[37] = 0
v5[49] = 0
```

This supports the existing caution that the persisted style object has a
larger runtime byte region than the 64 bytes emitted by the current
`DoIO` sequence; not every copied runtime slot is necessarily a stable
disk field.

## Parser Implication

Confirmed:

- `0x0030` remains `JStyleOverride`.
- The 64-byte current disk payload and 13-call `DoIO` sequence are now
  directly backed by `style.dll`, not only by fixture/probe evidence.
- `JStyleOverride` interface thunks are not persistence bodies.

Still gated:

- semantic names for the individual 13 fields;
- `StyleCluster` prefix layout;
- `/JSitesList` writer / stale-tail semantics;
- raw `/PSMspacemap` page layout;
- `0x0010` discriminator and `GraphicGroup` child/reference payload.

## Next Actions

1. Keep Rust parser behavior unchanged for `0x0030`; this pass confirms
   the existing decoder rather than requiring a code change.
2. Later local sweeps opened `J2DSrv.dll`, `XceedRAD.dll`, `jengine.dll`,
   `Linkole.dll`, `OLESITE.dll`, and attempted `OLECRT.dll`; only
   `OLESITE.dll` added PID-format evidence. See
   `2026-06-12-phase30-secondary-idb-sweep.md` and
   `2026-06-12-phase30-olesite-jsiteslist-ida.md`.
3. Do not use `style.dll` alone to name `StyleCluster` prefix fields or
   `0x0010` sub-kinds; this IDB did not expose those literals or readers.
   Further IDA should prefer a lower-level SmartPlant P&ID backend module
   such as `sppid.dll` or another product DLL / COM module that is not just
   the VB6 launcher.
