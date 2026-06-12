# Phase 30: `smartplantpid.exe` IDA Launcher Sweep

> Read-only IDA sweep on 2026-06-12 after opening
> `D:\work\plant-code\cad\pid-parse\dlls\smartplantpid.exe` as IDA MCP
> instance `127.0.0.1:13345`.

## Instance

| Binary | Port | Path |
|---|---:|---|
| `smartplantpid.exe` | 13345 | `D:\work\plant-code\cad\pid-parse\dlls\smartplantpid.exe.i64` |

Survey:

- 32-bit PE, base `0x400000`, image size `0xe1000`.
- 120 functions, 161 strings, 3 segments.
- Imports are dominated by `MSVBVM60`; the binary is a VB6 application /
  launcher rather than a C++ persistence module.
- Interesting strings include `SmartPlantPID`, `Smart Plant P&ID`,
  `sppid`, `SPErrorConstants`, `AttributeConstants`, `Registry`,
  `ErrorLogging`, `Form1`, `Module1`, and `modFormIcon`.

## Phase 30 Checklist Searches

| Query | Result |
|---|---|
| `JSitesList` | 0 hits |
| `OLEM` | 0 hits |
| `JSite` | 0 hits |
| `PSMspacemap` | 0 hits |
| `StyleCluster` | 0 hits |
| `JStyleOverride` | 0 hits |
| `GraphicGroup` | 0 hits |
| `P&IDAttributes` | 0 hits |
| `Dynamic Attributes` | 0 hits |
| `RAD_OBJECT_TYPE` | 0 hits |
| `IJPersist` | 0 hits |
| `IOContext` | 0 hits |
| `DoIO` | 0 hits |
| `PersistCluster` | 0 hits |

## Launcher Evidence

The string inventory is consistent with a VB6 front-end / launcher:

```text
0x4016b0 "Main routine to start Smart Plant P&ID"
0x401a0c "SmartPlantPID"
0x401a1a "Smart Plant P&ID"
0x401a2c "sppid"
0x401f80 "SPErrorConstants"
0x401f94 "AttributeConstants"
0x401fb8 "Registry"
0x401fcc "ErrorLogging"
0x401fdc "modFormIcon"
0x401fe8 "sppid"
```

Additional searches found registry API names (`RegOpenKeyEx`,
`RegQueryValueEx`, `RegCloseKey`) and common VB runtime calls, but no
clear product storage names, COM ProgIDs, or writer/reader module names.

## Registry Wrapper Notes

Registry calls are reached through VB / `DllFunctionCall` wrappers:

- `sub_405F14` wraps a dynamic API thunk and is called from
  `sub_41C820` / `sub_41CB50`.
- `sub_41C820` / `sub_41CB50` look like generic `RegQueryValueEx`-style
  string buffer helpers.
- The top-level VB routine `sub_41B000` calls these helpers, but IDA did
  not expose useful registry key literals in ordinary string output.

## Parser Implication

`smartplantpid.exe` does not change parser confidence:

- it does not contain the low-level storage names or persistence APIs
  needed to decode `.pid` stream bodies;
- it does not supersede the positive `style.dll` evidence for
  `JStyleOverride`;
- it does not supersede the positive `OLESITE.dll` evidence for
  `JSitesList`;
- `PSMspacemap`, `StyleCluster`, `0x0010`, and `GraphicGroup` remain gated
  on a lower-level product module or controlled fixture evidence.

## Next Actions

1. Treat this EXE as a launcher/front-end, not the PID binary format reader.
2. Continue IDA only if another SmartPlant P&ID backend DLL / COM module is
   available from the same installation.
3. If no additional product module is available, stop broad IDA searching and
   submit/review the Phase 30 documentation updates.
