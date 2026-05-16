# Phase 16 PSM 0x0030 = `JStyleOverride` — Final Summary

**Status**: Complete (Slice A → G, schema conflict §11 CLOSED).
**Date**: 2026-05-16
**Scope**: Re-identification of misnamed Phase 14 decoder, full IDA
reverse-engineering of `RAD style.dll!JStyleOverride`, additive
typed decoder + `PidGraphicKind::Annotation` variant + 98 cross-fixture
decoded entities, all 5 pre-commit gates green.

---

## 1. Mission

Phase 14 §6.1 future-slice flagged that `decode_primitive_arcs`
(PSM type `0x0030`) had "field-semantics caveats": bytes 32..63 were
suspected to contain packed integers rather than the assumed
`axis_ratio (f64) + sweep_direction (u8) + padding + sweep angles`.

Phase 16 takes the slice and turns it into a definitive answer:

1. Disprove the Phase 14 `GArc2d` hypothesis with byte-level evidence.
2. Reverse-engineer the COM dispatch chain back to the implementing DLL.
3. Decompile the implementation to obtain the authoritative on-disk
   schema.
4. Land a new decoder + DTO + emission path that preserves Phase 14
   baselines (strictly additive).

---

## 2. Result

### 2.1 Authoritative identity

`PSM type code 0x0030` is **`JStyleOverride`**, a Rapid Application
Development (RAD) 2D style class implemented in `style.dll`
(`FileVersion 07.00.00.0108`, `Product RAD`). It is **not** IGDS
`GArc2d`. CLSID `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`.

C++ inheritance: `JStyleOverride → JStyleR2d → JStyleBase`.

`SmartPlant P&ID` repurposes the RAD `JStyleOverride` class as an
**Annotation / tagged instrument placement** record — the
attribute tail commonly carries length-prefixed UTF-16LE plant
instrument tags (e.g. `"A3-FA060201"`), companion coordinates,
and PSM-style references to other records (`igTextBox`, `igSymbol2d`,
`igLineString2d`, `GraphicGroup`, `0x0010` sub-records).

### 2.2 Authoritative on-disk schema (Version 3, fixture path)

`style.dll!sub_1000F030` (JStyleOverride main vtable slot 52) writes
exactly 13 `IOContext::DoIO` calls totalling **64 bytes**, matching
the cross-fixture PSM payload exactly:

```text
disk +0..3   = 4B u32 (host this+22)
disk +4..7   = 4B u32 (host this+24)
disk +8..11  = 4B u32 (host this+25)
disk +12..15 = 4B u32 (host this+38)
disk +16..23 = 8B f64 (host this+26)
disk +24..31 = 8B f64 (host this+28) — rotation_angle candidate
disk +32..39 = 8B f64 (host this+30)
disk +40..47 = 8B f64 (host this+34)
disk +48..51 = 4B u32 (host this+32)
disk +52..55 = 4B u32 (host this+47)
disk +56..59 = 4B u32 (host this+48)
disk +60..61 = 2B u16 (host this+36)
disk +62..63 = 2B u16 (host byte+146)
```

### 2.3 The probe v5 vs IDA V3 schema conflict was illusory

probe v5 reads `+0..15` as 2 f64 anchor coordinates (always
normalized in `[0, 1]`), but IDA V3 reads the same bytes as 4 u32
fields. Both views are simultaneously correct:

| Layer | View of disk +0..15 |
|---|---|
| RAD `JStyleOverride::IJPersistImp` IO | 4 × u32 (untyped byte slots) |
| SmartPlant P&ID instrument placement | 2 × f64 (anchor.x, anchor.y) |

`JStyleOverride::Clone (sub_10010640)` uses
`qmemcpy(v5 + 22, this + 22, 0x58)` to copy 88 bytes as an
**untyped block**, proving RAD does not interpret field types.
`search_text "anchor"` returns 0 hits in `style.dll` — "anchor"
is a SmartPlant semantic, not a RAD semantic. SmartPlant uses RAD's
untyped byte slots as a `union { u32; f64 }` and writes IEEE 754
f64 values into them.

Phase 16 Slice F's design (`PidGraphicKind::Annotation` exposes the
SmartPlant f64 view as `anchor`; `SheetGeometry::decoded_jstyle_overrides`
preserves the RAD 4 u32 view) was correct on both abstraction layers.

### 2.4 Cross-fixture decoded counts (additive Phase 16 collection)

| Fixture | Phase 14 `decode_primitive_arcs` (misnamed) | Phase 16 `decode_jstyle_overrides` (authoritative) |
|---|---:|---:|
| `DWG-0201GP06-01.pid` | 8 | **20** |
| `DWG-0202GP06-01.pid` | 11 | **30** |
| `工艺管道及仪表流程-1.pid` | 28 | **47** |
| `export-test/publish-data/A01/A01.pid` | 1 | **1** |
| **Total** | **48** | **98** |

Phase 16 recovers **50 records** that Phase 14's spurious
`axis_a.y ≈ 0` constraint hard-rejected (those with rotation_angle
in `{π/2, 3π/2, 2π}`).

---

## 3. Reverse-Engineering Chain (5 IDA instances)

```text
Phase 14 GArc2d hypothesis (incorrect)
   ↓ ❌ Phase 14 §6.1 future-slice trigger
radsrvitem.dll!dword_5667B068 PSM type table[48]
   → CLSID {47FCC338-2D0F-11D0-A1FF-080036A1CF02}
   ↓
J2DSrv.dll → CONSUMER (sub_10001AB0 calls JCoCreateInstance)
   ↓
SmartSketch / RAD does not use Windows OLE registry;
   custom JCoCreateInstance dispatch imported from JUTIL.dll
   ↓
JUTIL.dll @ file offset 0x35680: RAD CLSID registry table
   (16B GUID + 16B implementing DLL name + 16B friendly name = 64B per entry)
   ↓
47FCC338 → style.dll : "JSL Override Style"
47FCC339 → style.dll : "JSL Offset Line Generator"
47FCC33A → style.dll : "JSL Bitmap Style"
   ↓
style.dll!DllGetClassObject (47FCC338 branch)
   → sub_10001600 → ClassFactory<JStyleOverride>::vftable @ 0x100697D4
   ↓
✅ Real C++ class = JStyleOverride
   Inheritance: JStyleOverride → JStyleR2d → JStyleBase
   ↓
JStyleBase::IJPersistImp::Save/Load (slots 5,6 = sub_10056DC0 thunk)
   → host vtable slot 32 = sub_10057B30 (version dispatcher)
   ↓
Version 3 path → JStyleOverride main vtable slot 52 = sub_1000F030
   ↓ 13 IOContext::DoIO calls = 64 bytes ✅ matches fixture
```

### 3.1 IDA address index

| Symbol | Address | Notes |
|---|---|---|
| `radsrvitem.dll PSM table` | `dword_5667B068` | 281 entries × 20B |
| `entry[48] = type 0x0030` | `dword_5667B068 + 960 = 0x5667B428` | CLSID 47FCC338 |
| `JUTIL.dll CLSID registry` | file offset `0x35680` | 64B / entry |
| `J2DSrv.dll CLSID xref` | RVA `0x100145F8` | consumer (sub_10001AB0) |
| `style.dll CLSID` | RVA `0x10066B64` | DllGetClassObject branch |
| `ClassFactory<JStyleOverride>::vftable` | `0x100697D4` | sub_10001600 |
| `JStyleOverride::vftable` | `0x1006A52C` | main, 50+ slots |
| `JStyleBase::vftable` | `0x1006E87C` | parent vtable |
| `JStyleBase::IJPersistImp::vftable` | `0x1006E9AC` | 16 slots, IJPersist thunk |
| `sub_10056DC0` | — | IJPersistImp slot 5,6 thunk → host slot 32 |
| `sub_10057B30` | — | host slot 32 = version dispatcher |
| `sub_1000F210` | — | JStyleOverride main vtable slot 17 = V2 IO (14 DoIO, 68B) |
| **`sub_1000F030`** | — | **JStyleOverride main vtable slot 52 = V3 IO (13 DoIO, 64B = fixture)** |
| `sub_10010640` | — | JStyleOverride::Clone (qmemcpy 88B untyped block) |

---

## 4. Implementation (strictly additive)

### 4.1 New types and functions

| Layer | File | New symbol |
|---|---|---|
| Parser | `src/parsers/sheet_records.rs` | `SheetJStyleOverrideDecoded`, `decode_jstyle_overrides`, `decode_jstyle_override_at`, `JSTYLE_OVERRIDE_PAYLOAD_LEN`, `JSTYLE_OVERRIDE_MIN_BYTES_TO_FOLLOW`, `JSTYLE_OVERRIDE_MAX_BYTES_TO_FOLLOW`, `JSTYLE_OVERRIDE_FIELD_DOMAIN_LIMIT` |
| Model | `src/model.rs` | `DecodedJStyleOverrideRecord`, `SheetGeometry::decoded_jstyle_overrides`, `SheetRecordKind::JStyleOverride` |
| Geometry | `src/geometry.rs` | `PidGraphicKind::Annotation { anchor: PidPoint, rotation_angle: f64, secondary_radius: f64, note: String }` + emission path |
| Pipeline | `src/streams/cluster.rs` | decoder integration |
| Pipeline | `src/cfb/reader.rs` | SheetGeometry init |
| Tests | `tests/parser_panic_safety.rs` | adversarial matrix entry |
| Tests | `tests/parse_real_files.rs` | `jstyle_override_decoder_emits_audit_records_with_provenance` + 3 updates to `normalized_geometry_probe_baseline_on_real_fixture` |
| Probe | `examples/probe_garc2d_packed_bytes.rs` | v5 final (btf bucket / cross-record OID ref / +24..31 rotation hypothesis) |
| Analysis | `docs/analysis/2026-05-15-garc2d-packed-int-tail.md` | 11 sections: 4 rounds of probe evidence + IDA chain |
| Analysis | `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md` | authoritative Version-3 schema + conflict-CLOSED §11 |

### 4.2 What was NOT changed (preserved Phase 14 baselines)

- `decode_primitive_arcs` / `SheetPrimitiveArcDecoded` / `DecodedPrimitiveArcRecord`
  remain untouched. Phase 14's misnamed family stays in place so existing
  downstream consumers (e.g. `geometry.rs::build_normalized_geometry`'s
  current Arc emission) are not broken.
- Cross-fixture counts: igLine2d=284, igLineString2d=119, igPoint2d=146,
  igTextBox=142, igSymbol2d=27, GLine2d=3, GArc2d=48 (Phase 14)
  and GraphicGroup=352 (Phase 15) all preserved verbatim.

A future Phase 17 will deprecate / rename the Phase 14 family once
downstream consumers migrate to the Phase 16 `decoded_jstyle_overrides`
collection and the `Annotation` variant.

### 4.3 Schema impact

`PidGraphicKind` gains a new variant `Annotation` and `SheetRecordKind`
gains a new variant `JStyleOverride`. Both are additive — existing
exhaustive matches over these enums **outside this crate** will need
to handle the new variants. Three in-crate exhaustive match sites in
`tests/parse_real_files.rs` were updated:

1. `normalized_geometry_inventory`: routes `Annotation` → `other_entities`.
2. `normalized_geometry_probe_baseline_on_real_fixture`: includes
   `decoded_jstyle_override_count` in the expected sum, adds a
   `decoded_annotations` bucket, includes `Annotation` in the
   Decoded-confidence typed whitelist.

---

## 5. Verification

All 5 pre-commit gates passed on the final commit (`43ce795`):

```powershell
cargo build --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs   # baseline = 0
```

Key cross-fixture test results:

| Test | Result |
|---|---|
| `jstyle_override_decoder_emits_audit_records_with_provenance` | ✅ 98 records (20+30+47+1) |
| `graphic_group_decoder_ratchets_fixture_counts_and_header_fields` | ✅ 352 records |
| `primitive_arc_decoder_emits_decoded_arcs_with_provenance` | ✅ 48 records (Phase 14 baseline preserved) |
| `iglines_decoder_emits_decoded_iglines_with_provenance` | ✅ 284 |
| `iglinestrings_decoder_emits_decoded_polylines_with_provenance` | ✅ 119 |
| `igpoints_decoder_emits_decoded_points_with_provenance` | ✅ 146 |
| `igtextboxes_decoder_emits_decoded_texts_with_provenance` | ✅ 142 |
| `igsymbols_decoder_emits_decoded_symbols_with_provenance` | ✅ 27 |
| `primitive_line_decoder_emits_decoded_lines_with_provenance` | ✅ 3 |
| `normalized_geometry_probe_baseline_on_real_fixture` | ✅ all buckets balanced |

---

## 6. Commit Log

| Commit | Description |
|---|---|
| `adb9039` | docs + evidence + goal-packages (Phase 15 + Phase 16) |
| `33927a3` | parsers + model + geometry implementation (1230 +, 22 −) |
| `43ce795` | docs: schema conflict §11 CLOSED |

Not yet pushed (per `AGENTS.md` policy — requires explicit user authorization).

---

## 7. Future Work

| Item | Phase | Notes |
|---|---|---|
| Deprecate / rename Phase 14 `decode_primitive_arcs` family | Phase 17 | Breaking schema change — needs downstream consumer migration plan |
| Decode `0x0010` sub-record family (638 cross-fixture hits) | Phase 18 | Referenced from JStyleOverride `+56..59`, GraphicGroup tail; likely embedded attribute fragment |
| Reverse other RAD `47FCC330..47FCC33E` siblings (PSM type 0x29..0x35) | Phase 19 | Same `JStyleOverride` family but different IDs; potentially other annotation kinds |
| Reverse JStyleOverride V2 path (`sub_1000F210`, 68B) usage in non-fixture data | — | Currently unused by SmartPlant fixtures; only needed if newer / older SmartPlant versions ship V2 streams |
| Plant instrument tag extraction from `raw_attribute_tail` | — | Tag is length-prefixed UTF-16LE at conditional offset; needs tail-layout decoder |

---

## 8. Acknowledgments

Phase 16 was driven through 12+ hours of reverse-engineering across:

- **5 IDA Pro instances**: radsrvitem.dll / J2DSrv.dll / JUTIL.dll
  (PowerShell static decode) / style.dll / AVEVA core.dll
- **4 rounds of probe iteration** (`probe_garc2d_packed_bytes.rs` v1 → v5)
- **2 analysis documents** (11 sections + 10 sections)
- **Strict-additive implementation** preserving all Phase 14/15 baselines

The session spanned 2026-05-15 13:38 → 2026-05-16 11:55 UTC+8, with
the final IDA pivot (`qmemcpy` evidence) closing the last open
question (`+0..15` schema interpretation) on the morning of
2026-05-16.
