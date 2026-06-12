# Phase 29 Slice B Follow-Up: StyleCluster Leftover Triage

> Generated from
> `cargo run --example probe_phase29_psmcluster0_body_triage -- StyleCluster`
> (run 2026-06-10). Investigation-only report; record semantics are not
> promoted. Companion to
> `2026-06-08-phase29-psmcluster0-leftover-triage.md`.

## Question

After `/PSMcluster0` was fully accounted, `/StyleCluster` became the next
cluster-family leftover hotspot (83,468 leftover bytes across 6 fixtures,
header-only coverage ratio 0.0011). Does its body reuse the PSM record
chain discovered in `/PSMcluster0`?

## Header And Leftover Summary

| Fixture | Stream bytes | `record_count` | `stream_type` | `body_len` | `flags` | Leftover bytes (pre-walker) |
|---|---:|---:|---:|---:|---:|---:|
| `d06` | 6,395 | 47 | `0x005A` | 2,368 | `0x2000` | 6,379 |
| `nonascii-process-1` | 10,127 | 84 | `0x005A` | 3,230 | `0x2000` | 10,111 |
| `dwg0201` | 17,097 | 135 | `0x005A` | 5,044 | `0x2000` | 17,081 |
| `dwg0202` | 22,121 | 180 | `0x005A` | 6,486 | `0x2000` | 22,105 |
| `publish-a01` | 5,703 | 42 | `0x005A` | 2,034 | `0x2000` | 5,687 |
| `publish-dwg0202` | 22,121 | 180 | `0x005A` | 6,486 | `0x2000` | 22,105 |

All fixtures share `stream_type = 0x005A` and `flags = 0x2000`;
`body_len` varies per fixture (unlike `/PSMcluster0`, where it is the
constant 113) and does not equal the chain start offset — its semantics
remain unknown.

## Finding 1: The Tail Is A Single End-Anchored PSM Record Chain

The min-chain-3 resync scan finds exactly **one** chain per fixture, and
that chain runs to end-of-stream with zero internal resync:

| Fixture | Chain start | Records | Chain bytes | Fraction of leftover | `record_count` |
|---|---:|---:|---:|---:|---:|
| `d06` | 2,376 | 47 | 4,019 | 0.6300 | 47 |
| `nonascii-process-1` | 1,545 | 77 | 8,582 | 0.8488 | 84 |
| `dwg0201` | 1,899 | 76 | 15,198 | 0.8898 | 135 |
| `dwg0202` | 2,267 | 78 | 19,854 | 0.8982 | 180 |
| `publish-a01` | 2,042 | 42 | 3,661 | 0.6437 | 42 |
| `publish-dwg0202` | 2,267 | 78 | 19,854 | 0.8982 | 180 |

**`record_count` caveat**: the chain length equals `header.record_count`
on only 2 of 6 fixtures (`d06` 47 = 47, `publish-a01` 42 = 42). The
other four fixtures carry larger `record_count` values (84/135/180 vs
77/76/78 chain records). Whatever the field counts there, it is not just
the tail chain — so the `/PSMcluster0`-style envelope corroboration does
**not** transfer, and StyleCluster records stay fully audit-only
(`Probed`), envelope included.

## Finding 2: The Prefix Is GUID-Table-Like, Unparsed

Between the 16-byte header and the chain start sits a variable-length
prefix (1,529–2,360 bytes) that opens identically on all 6 fixtures:

- ten `0x00` bytes (same width as the `/PSMcluster0` prologue, but
  all-zero instead of `00 00 01 00 …`);
- a `u16`-shaped `0D 00` (13);
- consecutive 16-byte CLSID-shaped entries
  (`… D0 11 B2 9B 08 00 36 22 D7 02`, `… CE 11 95 CA 08 00 36 48 39 02`,
  `… D0 11 A1 E1 08 00 …` — classic COM IID byte patterns);
- further uncharacterized structure up to the chain start.

This region is intentionally **left as leftover** (no `Probed` claim)
until a dedicated slice characterizes it.

## Finding 3: Type Codes And Payload Texture

Chain type histograms are dominated by `0x002C` / `0x002D` / `0x002E`
with a recurring minor set (`0x0018`, `0x001B`, `0x0020`, `0x002A`,
`0x002B`, `0x002F`, `0x0030`, `0x0032`, `0x0033`, `0x007B`). Payload
text runs show style-catalog vocabulary: `Normal`, `Viewport`,
`Office Automation`, `As Drawn`, `Nozzle - New`, font names
(`Arial Narrow`), GB2312-range glyphs, and dotted style paths
(`…ow.6`, `…12.6`).

**Namespace warning (stronger than PSMcluster0)**: several walked codes
collide numerically with Sheet/IGDS names (`0x0018 igLine2d`,
`0x0020 igRectangle2d`, `0x0030 JStyleOverride`, `0x007B igGroup`) in a
stream that plainly stores styles, not geometry. Do not map StyleCluster
type codes through any existing table without IDA evidence.

## Decision

Audit-only walker with an **end-anchored earliest-chain locator**
(parser-only, no semantics):

- locator: earliest offset after the header from which a strict chain of
  ≥ 3 records runs exactly to end-of-stream;
- records claimed entirely as `Probed` (envelope included — see the
  `record_count` caveat);
- prefix stays leftover;
- IDA target request: `stream_type 0x005A` reader, the GUID-table prefix
  layout, the `record_count` semantics, and names for `0x002C`–`0x002E`.

## Implementation Status (2026-06-10, same session)

Shipped as `src/parsers/cluster_header.rs::{decode_style_cluster_body_records,
parse_style_cluster_with_trace}` on the shared
`ClusterBodyRecordDecoded` / `decode_cluster_body_records` core (the
`/PSMcluster0` walker was renamed onto the shared names in the same
change; `decode_psm_cluster0_body_records` keeps its stream-specific
entry point).

Test results:

- 6 new unit tests (earliest-chain locator, end-anchor gate, min-records
  gate, headerless reject, trace Probed accounting, no-chain fallback) —
  pass; `parsers::cluster_header` suite now 22 tests.
- `tests/parser_panic_safety.rs` entry for
  `decode_style_cluster_body_records` — pass.
- Cross-fixture ratchet
  (`tests/parse_real_files.rs::stylecluster_body_chain_is_end_anchored_across_fixtures`):
  6/6 fixtures decode end-anchored chains at exactly the probe offsets;
  consumed ratios 0.631–0.898.
- 6-fixture snapshots regenerated: `/StyleCluster` family leftover drops
  from 83,468 to 12,300 bytes (consumed 96 → 71,264); whole-file ratios
  rise to 0.226–0.626.

Remaining `/StyleCluster` leftover = the GUID-table-like prefix only.
