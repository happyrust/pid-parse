# IDA Pro MCP Reconnaissance — SPPID DLL Inventory for Phase 14

> Date: 2026-05-13
> Scope: Phase 14 Sheet primitive geometry decode preflight.
> Method: `user-ida-pro-mcp` survey across all SPPID binaries currently
> loaded into IDA Pro instances under `d:/work/plant-code/cad/pid-parse/dlls/`.

## TL;DR

**Update 2026-05-14**: B1 unblocked. User supplied additional
SmartPlant runtime binaries to the WeChat bin folder. Static PE
import-table scan across 18 new candidate DLLs found exactly one
match for the CFB storage API surface:

| DLL | Size | `StgOpenStorageEx` import | Verdict |
|---|---|---|---|
| **`radsrvitem.dll`** | **3.7 MB** | ✓ ordinal `0x1B4` from `ole32.dll` | **Sheet primitive byte parser entry point** |
| `radsrv.dll` / `radnetbridge.dll` / `sppidwrap.dll` / `j2dsrv.dll` | varies | only generic `ole32.dll` (no Stg/Stream) | COM-only, not Sheet I/O |
| 13 others (ubspm2d1, pidobjmgrai, RadNetAutomation, etc.) | varies | none | Higher-level than Sheet I/O |

`radsrvitem.dll` is the binary the original IDA reconnaissance was
looking for — Intergraph's RAD2D family ships its Compound File I/O
in a `radsrv*` server-side binary, not in a `rad2d.dll` per se. The
original Phase 14 plan called for `rad2d.dll` / `pidobjectmanager.dll`
but `radsrvitem.dll` is the actual implementation.

Next step is to open `radsrvitem.dll` in IDA Pro at a new port and
resume slice A from step 2 of
`goals/phase14-sppid-sheet-geometry/slice-a-runbook.md`.

---

**Original 2026-05-13 finding (kept for context — still valid for the
8 prior binaries)**:

**None of the 8 SPPID DLLs / EXEs currently in IDA Pro contain the Sheet
stream byte parser.** They are all higher-level layers (VB6 COM dispatch,
MFC OLE/COM wrappers, drawing-level workshare management, COM type
libraries, automation-property documentation).

**Import-level conclusive evidence (added 2026-05-13 second pass)**:
All 8 SPPID binaries return **zero** matches when queried for
`*Stream*` / `*Storage*` / `ole32::*` imports. The only `ole32` hit
across all eight is `CoDisconnectObject` in `sppidautomationwrap.dll`
(COM cleanup, not stream I/O). This proves at the import-table level
that these binaries do not call CFB stream APIs directly; they must
delegate to a separate binary the user has not yet supplied.

The actual `.pid`-file Sheet primitive reader lives in a binary we have
**not yet acquired** — most likely the RAD2D 2D-CAD-engine core
implementation that the SmartPlant P&ID install loads at runtime. Until
that binary lands, IDA-driven evidence for `SheetRecordKind::PrimitiveLine`
/ `PrimitivePolyline` / `PrimitiveCircle` / `PrimitiveArc` cannot be
produced.

## What is in IDA right now

| Port | Binary | Size | Funcs | Strings | Role (confirmed) |
|---|---|---|---|---|---|
| 13337 | `core.dll` (AVEVA E3D) | — | — | — | Unrelated to SPPID; from another reverse-engineering project |
| 13338 | `sppidautomationwrap.dll` | 19 KB | — | — | MFC100/OLE32 wrapper, no geometry/sheet APIs |
| 13339 | `ipidobjectmanagerinf.dll` | 49 KB | 4 | 51 | Type library shim (`.tlb` wrapped as DLL). No segments, no entry points. **Interface definitions only.** |
| 13340 | `sppidautomation.dll` | 28 KB | — | — | COM dispatcher |
| 13341 | `sppid.dll` | 80 KB | 158 | 204 | VB6 (MSVBVM60) registry/event/error dispatcher. Strings: `PIDConstants`, `SPCommandConstants`, `Registry`, `StartupClass`, `appObjOuter`. **No Sheet-byte logic.** |
| 13342 | `smartplantpid.exe` | 920 KB | 120 | 161 | Application launcher. **No geometry strings.** |
| 13343 | `sppiddwgprocess.dll` | 991 KB | 481 | 408 | "Drawing Recovery, Workshare and Management Implementation". Touches `ISPArchive*`, `ISPDrawing*`, `ISPPidArchive*` COM type libraries; **does not parse Sheet bytes directly.** |
| 13344 | `sppidautomation.exe` | 192 KB | — | — | Automation main |
| 13345 | `llama.dll` | 7.4 MB | 9286 | 5242 | The **LM object/automation engine** — `LMInstrument`, `LMSymbol`, `LMSymbols`. Strings are SPPID's automation-API property-documentation surface. **Not the geometry binary engine.** None of the searched primitive strings (`Primitive*`, `Polyline`, `Polygon`, `Circle`, `Arc`, `Sheet[0-9]`, `RAD2D`, `PSMcluster`, `DocVersion`) hit. |

`llama.dll` is the surprise — its size and the codename hinted at the
RAD2D 2D-CAD core, but the actual string evidence is squarely on
SPPID's "Logical Model" automation API. RAD2D itself is presumably a
*separate* DLL.

## Why none of these binaries answer Phase 14

`Sheet*` storage in a `.pid` CFB file contains binary records that
SmartPlant's drawing renderer (RAD2D / Intergraph 2D engine) reads.
Phase 14 needs the **byte layout** of those records:

- `PrimitiveLine` start/end coordinates
- `PrimitivePolyline` vertex list
- `PrimitiveCircle` center + radius
- `PrimitiveArc` center + radius + start/end angles
- `SymbolPlacement` transform + symbol ref
- `TextPlacementStyle` font + position
- `CoordinatePageMetadata` page transform / units

To produce that evidence from IDA, we need the binary that performs
`IStorage::OpenStream("Sheet6")` followed by structured `IStream::Read`
calls into typed RAD2D primitive decoders. **None of the 8 binaries we
own do that.** Their imports are limited to MFC100, MSVCR100, ole32,
oleaut32, KERNEL32 (only generic OLE/IDispatch plumbing), MSVBVM60 for
sppid.dll's VB6 layer.

## Cross-check against existing project docs

| Doc | Conclusion |
|---|---|
| `docs/analysis/2026-05-09-external-sppid-format-evidence.md` | Public Intergraph / Bentley / Hexagon documentation does **not** carry record-level byte evidence for these primitives. |
| `docs/analysis/2026-05-09-primitive-line-record-evidence.md` | Task 14-03 is blocked by **evidence**, not by implementation mechanics. Current line output is `EndpointPair + Inferred`, not decoded. |
| `docs/plans/2026-05-09-phase-14-sppid-full-geometry-plan-cn.md` | Mandates Sheet-byte provenance for any `Decoded` upgrade. Synthetic / inferred shapes must stay `Inferred` / `ProbeOnly`. |

Adding IDA evidence from the wrong binaries would **not satisfy** the
phase 14 evidence contract (`PidGraphicProvenance.byte_range` + bounded
Sheet record range + repeatable shape across fixtures).

## What binaries we still need

The most-likely candidates that would carry the Sheet primitive reader,
ordered by expected payoff:

1. **`rad2d.dll`** (the COM implementation of RAD2D, distinct from
   `Interop.RAD2D.dll` which is only the .NET interop shim we already
   have) — Intergraph's 2D CAD engine.
2. **`pidobjectmanager.dll`** (no `inf` suffix) — the implementation
   side of `IPidObjectManager`, paired with the `ipidobjectmanagerinf.dll`
   interface shim we already own.
3. **Intergraph 2D drawing dependencies** — for example
   `sigma2d.dll` / `igrgcdt.dll` / `igrlnk.dll` / `igrloc.dll` /
   `pid2dcontext.dll`. These are part of Intergraph's pre-RAD2D legacy
   2D foundation but still ship with SmartPlant P&ID 12.x.
4. **`pidplot.dll`** — handles plot/print path which also walks Sheet
   primitives.

These binaries are typically installed under
`C:\Program Files (x86)\SmartPlant\P&ID\bin\` or
`...\Intergraph\Pid\bin\` on a real SmartPlant P&ID workstation. The
`bin` folder we already scanned
(`E:\weixin\xwechat_files\happydpc_b2ec\msg\file\2026-05\bin`) did not
contain `rad2d.dll`, but did reference RAD2D inside type libraries —
suggesting it's loaded from a sibling directory or registered via
`HKEY_CLASSES_ROOT`.

## Recommended Plan (revised)

The original "按推荐方案继续" trajectory pointed at publish-pipeline
gaps. That work is correct in isolation (Step 1 + Step 3 closed clean,
all gates green) but **it is not on the Phase 14 critical path** and
should be parked as an independent committable change while the real
bottleneck is unblocked.

### Plan A — Acquire RAD2D core (preferred)

1. Locate the real `rad2d.dll` / `pidobjectmanager.dll` on the
   workstation that produced the existing SPPID install. Copy them
   into `dlls/` alongside the other SPPID binaries.
2. Open them in IDA Pro and attach to `user-ida-pro-mcp` (new ports).
3. Run the same string + import survey as this document, this time
   expecting hits on `Sheet`, `Primitive*`, `IStorage::OpenStream`,
   `f64` coordinate-format strings, and CFB stream-name constants.
4. Locate `IStorage::OpenStream("Sheet1"…"SheetN")` callers and trace
   into the structured decoder.
5. Author a new evidence document
   `docs/analysis/2026-05-XX-rad2d-sheet-record-bytes.md` with bounded
   byte ranges for each primitive type, then unblock Task 14-03.

### Plan B — Controlled diff while DLLs are missing

Per `docs/plans/2026-05-09-controlled-diff-evidence-report-plan.md`,
the project already has a "before/after `.pid` edit" investigation
pathway that does NOT require IDA. While Plan A is pending acquisition
of `rad2d.dll`, push Plan B forward:

1. Implement `src/inspect/controlled_diff.rs` per the existing plan.
2. Generate the smallest possible `.pid` edits in SmartPlant P&ID
   (place one line, save; place one circle, save; ...).
3. Diff the before/after `.pid` at CFB stream + byte range level.
4. The bounded byte ranges from controlled diffs become candidate
   `SheetRecordKind` byte-range proofs **without** needing the
   reverse-engineered decoder yet.

### Plan C — Park publish-pipeline drift, commit clean checkpoint

This conversation produced:

- `T_ModelItem` 4-column audit-field surfacing (Step 1)
- `T_Pipeline` chained into the PipeRun subtable list, automatically
  unlocking the existing writer's `IFluidSystem` field routing (Step 3)
- Oracle `exp` dump format detection in `pid_backup_extract`
- `examples/oracle_exp_schema.rs` — Oracle DDL extraction diagnostic
- 41 SPPID DLLs / XMLs copied into `dlls/` for future IDA work
- 9 new unit tests (5 in `sqlite_load`, 4 in `pid_backup_extract`)

All five pre-commit gates green
(`cargo build --locked --workspace --all-targets`,
`cargo test --locked --workspace --all-targets`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, `missing_docs` ratchet).

These changes are **independent of Phase 14** and should be landed as
a self-contained commit while we figure out Plan A / Plan B.

## What is NOT a valid next step

- **Mining the 8 existing IDA instances for Sheet record byte layout.**
  The strings and imports prove the relevant code is not in them.
- **Hand-writing a Sheet decoder from existing inferred outputs.**
  Violates the Phase 14 anti-goal ("不用 endpoint topology 反推 CAD
  primitive").
- **Pulling `EXPORT:V12.01.00` Oracle dump binary records.** Per the
  separate conversation finding, the DWG fixture uses Oracle 12c
  `exp`, not SQL Server MTF — Sheet bytes are unrelated to Oracle
  schema and cannot be extracted there.
- **Heuristic UTF-16LE string scans across Oracle exp row regions.**
  We tried a 2-byte LE-length + UTF-16LE assumption against the
  `T_MODELITEM` region of the DWG fixture; only a single noise hit
  (`"P"`) surfaced. Oracle's `exp` row format is not that simple —
  it uses Oracle-internal type tags + variable-length encodings.
  A proper Oracle exp row parser would be a multi-day project on
  its own and is out of scope for this milestone.

## Open questions for the user

1. Can you copy `rad2d.dll` and any sibling Intergraph 2D DLLs from
   the SmartPlant P&ID install directory into the same WeChat folder
   we sourced the first 41 DLLs from?
2. Do you have access to a SmartPlant P&ID workstation where we can
   place a known primitive (one line, one circle, ...) in a test
   drawing, save it, and capture the before/after `.pid` pair?
3. Should we land Plan C (publish-pipeline checkpoint) now, or wait
   until Plan A / Plan B produce evidence?
