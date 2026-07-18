# Phase 33 PSM `0x0010` Discriminator — Mode A static `.asm` evidence

> Date: 2026-06-30
> Scope: read-only static reverse-engineering pass over
> `dlls/radsrvitem.dll.asm` (full IDA disassembly export, 40 MB) plus
> raw-PE byte verification, executing **Mode A** of
> `docs/analysis/2026-06-30-phase33-0010-discriminator-next-ida-evidence-plan.md`.
> Status: evidence note only. No parser / DTO / schema / writer / bundle /
> byte-audit / ratchet / roadmap-confidence change. `ROADMAP-0010` stays
> `TypedAudit`.

## Why Mode A (live decompiler unavailable)

`list_instances` on 2026-06-30 shows only `sppid.dll` (`127.0.0.1:13337`,
active) and `core.dll` (`127.0.0.1:13338`) reachable. **`radsrvitem.dll` is
not open as an IDA instance**, so Mode B (Hex-Rays on `radsrvitem.dll`) is
unavailable, and opening a GUI IDB is out of scope for this run. The full
`.asm` export and the raw PE (`dlls/radsrvitem.dll`) are present, so the
plan's Mode A is executed instead.

## A1 — Type-table identity re-verified from raw PE (independent)

Reading the 20-byte `PersistComTypeEntry` records straight from
`dlls/radsrvitem.dll` (no IDA needed):

| code | file off | entry bytes (hex) | GUID | tail16 | tail17 | parent |
|---|---|---|---|---|---|---|
| `0x0010` | `0x23A5A8` | `C028191D 00000000 C0000000 00000046 40 06 1501` | `1D1928C0-0000-0000-C000-000000000046` | `0x40` | `0x06` | `0x0115` |
| `0x0115` | `0x23BA0C` | `C028191D 00000000 C0000000 00000046 C0 06 0000` | `1D1928C0-0000-0000-C000-000000000046` | `0xC0` | `0x06` | `0x0000` |

Confirms Phase 20 (`docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`):
`0x0010` and root alias `0x0115` share GUID `1D1928C0...`. The PSM type code
itself is **not** stored in the entry (the table is GUID-sorted for binary
search), so PSM type codes carry no ordinal semantics. Byte-accurate field
split is `tail16 (1) + tail17 (1) + parent (2 LE)`.

## A2 — Deserialize dispatch architecture (`PSMSerializeIn sub_564915E0`)

Function span: asm `L147070–L147814`. Post-envelope control flow:

1. **Type extract (`L147350–147353`):**
   `mov eax,[ecx+8] ; shr eax,6 ; and eax,3FFFh`, where `ecx` = the in-memory
   record (`var_814`). i.e. `type = (rec.packed_id >> 6) & 0x3FFF`. Compared
   against the persisted type word `var_83C`.
2. **Type identity validation (`L147357–147408`):** parent-chain walk of the
   `PersistTypeTable` (`dword_567DDC90`) via `sub_564689C0`, matching
   `[entry+0x10]` (code) / `[entry+0x12]` (parent index). Pure identity check;
   reads no persisted body bytes.
3. **Instantiate object for type (`L147469–147490`):** `sub_56490B00` →
   `var_844` (the type's object); `call [ecx]` (vtable+0) init.
4. **Body-read delegation (`L147554–147560`) — the key site:**
   `mov eax,[var_824] ; push [edi+8] ; push eax ; mov ecx,[eax] ;
   call dword ptr [ecx+18h]` — calls **vtable+0x18** on the per-type object,
   passing the IOContext/stream pointer at `[edi+8]`. **Each type's field
   layout is consumed here, inside its own `vtable+0x18`.**
5. **Deferred / lazy-load (`L147561–147567`):** if `vtable+0x18` returns
   `0x80040233`, `PSMSerializeIn` sets flag bit **`0x400`** on the object
   (`or [eax+0Eh], 400h`). This is the same bit the OID→object reader
   `sub_56468B30` tests at `descriptor+0x0E` (`test eax,400h`, `L70094`) to
   route to the **SerialCluster lazy-load** path.
6. **Byte-accounting (`L147594–147603`):** `sub_56491050` returns the stream
   cursor; `cursor - var_840` (body start) is compared to `var_82C`
   (= `bytes_to_follow`); a mismatch emits
   `"Warning: PSMSerializeIn()[pMgr = 0x%p] ..."`. Confirms `bytes_to_follow`
   is the authoritative body length, consistent with the proven envelope
   `type(2) + bytes_to_follow(4) + oid(4) + aux(8) + inner(var)`.

## A3 — OID→object Read entry (`sub_56468B30`, `L70027–70291`)

`SegmentTable::GetIGDS9Pointer`-adjacent (string xref at `L70257`). Resolves
an OID via `sub_56479970` to a descriptor (`edi`), checks descriptor flags at
`+0x0E` (`0x400` deferred, `0x1100`), extracts a packed slot id
(`[edi+8] >> 0x14`, `L70110`), then routes:

- **SerialCluster path** (slot-id sign bit set, `L70129` `jns`): `sub_5648C0F0`
  / `sub_5648BBA0` → `call dword ptr [ecx+70h]` (= `sub_56493F50`,
  `SerialCluster::vtable+0x70`, `L70160`) with the descriptor + arg
  (lazy-load the persisted body).
- **vtable path** (else): `sub_5648BE20` / `sub_5648C170` →
  `call dword ptr [eax+50h]` (`L70209`).

Read-side slot map: per-type body read = **object `vtable+0x18`**;
SerialCluster load = **`vtable+0x70`**; cluster-open helper = **`vtable+0x50`**.

## What this proves / does not prove (vs the acceptance gate)

- **Proven (Mode A):** the envelope (`type + btf + oid + aux`) and the
  *mechanism* of body parsing — a per-type `vtable+0x18` Read under an
  IOContext, bounded by `bytes_to_follow`. The `0x0010` size buckets cannot be
  discriminated in any **shared** code path; discrimination can only happen
  **inside the type-`0x0010` object's `vtable+0x18` Read**.
- **Not proven (the gate question):** the post-`oid+aux` skip, the branch
  offset, and the size-31 / size-16 / size-86 fixed field layouts. Those live
  in the type-`0x0010` object's `vtable+0x18`. Per the prior IDA evidence the
  `1D1928C0` factory is `E_NOTIMPL` and objects load through SerialCluster
  (persist-container profile; no geometry class in `radsrvitem.dll`), so the
  expected behavior is that `0x0010`'s `vtable+0x18` either returns the
  deferral `0x80040233` (opaque SerialCluster blob) or reads generic
  attribute-fragment bytes (the Phase 26 text sub-family) — **not** a
  geometry-style discriminated fixed-field record. Mode A (asm only) cannot
  trace from the type code through `sub_56490B00` to that concrete class vtable
  with confidence; that needs the decompiler.

## Conclusion (negative closeout, docs-only)

This matches the plan's rollback criterion verbatim — *"the reader only proves
runtime dispatch or type identity ... or `radsrvitem.dll.i64` cannot be opened
and Mode A is inconclusive [for the size-31 width]."* → **keep `ROADMAP-0010`
at `TypedAudit`.** No parser, DTO, schema, writer, bundle, byte-audit, ratchet,
or confidence change. The raw `0x0010` decoder (ratchet 582) and the Phase 26
attribute-fragment decoder (ratchet 84) are untouched; `leading_word` remains
an audit field describing `payload[0..2]` and is **never** renamed `sub_kind`.

Net new evidence vs the 2026-06-30 plan: the exact in-binary body-read
dispatch site (`vtable+0x18`), the `0x80040233`→bit-`0x400`→SerialCluster
deferral linkage, the read-side vtable slot map, and an independent raw-PE
re-verification of the type-table entries. This narrows the remaining unknown
from "where/how is `0x0010` read" to the single concrete artifact "the
type-`0x0010` object's `vtable+0x18` Read", which is a Mode-B target.

## Sharpened re-open trigger (Mode B — single highest-ROI step)

Open `dlls\radsrvitem.dll.i64` in IDA (auto-starts a new MCP port), then:

1. `decompile` / `analyze_function` `sub_56490B00` (`L145614`) to recover the
   type-code → object-class instantiation for GUID `1D1928C0`.
2. Decompile that class's **`vtable+0x18`** Read; decide whether it returns
   `0x80040233` (defer → SerialCluster opaque blob) or reads fixed fields.
3. Only if it reads fixed fields: `trace_data_flow` the IOContext to the first
   branch `cmp`; record the discriminator offset (expected `≥ inner`, i.e.
   `payload[12..]`) and verify the recovered width reproduces the size-31
   bucket (182 records) across `≥ 2` fixtures before any parser slice.
4. In parallel, the concrete class name likely lives in the absent
   `style.dll`; open it if available.

## Evidence anchors (asm line ↔ address)

| Symbol / site | asm line | address / note |
|---|---|---|
| `PSMSerializeIn sub_564915E0` | `L147070` | top-level deserializer |
| type extract `(rec[8]>>6)&0x3FFF` | `L147350–147353` | — |
| body read `call [ecx+18h]` (vtable+0x18) | `L147559` | IOContext at `[edi+8]` |
| defer `0x80040233` → `or [eax+0Eh],400h` | `L147561–147567` | SerialCluster handoff |
| btf consume check (`sub_56491050`) | `L147594–147603` | `cursor-var_840` vs `var_82C` |
| `sub_56468B30` OID→object read | `L70027` | `SegmentTable::GetIGDS9Pointer`-adjacent |
| flag `test eax,400h` (descriptor+0x0E) | `L70094` | deferred bit |
| slot id `[edi+8]>>0x14` | `L70110` | — |
| SerialCluster `call [ecx+70h]` (`sub_56493F50`) | `L70160` | vtable+0x70 |
| vtable `call [eax+50h]` | `L70209` | vtable+0x50 |
| `sub_56490B00` (type→object instantiate) | `L145614` | Mode-B target |
| raw-PE type table | — | `0x0010`@`0x23A5A8`, `0x0115`@`0x23BA0C` |

## References

- `docs/analysis/2026-06-30-phase33-0010-discriminator-next-ida-evidence-plan.md`
  (the Mode A/B plan executed here)
- `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
  (type-table identity, factory `E_NOTIMPL`, SerialCluster profile)
- `docs/analysis/2026-06-23-phase33-0010-discriminator-ida-evidence.md`
  (superseded tooling-gated stop condition)
- `docs/plans/2026-06-19-pid-parser-roadmap-gates.md` (`ROADMAP-0010`)
- `src/parsers/sheet_records.rs` (raw `0x0010` decoder ~L3891; attribute
  fragment decoder ~L4021)
- `examples/probe_psm_0x0010_sub_kind.rs`
