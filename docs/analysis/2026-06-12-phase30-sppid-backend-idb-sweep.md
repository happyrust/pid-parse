# Phase 30: SmartPlant P&ID Backend IDB Sweep

> Read-only IDA sweep on 2026-06-12 after opening additional modules from
> `D:\work\plant-code\cad\pid-parse\dlls`.

## Instances

| Binary | Port | Path | High-level role |
|---|---:|---|---|
| `sppid.dll` | 13346 | `D:\work\plant-code\cad\pid-parse\dlls\sppid.dll.i64` | VB6 / COM application layer |
| `sppidautomation.dll` | 13347 | `D:\work\plant-code\cad\pid-parse\dlls\sppidautomation.dll.i64` | VB6 automation startup DLL |
| `sppiddwgprocess.dll` | 13348 | `D:\work\plant-code\cad\pid-parse\dlls\sppiddwgprocess.dll.i64` | DWG recovery / workshare / archive management |
| `ipidobjectmanagerinf.dll` | 13349 | `D:\work\plant-code\cad\pid-parse\dlls\ipidobjectmanagerinf.dll.i64` | Interface / type metadata |
| `sppidautomation.exe` | 13350 | `D:\work\plant-code\cad\pid-parse\dlls\sppidautomation.exe.i64` | VB6 automation launcher |
| `sppidautomationwrap.dll` | 13351 | `D:\work\plant-code\cad\pid-parse\dlls\sppidautomationwrap.dll.i64` | MFC/OLE automation wrapper |
| `llama.dll` | 13352 | `D:\work\plant-code\cad\pid-parse\dlls\llama.dll.i64` | Logical Model Automation object model |

## Checklist Result

Across these modules, the Phase 30 raw-storage terms remain absent:

| Query family | Result |
|---|---|
| `JSitesList`, `OLEM`, `JSite` | 0 hits |
| `PSMspacemap`, `StyleCluster` | 0 hits |
| `JStyleOverride`, `GraphicGroup` | 0 hits |
| `P&IDAttributes`, `Dynamic Attributes`, `RAD_OBJECT_TYPE` | 0 hits |
| `IJPersist`, `IOContext`, `DoIO`, `PersistCluster` | 0 hits |
| `Storage` | 0 hits in the checked modules |

## Module Notes

### `sppid.dll`

Survey:

- 32-bit PE, image size `0x14000`.
- 158 functions, 204 strings.
- Imports are dominated by `MSVBVM60`; this is another VB6 / COM layer.
- Strings include `sppiddll`, `PIDConstants`, `StartupClass`, `regRead`,
  `appObj`, `appObjOuter`, and `RAD2D`.

Interpretation: application/COM glue. It references RAD2D type-library
metadata (`r:\rad2d\bin\rad2d.tlb`) and startup objects, but does not
contain the low-level `.pid` storage/persistence reader strings.

### `sppidautomation.dll`

Survey:

- 32-bit PE, image size `0x7000`.
- 76 functions, 117 strings.
- VB6 automation startup DLL.
- Strings include `SPPIDAutomationDll`, `Startup Dll for
  SPPIDAutomation`, `objOuterApplication`, and toolbar startup names.

Interpretation: thin startup/automation layer, not a PID storage reader.

### `sppiddwgprocess.dll`

Survey:

- 32-bit PE, image size `0xf3000`.
- 481 functions, 408 strings.
- VB6-heavy DWG / archive / workshare management module.

Relevant strings include:

```text
Intergraph SmartPlant P&ID Drawing Recovery, Workshare and Management Implemnetation
ISPArchiveAppData
ISPDrawingISPItems
IStockPileSPItems
ISPProcessModule
ISPPidArchive
ISPPidArchive_LoadSPItems
ISPPidArchive_LoadSPItemsEx
ISPPidArchive_DrawingPath
ISPPidArchive_EmbeddedSPID
ISPPidArchive_LoadRevisionISPItems
ISPPidArchive_LoadVersionISPItems
strDrawingXMLFile
strSharedItemsXMLFile
```

Interpretation: useful for publish/archive/workshare semantics, but it
still does not expose raw CFBF stream names or byte-level persistence APIs.

### `ipidobjectmanagerinf.dll`

Survey:

- 64-bit single-segment metadata-like image, 4 null functions.
- 51 strings.

Interpretation: interface / type metadata only. It has object-manager
names but no executable storage reader surface.

### `sppidautomation.exe`

Survey:

- 32-bit PE, image size `0x2f000`.
- 83 functions, 124 strings.
- VB6 automation launcher.
- Strings include `SPPIDAutomation`, `Registry`, and `Main routine to
  start SPPIDAutomation`.

Interpretation: launcher/front-end, not a `.pid` storage body reader.

### `sppidautomationwrap.dll`

Survey:

- 32-bit MFC/OLE wrapper, image size `0x8000`.
- 231 functions, 68 strings.
- Strings include `RADApplication`, `WrapperApplication`, and
  `SPPIDAutomation.Application`.

Interpretation: automation wrapper around the VB/COM application layer.
No raw `.pid` storage terms.

### `llama.dll`

Survey:

- 32-bit PE, image size `0x72c000`.
- 9,286 functions, 2,571 strings.
- Large Logical Model Automation module.

Relevant strings show a rich object model:

```text
Intergraph SmartPlant P&ID Logical Model Automation
LMADataSource
LMPlantItems
LMPipeRuns
LMPipingComp
LMDrawing
LMDrawingSite
LMRepresentations
LMConnector
LMRelationships
LMAAttribute
```

Interpretation: this is valuable for semantic object model / database-level
understanding, but the first-pass checklist still did not expose raw PID
CFBF stream names or `IOContext` persistence bodies.

## Parser Implication

No Rust parser change is justified by this sweep:

- raw `/JSitesList` confidence remains backed by `OLESITE.dll`;
- `0x0030 = JStyleOverride` confidence remains backed by `style.dll`;
- `PSMspacemap`, `StyleCluster`, `0x0010`, and `GraphicGroup` remain
  unresolved at the byte-layout level;
- `llama.dll` may be useful later for mapping decoded objects to the
  product logical model, but not for claiming raw stream bytes.

## Next Actions

1. Stop broad searches in the current opened SPPID application/automation
   modules unless a concrete symbol/string clue appears.
2. If deeper IDA continues, target modules that are demonstrably closer to
   the persisted drawing/package layer (for example modules that directly
   reference CFBF stream names, jengine `IOContext`, or persist-manager
   APIs).
3. Consider a separate semantic-model task for `llama.dll` only after the
   byte-layout questions are no longer the bottleneck.
