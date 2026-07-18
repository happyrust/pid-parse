# Phase 31: OLECRT.dll Storage Entrypoint Sweep

> Date: 2026-06-13  
> IDA instance: `OLECRT.dll` at `127.0.0.1:13353`  
> Scope: follow-up on a module that was previously tooling-gated during
> Phase 30; determine whether it unlocks raw `.pid` storage layout questions.

## Summary

`OLECRT.dll` is now reachable through IDA MCP and is closer to the
SmartSketch/RAD OLE storage layer than the VB6 application modules. It imports
OLE storage APIs and jengine persistence helpers, but it still does not expose
the remaining SmartPlant P&ID business stream readers (`JSitesList`,
`PSMspacemap`, `StyleCluster`, `GraphicGroup`, `0x0010`, etc.).

The useful new evidence is:

- it opens CFBF/OLE storages through `StgOpenStorageEx`;
- it directly probes the wide stream name `DocVersion2`;
- it opens `SymbolInformationCluster` inside an external OLE storage during
  `CreateImagineerOleObject`;
- it calls jengine helpers such as `GetPersistManager`,
  `GetPersistCluster`, `UnBindSheetWrappers2`, and
  `DecoupleAutomationWrappersInCache`.

Parser implication: this supports treating `OLECRT.dll` as an embedded
OLE/symbol-object bridge and version probe layer, not as the missing
SmartPlant `.pid` byte-layout reader.

## Binary Survey

| Item | Result |
|---|---|
| Binary | `E:\reverse\pid\OLECRT.dll.i64` |
| Module | `OLECRT.dll` |
| Arch | 32-bit |
| Image size | `0x19000` |
| Functions | 337 total / 196 named / 112 unnamed |
| Strings | 180 |

Notable imports:

```text
ole32!StgOpenStorageEx
ole32!StgIsStorageFile
ole32!OleLoadFromStream
ole32!CreateFileMoniker
ole32!CLSIDFromString
jengine!GetPersistManager
jengine!GetPersistCluster
jengine!UnBindSheetWrappers2
jengine!DecoupleAutomationWrappersInCache
OLESITE!GetSiteManager
style!HCreateStyleInLib
style!HGetActiveLinearStyle
symbol!JGetSymbolNaturalStyle
symbol!GetSymbolInformation
```

## Negative Raw-Storage Checklist

String search returned zero hits for:

```text
JSitesList
JSite
OLEM
PSMspacemap
StyleCluster
GraphicGroup
JStyleOverride
IOContext
DoIO
PersistCluster
P&IDAttributes
TaggedTxtData
AppObject
PSMroots
JTaggedTxtStgList
```

`IJPersist` appears once, and generic `Storage` / `Stream` / `OpenStorage`
strings appear, but they are import/type strings rather than direct
SmartPlant stream names.

## Confirmed Wide Storage Names

UTF-16LE byte search found two relevant wide names:

| Name | Address | Xref |
|---|---:|---|
| `DocVersion2` | `0x10013be4` | `sub_1000A800` |
| `SymbolInformationCluster` | `0x10012790` | `sub_100017C0` |

No UTF-16LE hits were found for `JSitesList`, `PSMspacemap`,
`StyleCluster`, `P&IDAttributes`, `TaggedTxtData`, `AppObject`,
`PSMroots`, or `JTaggedTxtStgList`.

## Function Evidence

### `sub_1000A800`: `DocVersion2` probe

This helper opens a storage file and reads the first two 16-bit values from
the `DocVersion2` stream:

```text
0x1000a87b: StgOpenStorageEx(pwcsName, 0x10000, 5, 0, &STGOPTIONS{sector=4096}, 0, &IID_IUnknown, &storage)
0x1000a89f: storage->OpenStream(L"DocVersion2", 0, 16, 0, &stream)
0x1000a8bb: stream->Read(a2, 2, &bytes_read)
0x1000a8d3: stream->Read(a3, 2, &bytes_read)
```

Caller `sub_1000A920` uses this as a file/version compatibility check. It
first confirms the file exists, then calls `sub_1000A800`, and treats a first
version word `>= 0x21` as a warning/error condition.

This is evidence for a small version-probe path only. It does not parse the
rest of the `.pid` CFBF tree.

### `sub_100017C0`: embedded symbol information storage

`CreateImagineerOleObject` calls `sub_100017C0`. Inside that helper:

```text
0x100021bf: StgOpenStorageEx(pwcsName, 0x10000, 5, 0, &STGOPTIONS{sector=4096}, 0, &IID_IUnknown, &storage)
0x100021df: storage->OpenStream/OpenStorage(L"SymbolInformationCluster", ...)
```

The same function also calls style/symbol helpers, including
`JGetSymbolNaturalStyle`, `HGetActiveLinearStyle`, and
`GetSymbolInformation`. This points to external embedded OLE symbol handling,
not to the main SmartPlant `.pid` stream graph.

### `sub_10008180`: unbinding sheet wrappers

This function obtains the jengine persist manager and its storage, decouples
automation wrappers, and calls `UnBindSheetWrappers2`:

```text
GetPersistManager(a1, &persist_manager)
persist_manager->vtable[0x5c](&storage)
persist_manager->vtable[0x4c](a1, &id, 0)
DecoupleAutomationWrappersInCache(storage, &id, persist_manager, 0)
UnBindSheetWrappers2(storage, id, persist_manager)
BatchDelete(a1)
BatchFlush()
```

This is useful for understanding OLE embedded sheet wrapper lifecycle, but it
does not expose stream names or byte layouts for `/Sheet*`, `/JSitesList`, or
PSM streams.

### `sub_10008020`: persist manager / cluster bridge

`sub_10008020` gets `IJPersistManager` and optionally `PersistCluster`, then
invokes a vtable method at offset `0x44`. This confirms the module crosses
into jengine persistence infrastructure, but there is no nearby business
stream string or concrete `IOContext::DoIO` body to tie it to a PID record
layout.

### `sub_1000D290`: OLE file-type / CLSID resolution

This helper calls `StgIsStorageFile`, checks MicroStation V8 status, and then
queries an engine interface for:

```text
FileExtensions\<ext>\CLSID
```

It converts the returned string with `CLSIDFromString`. This is type routing /
file extension recognition, not `.pid` stream parsing.

## Interpretation

`OLECRT.dll` should be classified as:

- **Positive evidence** for generic OLE storage entrypoints and embedded
  symbol-object lifecycle;
- **Positive evidence** that `DocVersion2` is read as two 16-bit values in at
  least one native compatibility probe;
- **Negative evidence** for the remaining byte-layout backlog, because the
  module lacks business stream names and direct record reader/writer bodies.

## Parser Impact

No Rust parser promotion is justified from this sweep.

Safe implications:

- keep existing `DocVersion2` parser semantics as a small version stream;
- optionally cite `OLECRT.dll::sub_1000A800` as native evidence that
  `DocVersion2` contains at least two 16-bit values;
- keep `/Sheet*`, `/JSitesList`, `PSMspacemap`, StyleCluster, `0x0010`, and
  GraphicGroup gate status unchanged.

Do not:

- infer `/Sheet*` byte layout from `UnBindSheetWrappers2` lifecycle calls;
- treat `SymbolInformationCluster` as a top-level `.pid` stream without a
  fixture-backed path;
- promote PSM or JSitesList stale-tail fields from this module.

## Next Actions

1. Update the Phase 30 gated-action note to move `OLECRT.dll` from
   tooling-gated to checked.
2. Add `DocVersion2` native evidence to findings/progress.
3. Continue byte-layout work only with a module/function that exposes direct
   SmartPlant stream names or `IOContext::DoIO` record bodies.
