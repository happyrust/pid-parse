# PSM `0x0089` Dynamic-Attributes record trailer — cross-fixture validation

> Date: 2026-06-30
> Scope: pid-session backlog item "0x0089 DA head". Fixture-side cross-fixture
> validation of the documented `DaRecordTrailer` layout (`src/model.rs`
> L613–627, previously *"verified against two real-world samples"*) using a
> standalone Python CFB reader (FAT + mini-FAT — the Rust release build is
> blocked this session by an MSVC `LNK1318` PDB linker error). Status: evidence
> note only. No parser / DTO / schema / writer / byte-audit / confidence
> change.

## Method

Standalone CFB reader extracts `/Unclustered Dynamic Attributes` from all 5
local fixtures, scans `0x89 0x00` markers, parses the documented 31-byte
trailer at each marker, and checks each field. (The `+22 == 0xFFFF` and
`+10..+18 == 0` constraints corroborate that the `0x89 0x00` hits are real
trailers, not coincidental byte pairs.)

## Documented layout under test (`src/model.rs`)

```text
+0   u16    marker = 0x0089
+2   u32    size
+6   u32    record_id
+10  [u8;8] pad (zero)
+18  u32    field_x
+22  u16    separator = 0xFFFF
+24  u32    class_id
+28  [u8;3] tail = 0x14 0x00 0x00
```

## Result (5 fixtures)

| fixture | DA bytes | `0x89 00` markers | `sep==0xFFFF` | `pad8==0` | `tail==14 00 00` |
|---|---:|---:|---:|---:|---:|
| dwg0201 | 49,821 | 231 | 231 | 231 | 160 |
| dwg0202 | 34,364 | 170 | 169 | 169 | 103 |
| process1 | 11,989 | 68 | 68 | 68 | 31 |
| d06 | 8,812 | 47 | 47 | 47 | 26 |
| a01 | 395 (mini-stream) | 4 | 4 | 4 | 0 |

### Findings

1. **Trailer head confirmed cross-fixture.** `separator == 0xFFFF` and the
   8-byte pad `+10..+18 == 0` hold for **519/520 markers (99.8%)** across 5
   fixtures (a single exception in dwg0202) — the head
   `marker | size | record_id | pad8 | field_x | sep=0xFFFF | class_id` is a
   stable structure. This upgrades the prior "2 samples" claim to genuine
   cross-fixture coverage.
2. **`+28 tail = 0x14 0x00 0x00` is NOT an invariant** — only **320/520
   (~62%)** markers carry it (160/231, 103/169, 31/68, 26/47, 0/4). The stable
   trailer is therefore `+0..+28` (28 bytes, through `class_id`); byte `+28`
   onward is **class/variant-specific, not constant padding**. The `model.rs`
   doc comment over-claims it from 2 samples and should be softened in a future
   change. (The `DaRecordTrailer` DTO does **not** expose a tail field, so
   parsed JSON output is unaffected — this is a comment/accuracy issue only.)
3. **`class_id` is a wide enum (~20+ values).** Observed: `0xF6` Relationship,
   `0xEA` Drawing, `0x109` Symbol/Nozzle (the documented named set) **plus**
   `0xAC, 0xE1, 0xE7, 0xE9, 0xED, 0x100, 0x101, 0x103, 0x104, 0x106, 0x108,
   0x10A, 0x10B, 0x10C, 0x10D, 0x10F, 0x111`. The named set is a small subset;
   most `class_id`s remain unnamed (their names live in the DA Metadata schema,
   not here).
4. **Common header record across all fixtures.** Every fixture's first trailer
   is identical-shape: `record_id=0x6002, field_x=4, class_id=0xAC, size=192` —
   a per-drawing header/sentinel record.
5. A01's DA stream is a 395-byte **CFB mini-stream** (now readable): 4 markers,
   `sep`/`pad` valid but `tail != 14 00 00`, consistent with finding 2.

## Mode A (no new asm needed)

Phase 30-C already established the `radsrvitem.dll` boundary for `0x0089`:
`sub_5644B640` filters runtime records by `*record == 137` (`0x89`) and writes
only `RAD_OBJECT_TYPE = "137"`. Unlike `0x00FA` (250, which is out of the
type-name mapper's code range 6..0xCE), `0x0089` (137) is *in* range but takes
the mapper's **default** path (`sub_56448F70` → `sub_564462F0`) — no geometry
class name, no DA-head field decode. So `0x0089` has no named typed reader in
`radsrvitem.dll`, consistent with the cross-fixture head being readable while
the `class_id` semantics live in `/Dynamic Attributes Metadata`, not the
geometry server.

## Conclusion (stays audit/probe, docs-only)

- Trailer head `+0..+28` confirmed across 5 fixtures (`sep=0xFFFF` + `pad8=0`
  at 99.8%); `+28 tail` is variant, not constant — a correction to the 2-sample
  doc.
- No promotion: the `class_id` enum is mostly unnamed; naming needs the
  `/Dynamic Attributes Metadata` schema decode or IDA. No parser / DTO /
  byte-audit / confidence change here.

## Re-open / next

- Decode `/Dynamic Attributes Metadata` to map `class_id → class name` (turns
  the ~20 unnamed ids into names; pairs directly with this payload trailer).
- Future doc fix (separate, authorized change): soften the `model.rs`
  `DaRecordTrailer` comment — `+28 tail` is ~62% (not constant), and the layout
  is now "verified across 5 fixtures".

## Anchors

- `src/model.rs` `DaRecordTrailer` L613–656; parser
  `src/parsers/dynamic_attr_records.rs`.
- Phase 30-C: `docs/analysis/2026-06-12-phase30-radsrvitem-record-spacemap-ida.md`.
- Phase 29-I: `docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`.
