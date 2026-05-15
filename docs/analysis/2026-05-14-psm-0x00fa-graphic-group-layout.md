# PSM 0x00FA GraphicGroup / GraphicPersist Layout Notes

**Date**: 2026-05-14  
**Scope**: Phase 15 Slice A/B pre-decoder evidence for PSM `0x00FA`
records in SmartPlant `Sheet*` streams.

## Status

This is layout evidence, not a stable decoder contract yet.

`examples/probe_psm_0x00fa_shape.rs` was extended to print:

- bounded `0x00FA` hit counts per fixture
- `bytes_to_follow` distribution
- payload `sub_type_word` distribution
- `parent_ref` distribution
- nearby known PSM geometry records
- candidate OID words and whether they match known decoded geometry OIDs

Command:

```powershell
cargo run --release --example probe_psm_0x00fa_shape
```

Result: exit 0.

## Cross-Fixture Counts

Current broad bounded PSM-header scan reports:

| Fixture | Sheet | `0x00FA` candidates |
|---|---:|---:|
| `test-file/DWG-0201GP06-01.pid` | `/Sheet6` | 136 |
| `test-file/DWG-0202GP06-01.pid` | `/Sheet6` | 84 |
| `test-file/工艺管道及仪表流程-1.pid` | `/Sheet6` | 125 |
| `test-file/export-test/publish-data/A01/A01.pid` | `/Sheet6` | 8 |
| **Total** | | **353** |

This revises the earlier Phase 14 Slice Q summary of 250 cross-fixture
hits. Do **not** update stable docs yet: first determine whether the
delta is a counting-method difference, a fixture-set difference, or
wide-scan false positives that need an additional validation rule.

## Phase 15 Slice C Count Reconciliation

The first parser-level ratchet in
`tests/parse_real_files.rs::graphic_group_decoder_ratchets_fixture_counts_and_header_fields`
locks the conservative decoder output at **352** records across the same
four fixtures:

| Fixture | Broad bounded probe | Conservative decoder |
|---|---:|---:|
| `test-file/DWG-0201GP06-01.pid` | 136 | 135 |
| `test-file/DWG-0202GP06-01.pid` | 84 | 84 |
| `test-file/工艺管道及仪表流程-1.pid` | 125 | 125 |
| `test-file/export-test/publish-data/A01/A01.pid` | 8 | 8 |
| **Total** | **353** | **352** |

The one-record reduction is intentional: the decoder applies additional
header validation beyond the broad probe scan. This makes **352** the
current parser-level ratchet, while **353** remains the broad evidence
count.

The older Phase 14 Slice Q count of **250** is not yet reproducible from
the current probe source because that summary did not preserve the exact
fixture set and filtering rule. Treat it as a legacy discovery count, not
as the Slice C acceptance ratchet. The most likely explanation is a
counting-method or fixture-set difference from the earlier probe pass,
but the exact source should not be asserted until the old command/output
is recovered.

## Stable Header Evidence

Across sampled hits:

```text
PSM header:
  0..1   u16   type word, low 14 bits = 0x00FA
  2..5   u32   bytes_to_follow

Payload:
  0..3   u32   oid
  4..7   u32   parent_ref
  8..13  6 bytes zero in sampled records
  14..15 u16   small kind/count-like value (observed 1, 2, 4)
  16..17 u16   sub_type_word / version-like discriminator
  18..   variable tail
```

`parent_ref` was `6` for every candidate in all four fixtures:

```text
DWG-0201: parent_ref 6 => 136 hits
DWG-0202: parent_ref 6 => 84 hits
工艺管道及仪表流程-1: parent_ref 6 => 125 hits
A01: parent_ref 6 => 8 hits
```

This makes `parent_ref == 6` a strong candidate validation rule for the
current fixture family, but not yet a universal SmartPlant invariant.

## Size Buckets

The most common `bytes_to_follow` buckets are:

| Fixture | Top buckets |
|---|---|
| DWG-0201 | 66 x64, 154 x16, 104 x12, 170 x11, 44 x10, 54 x8 |
| DWG-0202 | 66 x27, 104 x13, 122 x9, 154 x9, 44 x4, 98 x4, 110 x4 |
| 工艺管道及仪表流程-1 | 66 x34, 104 x19, 98 x12, 122 x12, 54 x10, 44 x9 |
| A01 | 66 x3, 104 x2, 44 x1, 54 x1, 170 x1 |

The repeated buckets support a bounded decoder with an allowlist or
range guard, but the range alone is not enough to prove semantics.

## Candidate OID Patterns

The variable tail often contains u32 words that match decoded geometry
OIDs from nearby records.

Examples:

```text
DWG-0201 hit @ 0x00026a:
  oid=105 parent_ref=6 sub_type=0x0044 bytes_to_follow=104
  prev igTextBox oid=97
  next igSymbol2d oid=111
  payload +034 = 97 (geometry_oid)
```

```text
DWG-0201 hit @ 0x000357:
  oid=138 parent_ref=6 sub_type=0x0146 bytes_to_follow=54
  prev igSymbol2d oid=111
  next igSymbol2d oid=139
  payload +026 = 35 (geometry_oid)
```

```text
工艺管道及仪表流程-1 hit @ 0x000431:
  oid=6326 parent_ref=6 sub_type=0x1C7E bytes_to_follow=164
  prev igLine2d oid=6325
  next igLine2d oid=6327
  payload candidate geometry OIDs at +034, +046, +058, +070, +082, +094
```

```text
A01 hit @ 0x000044:
  oid=42 parent_ref=6 sub_type=0x0033 bytes_to_follow=66
  next igPoint2d oid=43
  payload +022 = 184 (geometry_oid)
  payload +034 = 43 (geometry_oid)
```

The evidence supports the working hypothesis that the tail stores one or
more references to geometry OIDs, but the encoding is not yet a simple
flat `u32[]`:

- candidate offsets differ by size bucket and sub-type
- some words are flags/sentinels (`0x00010000`, `0x0000FBFF`,
  `0x00000077`, `0x00000007`)
- large buckets contain both OID-like words and f64-looking trailing
  values

## Candidate OID Bucket Audit

`examples/probe_psm_0x00fa_shape.rs` now prints an audit-only
`bucket geometry-OID candidate summary` grouped by
`(bytes_to_follow, sub_type_word)`.

Observed repeated patterns:

| Bucket pattern | Candidate offset evidence |
|---|---|
| `btf=54` records | geometry OID candidates concentrate at payload `+026` |
| `btf=66` records | candidates usually appear at `+034`, with some buckets also using `+022` |
| `btf=104` records | candidates commonly appear at `+034` |
| `btf=122` records | repeated candidates appear at `+046`, `+058`, `+078`, sometimes `+034` |
| `btf=154` records | repeated candidates appear around `+050`, with optional earlier words |
| `btf=44` records | often contain no known geometry OID candidate |

This supports an audit helper that reports candidate offsets per
size/sub-type bucket, but it is still not strong enough for a stable
`child_oids` field. The offset rules vary by bucket and still mix real
geometry OIDs with flags, sentinels, and scalar-looking payload tails.

## Decoder Implications

Safe first decoder contract:

- accept only `type_code == 0x00FA`
- require bounded payload length
- expose `byte_range`, `type_flags`, `bytes_to_follow`, `oid`,
  `parent_ref`, small payload word at `14..15`, `sub_type_word`, and raw
  tail bytes from `18..`
- surface decoded records through `SheetGeometry::decoded_graphic_groups`
  as an audit-only collection; do not emit normalized `PidGraphicEntity`
  geometry from these records
- keep candidate OID extraction as audit/probe output until validated
  per size/sub-type bucket

Do not expose a stable `child_oids` field yet.

## Rejected Interpretations For Now

- `0x00FA` is not an inline attribute tail after each geometry record.
  Phase 14's `probe_igline2d_attribute_tail` showed geometry records are
  densely packed.
- The tail is not proven to be style/color/layer data.
- PSM `0x0010` should not be decoded as part of this goal; it still
  appears to be an embedded fragment family.
- The current 353 broad-scan count should not replace the Phase 14
  summary count until a decoder validation rule explains the difference.

## Next Step

Slice C should implement a conservative `SheetGraphicGroupDecoded` DTO
and parser that captures the stable header plus raw tail. Then a
separate fixture test can ratchet the count and determine whether
candidate child OID extraction is stable enough for an audit-only helper.
