# Phase 33 PSM `0x0010` Discriminator — Next-Step IDA Evidence Plan

> Date: 2026-06-30
> Scope: resumable, evidence-gated next-step plan for the SmartPlant /
> Smart P&ID Sheet PSM sub-record family `0x0010` discriminator.
> Status: planning artifact only. No parser, DTO, schema, writer, bundle,
> or roadmap confidence change is introduced by this note.
> Supersedes the stop condition in
> `docs/analysis/2026-06-23-phase33-0010-discriminator-ida-evidence.md`
> (see "Reframing" below).

## Reframing: the blocker is "IDB not open", not "binary unavailable"

The 2026-06-23 closeout was a *tooling-gated* negative: `list_instances`
returned only `sppid.dll` (`13337`) and `core.dll` (`13338`), so the plan
correctly stopped before parser work. That is still the live instance
state on 2026-06-30:

| Module | Host:Port | IDB path | Reachable | Active |
|---|---|---|---|---|
| `sppid.dll` | `127.0.0.1:13337` | `dlls\sppid.dll.i64` | yes | yes |
| `core.dll` | `127.0.0.1:13338` | `D:\AVEVA\Everything3D3.1\core.dll.i64` | yes | no |

But the binary inventory tells a different story. The local `dlls/`
reverse-asset library (gitignored — `Glob`/`Grep` skip it, use
`Select-String` / `--no-ignore`) **already contains the #1 preferred
persistence target as a prebuilt IDA database**:

| Asset in `dlls/` | Size | Meaning |
|---|---:|---|
| `radsrvitem.dll` | 3.84 MB | RAD persisted-object server (raw PE) |
| **`radsrvitem.dll.i64`** | **36.95 MB** | **prebuilt IDA database — just needs opening** |
| `radsrvitem.dll.asm` | 40.15 MB | full IDA disassembly export (Mode-A source) |
| `j2dsrv.dll` | 189 KB | 2D record / Sheet projection candidate (no IDB) |
| `xceedrad.dll` | 150 KB | RAD geometry helper candidate (no IDB) |
| `ubspm2d1.dll` | 2.67 MB | 2D geometry server candidate (no IDB) |
| `ugeom2d1.dll` | 598 KB | 2D geometry server candidate (no IDB) |
| `radsrv.dll`, `RadNetAutomation.dll`, `radnetbridge.dll`, `RadNetSymbolLabel.dll`, `Interop.RAD2D.dll`, `rad2d.dat` | — | RAD family siblings |

So the Phase 33 re-open trigger ("a reachable IDA instance for
`radsrvitem.dll`") is **satisfiable in one action**: open
`dlls\radsrvitem.dll.i64` in IDA, which auto-starts the MCP server on a
new port. There is no missing-binary blocker for the primary target.
`style.dll`, `OLESITE.dll`, `OLECRT.dll`, and `jengine.dll` remain absent
from `dlls/`, but the `0x0010` Read path lives in `radsrvitem.dll`, which
is present.

Additionally, the IDA MCP tool surface is now far richer than the
2026-06-21 short-name surface (which lacked `idalib_open`). It now
includes `decompile`, `analyze_function`, `trace_data_flow`, `read_struct`,
`xrefs_to`, `get_bytes`, `find_bytes`, `find_regex`, `search_text`,
`make_signature`, and `py_exec_file` — enough to resolve the Read path
fully in Mode B.

## What is already proven (do not re-derive)

From `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md` and
`docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md` (and the landed
Phase 26 decoder):

1. **Type-table identity (confirmed).** `radsrvitem.dll`
   `PersistTypeTable<PersistComTypeEntry>` (`dword_567DDC90`, 281 × 20-byte
   entries at static base `0x5667B068`) maps index `0x0010` →
   GUID `1D1928C0-0000-0000-C000-000000000046`, `tail16=0x40`,
   `tail17=0x06`, `parent=0x0115`. Root alias `0x0115` has the same GUID,
   `tail16=0xC0`, `parent=0`. The table is GUID-sorted (binary search), so
   PSM type codes carry no ordinal semantics. Independently reproduced
   from raw PE bytes at file offsets `0x23a5a8` / `0x23ba0c`.
2. **Envelope (confirmed by Phase 14 Slice A + `.asm` re-trace).** A
   top-level on-disk PSM record is
   `type(2) + bytes_to_follow(4) + oid(4) + aux(8) + inner(var)`. For
   `0x0010` this means the current decoder's `raw_payload` (everything
   after the 6-byte `type+btf` header) is `oid(4) + aux(8) + inner`.
3. **`leading_word` is an oid fragment, NOT a sub-kind (resolved, H2).**
   `raw_payload[0..2]` is the low 16 bits of `oid`/marker. Fixture
   cross-check: JStyleOverride references a `0x0010` record with
   `oid = 65538 = 0x10002` → LE bytes `02 00 01 00` → `leading_word =
   0x0002`, which is exactly the ~28% bucket. **Never rename
   `leading_word` to `sub_kind`.**
4. **Attribute/annotation text sub-family decoded (Phase 26, audit-only).**
   A large subset carries `marker(4) + aux(8) + [u16 len + UTF-16LE]*`
   real engineering text (instrument tags, line numbers `<size>-<service>-<seq>`,
   DN sizes, drawing numbers, CJK labels). Landed as additive,
   content-gated `decode_attribute_fragments` /
   `SheetAttributeFragmentDecoded` (`src/parsers/sheet_records.rs`
   ~L4021/L3988). Cross-fixture ratchet = **84** fragments; raw Phase-18
   baseline preserved at **582**.
5. **Factory shape (qualitatively resolved).** The direct COM factory for
   `1D1928C0...` is an `E_NOTIMPL` stub; objects are instantiated via the
   `SerialCluster` lazy-load path. RTTI in `radsrvitem.dll` exposes only
   the persist framework (`PersistManager`, `SerialCluster`,
   `PersistTypeTable`, `IJPersist*`, `tagAnnotPersistData`,
   `tagDimPersistData`, …) — no geometry class for this GUID. Profile is
   consistent with an **internal persist container / attribute fragment**,
   not a named geometry class. The concrete class name (if any) likely
   lives in the absent `style.dll`; it is not required for a byte-layout
   discriminator.

### Key address / anchor index (verified present in `dlls/radsrvitem.dll.asm` on 2026-06-30)

| Symbol | Address / asm line | Role |
|---|---|---|
| `PSMSerializeIn` `sub_564915E0` | `0x564915E0` | top-level deserializer; type extract `([rec+8]>>6)&0x3FFF` at `0x564918DE..EF` |
| `PSMSerializeOut` `sub_56491E80` | `0x56491E80` | serializer; mirrors packed-id reads |
| `sub_56468B30` | asm **line 70027** | OID/slot resolver → vtable or `SerialCluster` lazy-load (Read-path entry) |
| `sub_56471660` | referenced at asm **line 6415** (`push offset sub_56471660`) | category callback; accepts `tail17` categories incl. `0x06` |
| runtime type table | `dword_567DDC90` | `PersistTypeTable` |
| static type table base | `0x5667B068` | 281 × 20 B |
| static entry `0x0010` | `0x5667B1A8` | GUID `1D1928C0...` |
| static entry `0x0115` | `0x5667C60C` | same GUID, root alias |
| callback global | `dword_567DDC60` → `sub_56471660` | category dispatch |
| OID/slot resolver | `sub_56479970` | packed OID lookup |
| `SerialCluster` ctor | `sub_56493840` | storage path |

## Refreshed fixture evidence (2026-06-30 probe)

`cargo run --release --example probe_psm_0x0010_sub_kind` over `/Sheet6`
in the four Sheet-bearing fixtures (advancing scan, 578 records; the
decoder ratchet is 582 — the 4-record gap is the probe's narrower
acceptance window, already documented in the Phase 19 ratchet test):

**The family is multi-modal, and the modes separate by `(marker-word, size)`:**

| Cluster | Signature | Example buckets (size → count) | Interpretation |
|---|---|---|---|
| **Small-marker / text** | `word@+0 ∈ {0x0001,0x0002,0x0003,0x0004}` (often 100% of bucket), variable size | 41→16, 76→24, 50→16, 27→9, 45→8, 36→11 (all `0x0003`) | attribute/annotation fragments (Phase 26 decodes the clean-UTF-16LE ones) |
| **Size-31 binary** (dominant) | `word@+0` fully heterogeneous, `0x0002 = 0%` | **31→182** | fixed-width binary record; **highest-ROI single target** |
| **Size-16 binary** | clusters at `0x4C1C/0x4E1C`, `byte@+0 0x1C = 66%` | 16→24 | fixed-width binary record |
| **Size-86 binary** | clusters at `0x8EA5` (85%) | 86→7 | fixed-width binary record |
| **Other binary** | heterogeneous | 70→53, 13→21, 43→18, 74→11, 58→10 | binary, mixed widths |

Global: `byte@+1 = 0x00` is 36% (consistent with a small oid/marker low
word). `word@+0 = 0x0002` 28% / `0x0003` 3% / `0x0001` 3%.

**The crux this exposes:** the marker word at `payload[0..2]` only crudely
separates "small-marker/text" from "binary"; it is **not** the structural
sub-kind. The dominant size-31 bucket (182 records, ~31% of the family)
carries no `0x0002` marker and is exactly the binary variant Phase 26
left raw. The authoritative discriminator must come from the Read path —
specifically *what offset inside `inner` (= `payload[12..]`) the `0x0010`
reader branches on, and what fixed fields each branch consumes.*

## The single question IDA must answer

> In the `0x0010` Read/DoIO path, after the `oid(4) + aux(8)` envelope,
> (a) how many bytes are skipped before the variant body, (b) what byte/word
> offset does the reader branch on to choose a variant, and (c) what is the
> fixed field layout it reads for the size-31 (and size-16, size-86) binary
> variants?

Everything else (text sub-family, oid semantics, type identity) is already
resolved. A pure "type identity" or "runtime dispatch" answer is **not**
sufficient — per the promotion gate, the candidate function must read
persisted bytes and the branch offset must reconcile with the fixture
size buckets above.

## Execution plan — two modes

### Mode A — static `.asm` / raw-PE read (no IDA instance required; do this first)

Lowest cost, available immediately, no environment dependency. Source:
`dlls\radsrvitem.dll.asm` (40 MB) + `dlls\radsrvitem.dll` raw bytes via a
local Python `open()`.

1. **Re-verify the type-table entry** at file offsets `0x23a5a8`
   (`0x0010`) and `0x23ba0c` (`0x0115`): GUID `1D1928C0...`, `tail16`,
   `tail17`, `parent`. Confirms the asset matches Phase 20.
2. **Read the Read-path control flow** around `sub_56468B30` (asm
   line 70027) and `sub_56471660` (def near the `push offset sub_56471660`
   at line 6415). Extract: the byte/dword loads off the record buffer
   pointer, the post-envelope skip, and any `cmp`/`movzx` on a field that
   gates a branch. Note candidate discriminator offsets.
3. **Cross-read `PSMSerializeIn` `sub_564915E0`** to confirm the
   `type(2)+btf(4)+oid(4)+aux(8)` envelope handling is shared, so the
   `inner` base for `0x0010` is `payload[12..]`.
4. Record candidate offsets + the asm line ranges in the evidence note.
   Mode A can *propose* a discriminator offset but should not be treated
   as final without Mode B decompiler confirmation (raw asm is easy to
   mis-slice on register reuse).

### Mode B — open `radsrvitem.dll.i64` (authoritative; resolves the question)

1. Open `dlls\radsrvitem.dll.i64` in IDA (File → Open) so a new MCP port
   appears. Then `list_instances` → `select_instance(<radsrvitem-port>)` →
   `survey_binary(detail_level="minimal")`.
2. `decompile`/`analyze_function` on `sub_56468B30` and `sub_56471660`.
   Identify the record-buffer pointer parameter and the post-`oid+aux`
   read sequence.
3. `trace_data_flow` from the record buffer pointer to the first
   branch comparison; record the **discriminator offset** (expected in
   `inner`, i.e. ≥ `payload[12]`) and the compared constants.
4. For each variant branch, read the field-by-field consumption to derive
   the **fixed layout of the size-31 variant** (and size-16, size-86).
   Use `read_struct` if a struct type is applied; otherwise hand-derive
   from the load offsets.
5. `xrefs_to` the branch targets and the type-table entry to confirm the
   path is the persisted reader (not a runtime-only helper).
6. If the trace crosses into 2D geometry, create IDBs for the on-disk
   `j2dsrv.dll` / `ubspm2d1.dll` / `ugeom2d1.dll` / `xceedrad.dll` and
   follow the xref. (These have no `.i64` yet.)

## Mapping IDA findings → fixture buckets (acceptance)

A discriminator is **accepted for a parser slice** only if all hold:

- The branch offset is inside `inner` (`payload[12..]`) — consistent with
  the proven `oid(4)+aux(8)` envelope; if instead the reader branches on
  `payload[0..2]`, that contradicts H2 and must be reconciled before any
  naming.
- The recovered fixed layout **predicts the size-31 record width exactly**
  (31 = the field sum the reader consumes for that variant), and likewise
  for at least one more fixed bucket (size-16 or size-86).
- The discriminator value partitions records the same way across **≥ 2
  fixtures** (e.g. DWG-0201 + 工艺管道-1), not one fixture.
- Each proposed field has a half-open byte range and a representative
  decoded value from a real fixture record.

## Promotion gate (unchanged) and rollback

If accepted, the future parser slice (separate from this doc) must:

- add red unit tests for the proven variant in
  `parsers::sheet_records::tests`;
- add a narrow `decode_*` helper without disturbing the raw Phase-18
  decoder or the Phase-26 attribute decoder (both byte-for-byte stable);
- keep the raw `0x0010` ratchet at **582** and attribute ratchet at **84**
  (or update both ratchets in the same change with justification);
- mirror only proven fields into model/schema; keep `inner` tails that are
  not structurally validated as leftover (no "stream consumed" shortcut);
- add `tests/parser_panic_safety.rs` coverage for the new entry point;
- update the atlas + roadmap confidence only for the proven variant.

**Rollback / negative-closeout criteria** (keep `ROADMAP-0010` at
`TypedAudit`, change docs only): the reader only proves runtime dispatch
or type identity; the branch offset does not reconcile with the size-31
width; the partition is single-fixture; or `radsrvitem.dll.i64` cannot be
opened and Mode A is inconclusive.

## Updated re-open trigger / next action

- **Primary next action:** open `dlls\radsrvitem.dll.i64` and run Mode B
  steps 1–5 against `sub_56468B30` / `sub_56471660`. This is the single
  highest-ROI step and is no longer environment-blocked.
- **Fallback (no IDA session):** run Mode A static `.asm` reads now to
  pre-stage candidate discriminator offsets.
- **Target priority for the discriminator:** size-31 binary variant (182
  records) first; then size-16 and size-86.

## Command appendix

Fixture-side refresh:

```powershell
cargo run --release --example probe_psm_0x0010_sub_kind
cargo run --release --example probe_psm_0x0010_shape
cargo test --locked --test parse_real_files sub_records_0x0010 -- --nocapture
cargo test --locked --test parse_real_files jstyle_override_decoder_emits_audit_records_with_provenance -- --nocapture
```

IDA Mode-A (static, dlls/ is gitignored → use Select-String):

```powershell
Select-String -Path dlls\radsrvitem.dll.asm -Pattern 'sub_56468B30|sub_56471660|sub_564915E0|sub_56491E80'
# then read the surrounding asm line ranges and the raw-PE type-table bytes via a local python open()
```

IDA Mode-B (after opening dlls\radsrvitem.dll.i64):

```text
list_instances
select_instance(<radsrvitem-port>)
survey_binary(detail_level="minimal")
decompile(0x56468B30); decompile(0x56471660)
analyze_function(0x56468B30); analyze_function(0x56471660)
trace_data_flow(<record-buffer-ptr>)
xrefs_to(0x5667B1A8)        # type-table entry 0x0010
get_bytes / read_struct around the recovered variant layout
```

## References

- `docs/analysis/2026-06-22-phase33-0010-discriminator-ida-plan.md`
- `docs/analysis/2026-06-23-phase33-0010-discriminator-ida-evidence.md` (superseded stop condition)
- `docs/specs/2026-06-22-phase33-0010-discriminator-dev-test-plan/` (spec package)
- `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
- `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`
- `docs/plans/2026-06-19-pid-parser-roadmap-gates.md` (`ROADMAP-0010`)
- `docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`
- `src/parsers/sheet_records.rs` (raw `0x0010` decoder ~L3891; attribute fragment decoder ~L4021)
- `examples/probe_psm_0x0010_sub_kind.rs`, `examples/probe_psm_0x0010_shape.rs`
