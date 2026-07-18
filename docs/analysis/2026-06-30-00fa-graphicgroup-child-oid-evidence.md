# PSM `0x00FA` GraphicGroup — child-OID-array hypothesis test + mapper evidence

> Date: 2026-06-30
> Scope: read-only evidence pass for the `pid-session` backlog item
> "0x00FA GraphicGroup payload". Fixture-side hypothesis test (standalone
> Python CFB reader — the Rust release build is blocked this session by an
> MSVC `LNK1318` PDB linker error) + Mode A static `.asm` check of
> `dlls/radsrvitem.dll`. Status: evidence note only. No parser / DTO / schema
> / writer / byte-audit / ratchet / confidence change. `0x00FA` stays
> `TypedAudit` (Phase 15 contract).

## Question

Phase 15 decoded the `0x00FA` header (`oid` / `parent_ref` / `group_kind_word`
/ `sub_type_word`) + raw tail from byte 18 but deliberately did **not** name a
`child_oids` field, because the geometry-OID candidate offsets are
bucket-specific. This pass tests the most natural promotion hypothesis: *is
the tail a count-prefixed contiguous child-OID array?*

## Fixture-side test (5 local fixtures)

Tool: standalone CFB reader extracting `/Sheet6`, loose PSM scan, geometry-OID
set = OIDs of records typed `{GLine2d, GArc2d, igLine2d, igLineString2d,
igPoint2d, igTextBox, igSymbol2d}` (`oid > 16`). For every `0x00FA` record,
search all payload offsets for `u32 count C` (`2..200`) followed by `C` ×
`u32` that **all** resolve to a geometry OID, preferring `arr_end` at/near the
record end.

| fixture | `/Sheet6` | `0x00FA` | count+C geom-OIDs (exact) | loose all-OID | tail-anchored |
|---|---:|---:|---:|---:|---:|
| DWG-0201 | 29,594 | 136 | **0** | 11 (`+35`, C=3) | 0 |
| DWG-0202 | 23,731 | 84 | **0** | 3 (`+35`/`+53`, C=3) | 0 |
| 工艺管道-1 | 46,540 | 125 | **0** | 0 | 0 |
| D06 | 4,122 | 21 | **0** | 0 | 0 |
| A01 | mini-stream | — | — | — | — |

**Result: the count-prefixed child-OID-array hypothesis is false** — 0 of 366
`0x00FA` records (across 4 readable fixtures) carry a `[count][C × geometry
OID]` block. The only hits are a loose "all `u32` resolve to *some* record
OID" variant: count always 3, only in the DWG family, never tail-anchored —
i.e. coincidental 3-tuples, not a list. (A01's `/Sheet6` is a CFB
*mini-stream* < 4 KiB, skipped by the FAT-only reader; it holds only ~8
groups.)

This matches and hardens the Phase 15 finding: the geometry OIDs in the tail
sit at **`sub_type`/size-specific offsets** (`btf=66→+34`, `btf=104→+34`,
`btf=122→+46/+58/+78`, `btf=154→+50`) — embedded references inside a
per-`sub_type` *fixed-variant* record, **not** a generic length-prefixed list.

## Mode A — `radsrvitem.dll` type-name mapper (`sub_56448F70`)

The type→name mapper reads the record type word and switches
(`L12487–12497`):

```
movzx eax,[rec]            ; type code
cmp   eax,115h  (jg/jz special)
sub   eax,6                ; switch base = 6
cmp   eax,0C8h
ja    default              ; named classes only for code-6 in [0,0xC8]
movzx eax, byte_56449170[eax]
jmp   jpt_56448FA4[eax*4]
```

So named RAD classes occupy **codes `6..0x0CE` only** (`igLine2d`=case 24,
`igArc2d`=case 97, `igEllipticalArc2d`=case 126, `igGroup`=case 123, … — the
"case" is a compact `byte_56449170` jumptable index, not the raw code).

- **`0x00FA` (250) is out of switch range** (`250-6 = 244 > 0xC8`) → it falls
  to the **default case**: `radsrvitem.dll` assigns `0x00FA` **no RAD class
  name**. Neither `GraphicGroup` nor `GraphicPersist` strings exist in the
  binary; `igGroup` exists but is a different, lower code (≤ `0xCE`).
- Cross-check: the same mapper **special-cases `0x115`** (`cmp eax,115h; jz`)
  — that is the `0x0010` root alias confirmed in the Phase 33 work, an
  independent consistency check of this mapper.

So `0x00FA` has no dedicated named geometry reader in `radsrvitem.dll`; like
`0x0010` it is deserialized through the generic per-type `vtable+0x18` path
(see `2026-06-30-phase33-0010-discriminator-mode-a-asm-evidence.md`),
`bytes_to_follow`-bounded.

## Conclusion (stays `TypedAudit`, docs-only)

- The `0x00FA` tail is **not** a count-prefixed child-OID list (disproven
  across 4 readable fixtures, 0/366 records).
- `0x00FA` is **unnamed** in the radsrvitem type mapper (default case), so
  there is no in-binary typed reader to mine for a fixed `child_oids` layout.
- Keep the Phase 15 contract: header + raw tail, no `child_oids`, no
  `PidGraphicKind` / geometry emission / byte-audit / confidence change.

## Re-open trigger

- Mode B: open `dlls\radsrvitem.dll.i64`, find the `0x00FA` deserialize
  handler (the object whose `vtable+0x18` Read consumes the group body) and
  lay out the per-`sub_type` variant fields; only then can the embedded
  geometry references be named as members.
- Or a controlled fixture (add / remove one group member, diff the tail) to
  prove which bytes are the member list per `sub_type`.

## Anchors

- mapper `sub_56448F70` `L12481`; range gate `L12490–12495`; `igGroup` name
  `aIggroup` `L838967` (jumptable case 123).
- fixture probe: `count + C×u32` geometry-OID test, 0 exact matches across
  DWG-0201 / DWG-0202 / 工艺管道-1 / D06.

## References

- `docs/plans/2026-05-14-phase15-graphic-group-final-summary.md`
- `docs/analysis/2026-05-14-psm-0x00fa-graphic-group-layout.md`
- `docs/analysis/2026-06-30-phase33-0010-discriminator-mode-a-asm-evidence.md`
- `examples/probe_psm_0x00fa_shape.rs`
