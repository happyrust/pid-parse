# Phase 30: Secondary IDB Sweep After `style.dll`

> Read-only IDA sweep on 2026-06-12 after locating the SmartSketch /
> RAD runtime modules under `E:\reverse\pid`.

## Candidate Directory

`E:\reverse\pid` contains SmartSketch / RAD-era modules rather than a
SmartPlant P&ID main install:

- present: `style.dll`, `J2DSrv.dll`, `jengine.dll`, `JUTIL.dll`,
  `Linkole.dll`, `OLECRT.dll`, `OLESITE.dll`, `SmartSketch.exe`,
  `SmartSketchWrap.dll`, `XceedRAD.dll`;
- not present by keyword scan: `sppid.dll`, `smartplantpid.exe`.

## `J2DSrv.dll`

Opened as IDA MCP instance `127.0.0.1:13340`.

Survey:

- 32-bit PE, image size `0x1f000`.
- 692 functions, 339 strings.
- Imports geometry/render/style helpers such as `HSetStdDashStyleData`,
  `HSetStdDashTypeData`, `HSetFillDataFromUF`, `HSetCustomDashStyleData`,
  `HSetContinuousLineStyleData`, `GetJSCustodian`, and `RealRender2d`.

Negative searches:

| Query | Result |
|---|---|
| `JSitesList` | 0 hits |
| `OLEM` | 0 hits |
| `JSite` | 0 hits |
| `PSMspacemap` | 0 hits |
| `StyleCluster` | 0 hits |
| `JStyleOverride` | 0 hits |
| `GraphicGroup` | 0 hits |
| `IJPersist` | 0 hits |
| `PSM` | 0 hits |

Interpretation: useful as a 2D geometry/render/style helper, but not a
direct storage-name or PID container reader/writer target for the current
Phase 30 questions.

## `XceedRAD.dll`

Opened as IDA MCP instance `127.0.0.1:13341`.

Survey:

- 32-bit module, image size `0x24000`.
- 329 named functions, no ordinary string table hits from IDA survey.
- Export/function names indicate Xceed compression helpers:
  `XcCompressFile`, `XcCompressFile2`, `XcDecompressFile`,
  `XcDecompressFile2`, `XcInitialize`.

Negative searches:

| Query | Result |
|---|---|
| `JSitesList` | 0 hits |
| `OLEM` | 0 hits |
| `JSite` | 0 hits |
| `PSMspacemap` | 0 hits |
| `StyleCluster` | 0 hits |
| `JStyleOverride` | 0 hits |
| `GraphicGroup` | 0 hits |
| `IJPersist` | 0 hits |
| `PSM` | 0 hits |
| `RAD_OBJECT_TYPE` | 0 hits |

Interpretation: not useful for PID structure decoding; likely compression
support rather than SmartPlant/RAD record semantics.

## `jengine.dll`

Opened as IDA MCP instance `127.0.0.1:13342`.

Survey:

- 32-bit PE, image size `0xa7000`.
- 2,919 functions, 1,157 strings.
- Exports include general document / automation / persistence machinery:
  `JGetRootOwner`, `JCreateDocument`, `GetPersistManager`, and related
  automation helpers.
- Imports and strings include low-level persistence concepts:
  `IJPersist`, `IOContext`, `DoIO`, `PersistCluster`,
  `GetPersistCluster`, and PSM/segment diagnostic text such as
  `Segment::GetNextAvailIndex()[pMgr = 0x%p]: ...`.

Negative business-name searches:

| Query | Result |
|---|---|
| `JSitesList` | 0 hits |
| `OLEM` | 0 hits |
| `JSite` | 0 hits |
| `PSMspacemap` | 0 hits |
| `StyleCluster` | 0 hits |
| `JStyleOverride` | 0 hits |
| `GraphicGroup` | 0 hits |

Positive engine-level searches:

| Query | Result |
|---|---|
| `IJPersist` | 38 hits |
| `PSM` | 7 hits |
| `DoIO` | 1 string hit |
| `IOContext` | 10 hits |
| `PersistCluster` | 5 hits |
| `GetPersistCluster` | 1 hit |

Interpretation: `jengine.dll` is the common persistence/automation engine
used by `style.dll` and other modules. It can help explain generic
`IOContext` / persist-manager mechanics, but this sweep did not expose
PID-specific storage names or class-specific payload readers.

## Parser Implication

This `J2DSrv.dll` / `XceedRAD.dll` / `jengine.dll` secondary sweep does
not require Rust parser changes:

- keep the `style.dll`-confirmed `0x0030 = JStyleOverride` decoder;
- do not upgrade `JSitesList`, `StyleCluster`, `PSMspacemap`, `0x0010`,
  or `GraphicGroup` semantics from these three modules alone;
- continue to treat business storage names as gated on a writer/reader
  module that actually references those names.

## Local OLE Follow-Up

`Linkole.dll` was opened as IDA MCP instance `127.0.0.1:13343` after the
secondary sweep. Its module identity is OLE link / moniker support; it did
not become a higher-value PID storage target than `OLESITE.dll`.

`OLESITE.dll` was opened as IDA MCP instance `127.0.0.1:13344` and became
the productive OLE-side target. See
`2026-06-12-phase30-olesite-jsiteslist-ida.md`.

`OLECRT.dll` was launched in IDA (`ida.exe E:\reverse\pid\OLECRT.dll`,
process `128572`) but did not register as an IDA MCP instance after the
same wait/poll loop used for the other modules. No IDA tool survey was
therefore possible in this pass.

## Recommended Next IDB

The local SmartSketch / RAD runtime sweep has now produced its useful
positive evidence (`style.dll` for `JStyleOverride`, `OLESITE.dll` for
`JSitesList`) and exhausted the low-cost local DLL targets. Do not continue
broad local-module searches unless a concrete string/function clue appears.

If a true SmartPlant P&ID install becomes available, prefer `sppid.dll`
or another lower-level backend DLL / COM module over additional SmartSketch
support modules or the VB6 `smartplantpid.exe` launcher.
