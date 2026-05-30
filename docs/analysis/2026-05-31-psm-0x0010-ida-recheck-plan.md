# PSM type `0x0010` IDA re-check plan (Phase 20 follow-up)

> Date: 2026-05-31
> Scope: Phase 20 follow-up — code-side audit + IDA re-check plan
> Status: **blocked (environment)** — the SmartPlant target DLLs are not
> present on this workstation, so no new IDA disassembly was possible.
> Prereq to execute: `radsrvitem.dll` (+ `style.dll`) loaded in IDA, or
> available as a raw PE file for static `py_eval` reads.

## TL;DR

This session could not advance the IDA disassembly because the only
reachable IDA instance is AVEVA E3D `core.dll` (unrelated product) and
the historical SmartPlant idbs under `E:\reverse\pid\` are gone. Instead
it produced two **new, code-side** insights that should be the first
things verified once a real `radsrvitem.dll` is available:

1. The current `0x0010` decoder assumes a **6-byte header**
   (`type + bytes_to_follow`). Phase 14's `PSMSerializeIn` top-level
   layout is `type(2) + btf(4) + oid(4) + aux(8) + inner`. **If `0x0010`
   also carries `oid + aux`, then the current `leading_word =
   payload[0..2]` is actually the low 16 bits of the record `oid`, not a
   sub-kind discriminator.** This neatly explains the Phase 19 puzzle
   (heterogeneous `+0` across size buckets = oid naturally disperses;
   `leading_word == 0x0002` at ~28% = oid-low clustering for one object
   class).
2. The three `style.dll` GUID constants at `.rdata:0x10068F44`
   (`1D1928C0...` / `09D6BBB0...` / `8EC51800...`) are all in the
   OLE-reserved `*-0000-0000-C000-000000000046` space and sit
   **contiguously** — i.e. a GUID **array**. Phase 20 searched the xref
   of the **single** GUID address (`0x10068F44`, result `0`) and
   declared it unreferenced. The correct move is to find the xref of the
   **array base** (which may be at an address `< 0x10068F44`), because
   array consumers index `base + n*16`, never the middle element
   directly.

## Environment blocker (2026-05-31, evidence)

| Probe | Result |
|---|---|
| `ida-pro-mcp list_instances` | single instance: `core.dll`, port 13337, idb `D:\AVEVA\Everything3D3.1\core.dll.i64` |
| `py_eval` GUID byte scan in `core.dll` | `guid_1D1928C0_hits = []` over a 64.5 MB image → confirms AVEVA E3D is unrelated |
| `E:\reverse\pid\` listing | `itacad.ini`, `install.log`, `ImageScape.ini`, `itmstn.ini`, `acadmstn.rgt`, `ISServer.rgt`, `Prod.rgt`, `acfpdf.txt`, `draft.ver`, `resdlls\...` — **no** `radsrvitem.dll` / `style.dll` / `*.i64` |

Conclusion: PSM `0x0010` IDA reverse engineering cannot start on this
machine. The Phase 16/20 idbs (`E:\reverse\pid\radsrvitem.dll.i64`,
`style.dll.i64`) are no longer on disk; that folder now holds Intergraph
ImageScape / I-CAD configuration instead.

## Current Rust decoder assumptions

Source: `src/parsers/sheet_records.rs` (`decode_sub_record_0x0010_at`).

```text
offset+0 .. +2   type_word (u16 LE); type_code = type_word & 0x3FFF == 0x0010
offset+0 .. +2   type_flags = type_word >> 14   (top 2 bits)
offset+2 .. +6   bytes_to_follow (u32 LE)         [accepted range 8 ..= 100000]
offset+6 .. end  raw_payload (len == bytes_to_follow)
raw_payload[0..2] leading_word (u16 LE)           [audit-only, byte-position name]
```

The decoder is deliberately audit-only (Phase 18 template): stable
6-byte PSM header + raw payload + provenance, no sub-kind naming, no
`PidGraphicKind` emission.

## New insight #1 — header may omit `oid + aux`

Phase 14 Slice A (`PSMSerializeIn` / `PSMSerializeOut`,
`radsrvitem.dll!sub_564915E0` / `sub_56491E80`) recorded the top-level
PSM record layout as:

```text
type(2 LE) + bytes_to_follow(4 LE) + oid(4 LE) + aux(8) + inner_payload(var)
```

The `0x0010` decoder intentionally stops after the 6-byte
`type + btf` prefix. Two mutually exclusive hypotheses follow, and only
IDA can decide:

| Hypothesis | Then `leading_word` (`payload[0..2]`) is… | Predicted fixture signature |
|---|---|---|
| **H1: `0x0010` is a true sub-record / fragment** (no `oid+aux`) | the first bytes of the real inner payload | a genuine discriminator candidate |
| **H2: `0x0010` carries `oid(4)+aux(8)` like top-level records** | the **low 16 bits of `oid`** | dispersed across size buckets, with incidental clustering (matches Phase 19: `0x0002` ~28%, buckets 31/70/13/16/43 heterogeneous at `+0`) |

The Phase 19 distribution already **leans toward H2**. Verifying H1 vs
H2 is the single highest-ROI cut, because under H2 the whole
"`leading_word` as sub-kind" line of inquiry is a dead end and the real
discriminator lives at `payload[12..]` (after `oid+aux`).

## New insight #2 — find the GUID **array base** xref

Phase 20 facts (from `2026-05-17-phase20-psm-0x0010-rad-class.md`):

- `style.dll .rdata:0x10068F44` = `1D1928C0-0000-0000-C000-000000000046`
- immediately followed by `09D6BBB0-...` and `8EC51800-...`
- `0x10068F44` has **zero** static xrefs

The missed step: these are a contiguous GUID array. The consumer does
`base + n*16`, so the *element* address never gets a static xref, but the
*base* address does. Re-check plan:

1. Walk **backward** from `0x10068F44` in 16-byte steps while each slot
   still matches the `*-0000-0000-C000-000000000046` shape; the first
   non-matching slot + 16 is the array base.
2. Walk **forward** the same way to get the full element count.
3. `find_xref_signatures(base)` (not the middle element) → the indexing
   function. That function names the family and likely reaches an
   `IJPersist` / factory path for `1D1928C0...`.

## IDA re-check table

| Item | Current Rust assumption | Phase 18/19/20 evidence | What IDA must confirm |
|---|---|---|---|
| header length | 6 B (`type + btf`) | top-level PSM = `type + btf + oid(4) + aux(8)` | does the `0x0010` Read skip 6 or 18 bytes before payload? |
| `type_code` | `& 0x3FFF == 0x10` | PersistTypeTable idx `0x10`, GUID `1D1928C0...` | confirmed; re-verify `.data:0x5667B1A8` |
| `type_flags` | `>> 14` (2 bits) | `tail16=0x40`; alias `tail16=0xC0` | are record flags the same field as `tail16`? |
| `leading_word` | `payload[0..2]` | `0x0002`=28%; buckets heterogeneous | is it `oid` low-16 (H2) or a true tag (H1)? |
| sub-kind offset | none (deferred) | `+0` heterogeneous for sizes 31/70/13/16/43 | which offset does `Read`/`DoIO` branch on? |
| references | none | JStyleOverride `+38..41` / `+56..59` reference its oid | confirm the `oid` field position in the header |

## Five-step battle plan (once `radsrvitem.dll` is loaded)

1. `list_instances` → note the new port (it changes per IDA restart);
   `select_instance(<port>)`.
2. Re-confirm PersistTypeTable entry `0x0010` at `.data:0x5667B1A8`
   (GUID / `tail16=0x40` / `tail17=0x06` / `parent=0x0115`) is stable.
3. **Core cut:** trace the Read path
   `sub_56468B30 → sub_56471660` (category `tail17=0x06`) and read how
   many header bytes are consumed before the payload, and which offset
   the sub-kind branch keys on. Directly decides H1 vs H2.
4. `style.dll` GUID-array base discovery + base xref (insight #2).
5. Cross-instance byte search for
   `C0 28 19 1D 00 00 00 00 C0 00 00 00 00 00 00 46` across **all**
   reachable instances (Phase 20 only covered radsrvitem + style + 6
   siblings).

## Two execution modes

| Mode | Prereq | Reaches |
|---|---|---|
| **A — raw PE, no IDA** | path to `radsrvitem.dll` / `style.dll` on disk; `py_eval` opens the file and parses the PE | static-data items only: re-verify PersistTypeTable `0x0010` entry, enumerate the `style.dll` GUID array. Cannot read disassembly, so cannot resolve H1/H2. |
| **B — IDA full** | `radsrvitem.dll` open in IDA | everything above + the Read-path header-skip and sub-kind branch (steps 3–5). The only mode that resolves the core question. |

## Do-not-regress reminders

- `0x0010` stays **audit-only** until H1/H2 is resolved; do **not** rename
  `leading_word` to `sub_kind`.
- `decoded_sub_records_0x0010` cross-fixture baseline = `582`
  (DWG-0201=161, DWG-0202=104, 工艺管道-1=306, A01=11). Any decoder
  change must preserve this count or update the ratchet in the same change.
- No new `PidGraphicKind` variant from `0x0010` evidence alone.

## 2026-05-31 update — Mode-A static verification + fixture cross-check

### Asset discovery

`dlls/` (gitignored, so `Glob`/`Grep` skip it; use `--no-ignore` or
`Select-String`) turned out to be a full SmartPlant P&ID reverse asset
library (~110 files): `radsrvitem.dll` + **`radsrvitem.dll.i64` (IDA
database, 36 MB)** + **`radsrvitem.dll.asm` (full disassembly export,
39 MB)** + `j2dsrv.dll`, `sppid.dll` (+idb), `llama.dll` (+idb),
`smartplantpid.exe` (+idb), `sppidautomation.dll.i64`, `ubspm2d1.dll`,
`ugeom2d1.dll`, `Interop.RAD2D.dll`, plus many `sppiddatamap_*.xml`
version maps. **No `style.dll`** — insight #2's GUID-array enumeration
still lacks its binary (but `j2dsrv.dll` is present).

### PersistTypeTable independently reproduced (Mode A, raw PE)

Reading `dlls/radsrvitem.dll` **raw bytes** (not the idb, via `py_eval`
`open()`), GUID `1D1928C0` appears at three file offsets; the two table
entries match Phase 20 exactly:

| file offset | tail16 | tail17 | parent | = Phase 20 entry |
|---|---|---|---|---|
| `0x223b44` | (.rdata constant) | — | — | `.rdata` GUID |
| `0x23a5a8` | `0x40` | `0x06` | `0x0115` | `0x0010` |
| `0x23ba0c` | `0xC0` | `0x06` | `0x0000` | `0x0115` alias |

Spacing `0x23ba0c − 0x23a5a8 = 0x1464 = 5220 = 261 × 20 = (0x115 − 0x10)`
entries × 20-byte stride — independently confirms the 20B
`PersistComTypeEntry` layout without IDA.

### Insight #1 now has strong fixture support for H2

Cross-referencing **existing** probe evidence (no new disassembly):

- Phase 16 probe v5: JStyleOverride references a `0x0010` record with
  `oid = 65538`.
- `65538 = 0x10002` → little-endian bytes `02 00 01 00`.
- Phase 18 independently recorded `02 00 01 00` as the 0x0010 "leading
  discriminator".
- `payload[0..2] = 02 00 = 0x0002` = the `leading_word` value covering
  ~28% of records.
- Phase 20 oid packing: `page = oid >> 13`, `slot = oid & 0x1FFF` →
  `0x10002` = page 8, slot 2.

Conclusion: the current `leading_word = payload[0..2]` is **very likely
the low 16 bits of the record oid (slot)** (H2), not a sub-kind tag. The
6-byte-header assumption is probably missing `oid(4) + aux(8)`:
`payload[0..4]` is the oid, and any real sub-kind/discriminator lives at
`payload[12..]`. This is strong-but-not-final; the authoritative check is
the 0x0010 Read path disassembly.

### Final confirmation path

`radsrvitem.dll.i64` is present in `dlls/`. Opening it in IDA
(File → Open → `dlls\radsrvitem.dll.i64`) lets `analyze_function` on the
0x0010 Read path (`sub_56468B30` → `sub_56471660`, category `tail17=0x06`)
confirm the exact header-skip in seconds — faster than reading the 39 MB
`.asm`. For the manual route, `PSMSerializeIn` (`sub_564915E0`) is at
`radsrvitem.dll.asm` line 147070.

### Insight #1 conclusion — H2 (multi-evidence; byte-final check still advisable)

The `.asm` re-trace closes the loop on H2 without the idb:

- `PSMSerializeOut` (`sub_56491E80`, asm line 147815) at `loc_56491F00`
  reads the in-memory record's packed type from `[ebx+8]` via
  `shr 6 / and 3FFFh` (type) and `shr 0x14` (a second packed field) —
  identical to `sub_56468B30` (asm 70027) and PSMSerializeIn. The record
  object clearly carries oid / packed-id state beyond the bare type word.
- Phase 14 Slice A already established the on-disk envelope as
  `type(2) + btf(4) + oid(4) + aux(8) + inner`.
- Therefore a top-level on-disk `0x0010` record's bytes after the 6-byte
  `type+btf` prefix are `oid(4) + aux(8) + inner`, so the decoder's
  `payload[0..4]` is the oid and `leading_word = payload[0..2]` is the
  oid's low 16 bits (slot).
- Fixture cross-check is consistent: JStyleOverride → `0x0010` oid 65538
  = `0x10002` = bytes `02 00 01 00` = the observed `payload[0..4]`.

Net: **H2 holds.** `leading_word` is an oid fragment, not a sub-kind tag.

### Proposed decoder evolution (DEFERRED — do not implement yet)

When a future session can open `dlls\radsrvitem.dll.i64` and confirm the
0x0010 inner read at the byte level (and verify that scanned hits are
top-level records, not embedded fragments), the audit DTO should evolve:

| current | proposed |
|---|---|
| `raw_payload` from +6 | `oid: u32` = payload[0..4]; `aux: [u8;8]` = payload[4..12]; `inner: Vec<u8>` = payload[12..] |
| `leading_word = payload[0..2]` | deprecated alias of `oid & 0xFFFF` |
| (no sub-kind) | sub-kind discriminator, if any, searched in `inner` (payload[12..]) |

Constraints unchanged: audit-only, `582` baseline preserved, no
`PidGraphicKind` emission, no `confidence` / `kind` change. The rename is
a breaking DTO change → requires the byte-final IDA confirmation first
(also to rule out embedded-fragment hits that would lack the envelope).

## 2026-05-31 follow-up — human class name attack (qualitative result)

Phase 20 never recovered a human class name for GUID `1D1928C0`. New
angles tried this session (all via `py_eval` raw-file reads of `dlls/`):

| Angle | Result |
|---|---|
| .NET interop metadata (`Interop.RAD2D.dll`, `RadNetAutomation.dll`) | GUID `1D1928C0` **0 hits** (binary + ascii) — COM interop does not expose this persist GUID |
| Cross-DLL GUID scan over all ~110 `dlls/` files | `1D1928C0` appears **only in radsrvitem.dll** (3×); even control GUID `19F333B0`/JStyleBase only in radsrvitem (its class lives in the absent style.dll) |
| radsrvitem.dll MSVC RTTI (188 classes) | **No concrete geometry classes.** Only the persist framework: `PersistManager`, `SerialCluster`, `EnumSerialCluster`, `PersistTypeTable<PersistComTypeEntry>`, `IJPersist`, `IJPersistCluster`, `IJPersistClusterPriv`, `IJPersistRoot`, `IJPersistManager`, `IGDSFactoryAttributeSet`, `tagAnnotPersistData`, `tagDimPersistData`, `tagDimGrpPersistData`, `IJPersistImp@JStyleBase` |

### New structural finding — PersistTypeTable is GUID-sorted

Dumping entries `0x08..0x18` shows the 281-entry table is sorted ascending
by GUID `Data1` (16E8 < 1777 < 18BA < 1918 < 19F3 < 1B2C < 1BA3 < **1D19**
< 1E1E < … < 2D4E) — i.e. a binary-search lookup keyed on the COM GUID,
which is why PSM type codes carry no ordinal semantics. Notable:

- idx `0x000C` = `19F333B0-4F81-11D0-A223-080036A1CF02` = **JStyleBase**
  (independently confirms the Phase 20 control GUID and proves the
  type-code→class mapping works).
- Most neighbors are `...-080036xxxxxx` (Intergraph OUI `08-00-36`).
  `1D1928C0-0000-0000-C000-000000000046` is an **OLE-reserved-format
  outlier**; idx `0x0008` (`16E812A2-0000-0000-C000-000000000046`,
  tail16=0x40 tail17=0x05) is the same OLE-format family.

### Qualitative conclusion

0x0010's profile — OLE-format GUID, registered in the persist type table,
factory is an `E_NOTIMPL` stub, instantiated via the `SerialCluster`
lazy-load path, polymorphic payload, "embedded fragment inside larger
records" — is most consistent with an **internal persist
cluster/root/fragment container type of the RAD persistence framework,
NOT a named geometry/object class**. That is why no geometry class name
exists: there isn't one. The exact mangled name (if any) would live in
the absent `style.dll` or be a non-RTTI internal type. This downgrades the
"human class name" unknown from *open* to *qualitatively resolved*:
**0x0010 = persist container/fragment, not geometry** — consistent with
keeping it audit-only and never emitting a `PidGraphicKind`.

## 2026-05-31 BREAKTHROUGH — 0x0010 carries SmartPlant attribute/annotation text

Scanning `test-file/DWG-0202GP06-01.pid` for `0x0010` records and decoding
their payload as UTF-16LE (via `py_eval`) reveals the records carry **real
engineering attribute text**:

| Decoded string | Kind |
|---|---|
| `A3-FA060201` | instrument plant tag |
| `ODOIL020150 MM`, `ODOIL040150 MM` | line numbers |
| `DN80` | nominal diameter |
| `DWG-0202GP06-02` | drawing number |
| `i=0.3 %`, `272` | attribute values |
| `说明` (Chinese) | annotation text |

### Structure decoded

The record `ODOIL020150 MM` (14 chars, `btf=42`) matches `4+8+2+28 = 42`
exactly, giving:

```text
payload[0..4]   marker / type   (frequently 0x00010002)
payload[4..12]  8-byte header / aux
payload[12..14] u16 string length (char count)
payload[14..]   UTF-16LE string  (longer records carry more fields/strings)
```

### This REVISES insight #1 (H2) and resolves the Phase 19 puzzles

- `payload[0..4]` **repeats** (`0x00010002`) across many *distinct*
  records carrying *different* strings → it is an **attribute-type marker,
  NOT a unique oid**. So the earlier H2 ("payload[0..4] = the record's own
  oid") is **corrected**: it is a shared type/marker word, not a per-record
  identifier. `leading_word = payload[0..2] = 0x0002` is the low half of
  that marker — which is exactly why `0x0002` covers ~28% of records.
- The size-bucket heterogeneity that blocked Phase 19 (sizes 13/16/31/43/70
  heterogeneous at `+0`) is now **fully explained**: records carry
  **variable-length tags/text**, so both total size and the bytes at `+0`
  vary with string content.
- `0x0010` identity is refined from "persist container" to **attribute /
  annotation fragment** — matching the RTTI `tagAnnotPersistData` finding
  and Phase 16's "JStyleOverride references 0x0010 + plant tag".

### Engineering value + future-phase seed

Decoding `0x0010` strings yields P&ID **instrument tags, line numbers,
nominal sizes, and drawing references** — core semantic data currently not
extracted. This is a strong candidate for a future promotion phase:

- new typed decoder: `marker(4) + aux(8) + [u16 len + UTF-16LE]*`
- emit attribute/text evidence (tags/line-nos) with provenance
- still gate on byte-final IDA confirmation of the multi-string tail layout
  and on separating true attribute fragments from any non-attribute 0x0010
  variants
- baseline `582` stays until the new decoder lands with its own ratchet

### Cross-fixture validation (all 5 fixtures)

Whole-file `0x0010` scan + UTF-16LE decode across the fixture set confirms
the attribute-text payload is **robust, not a one-fixture fluke**:

| fixture | 0x0010 recs (whole-file scan) | sample extracted engineering strings |
|---|---|---|
| `工艺管道及仪表流程-1.pid` | 611 | line numbers `250-LNG-57602`, `50-LNG-58101`, `300-DF-58401`, `50-IA-58101`, `200-WD-58102`, `80-WD-58101` (standard `<size>-<service>-<seq>`) |
| `DWG-0201GP06-01.pid` | 351 | attribute labels `设计温度`/`操作温度`/`设备位号`, drawing `DWG-0201`, `污水外运` |
| `DWG-0202GP06-01.pid` | 115 | `A3-FA060201`, `DN80`, `ODOIL020150 MM`, `ODOIL040150 MM`, `DWG-0202GP06-02`, `i=0.3 %` |
| `D06.pid` | 20 | valve tag `BV01` |
| `A01.pid` | 11 | `#PH-010210` |

Observations:

- Line numbers follow the standard P&ID `<nominal>-<service>-<sequence>`
  convention (`250-LNG-57602` = DN250 LNG line 57602).
- Both English/numeric tags and CJK attribute labels decode cleanly.
- **Caveat:** these counts come from a *whole-file* byte scan, so they
  include CFB-metadata noise and differ from the Phase 18 Sheet-stream
  baseline (161/104/306/…/11). A production decoder MUST extract from the
  proper Sheet streams using the length-prefixed structure, not a raw
  whole-file scan, and must filter non-attribute `0x0010` variants. The
  true signal (tags / line-nos / sizes / labels) is clearly separable from
  the f64-region false positives.

## 2026-05-31 Phase 26 — attribute-fragment decoder LANDED (additive, audit-only)

The breakthrough is now implemented as a strict-additive, audit-only
decoder (Phase 26 goal package:
`goals/phase26-psm-0x0010-attribute-fragment-decoder/`).

- **Parser** (`src/parsers/sheet_records.rs`): `decode_attribute_fragments`
  / `decode_attribute_fragment_at` + `SheetAttributeFragmentDecoded` +
  `DecodedAttributeString`. Parses `marker(4) + aux(8) + [u16 len +
  UTF-16LE]*`; rejects records whose tail is not clean UTF-16LE (they stay
  with the raw Phase 18 decoder). 9 unit tests.
- **Model** (`src/model.rs`): `DecodedAttributeFragment` +
  `DecodedAttributeStringRecord` +
  `SheetGeometry::decoded_attribute_fragments`. Schema needles ratcheted.
- **Pipeline** (`src/streams/cluster.rs`): populated alongside the raw
  `decoded_sub_records_0x0010` collection (byte-for-byte unchanged).
- **Cross-fixture ratchet** (`tests/parse_real_files.rs`): per-fixture
  `DWG-0201=34 / DWG-0202=26 / 工艺管道-1=24 / A01=0`, total **84**
  fragments/strings; the raw Phase 18 baseline is asserted to remain
  exactly **582**.
- **Panic safety**: new decoder added to the adversarial matrix.

All 5 pre-commit gates green (build / test / clippy / fmt / missing-docs).
**No `PidGraphicKind` emission** — promotion remains a future phase.
**Slice B (aux 8-byte semantics)** is deferred: Slice A's `multi=0` result
proved the conservative single-string path is complete for the extraction
goal. Not committed (per the Phase 26 brief gates).
