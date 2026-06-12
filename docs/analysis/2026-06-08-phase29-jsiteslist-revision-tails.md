# Phase 29 Slice M: JSitesList / Revision Unregistered Tail Closeout

> Evidence from `cargo run --release --example
> probe_phase29_unregistered_tails` over the 6 local `.pid` fixtures
> (2026-06-12 session). This slice closes the last multi-fixture
> common-unregistered top-level paths that did not require IDA.

## Question

After Phase 29 Slices A–L, the only top-level paths still appearing in
the common-unregistered list across multiple fixtures were
`/JSitesList` (6/6) and `/TaggedTxtData/Revision` (5/6). Are they
parseable with bounded evidence, or gated like `PSMspacemap`?

## Finding 1: `/JSitesList` Is An "OLEM" Slot Table Of JSite Ids

Layout (6/6 fixtures):

```text
+0..4   "OLEM" magic (4F 4C 45 4D)
+4..8   u32 LE count
+8..    4-byte-aligned u32 LE slot table
```

| Fixture | Bytes | count | Slots | Trailing slots | Entries matching `JSite<id>` storages |
|---|---:|---:|---:|---:|---:|
| `d06` | 44 | 9 | 9 | 0 | 9/9 |
| `nonascii-process-1` | 48 | 10 | 10 | 0 | 10/10 |
| `dwg0201` | 88 | 20 | 20 | 0 | 20/20 |
| `dwg0202` | 72 | 13 | 16 | 3 | 13/13 |
| `publish-a01` | 28 | 5 | 5 | 0 | 5/5 |
| `publish-dwg0202` | 72 | 13 | 16 | 3 | 13/13 |

- **Every logical entry (the first `count` slots) matches an existing
  `JSite<id>` storage in the same package, on every fixture** — the
  stream is the directory of JSite symbol-instance storages.
- `dwg0202` / `publish-dwg0202` carry 3 trailing slots beyond `count`
  that repeat logical values (`793, 4458, 4458`): a stale,
  non-truncated tail. The first ratchet attempt with an exact-size gate
  failed on exactly these fixtures, which is what surfaced the
  count-vs-slot distinction.

## Finding 2: `/TaggedTxtData/Revision` Is A 0-Byte Placeholder

The stream exists on 5 fixtures and is empty (0 bytes) on all of them.
It contributes no leftover bytes; its unregistered status was pure
inventory noise.

## Implementation (same session)

- `src/parsers/jsites_list.rs`: `parse_jsites_list` /
  `parse_jsites_list_with_trace` + `JSitesListDecoded { count,
  entries, trailing_slots }`. Gate: magic, count cap, 4-byte-aligned
  body, `len >= 8 + 4 * count`. Trace: header `[0..8)` `Decoded`,
  logical table `Probed`, **stale trailing slots stay leftover**.
  8 unit tests; panic-safety entry.
- `src/byte_audit/aggregate.rs`: top-level `/JSitesList` branch +
  nested `JSite*/JSitesList` dispatch (Slice L registry table) +
  `/TaggedTxtData/Revision` registered as `revision_empty_stream`
  (zero claims; future non-empty content surfaces as leftover under a
  registered path).
- Ratchet `jsites_list_parses_with_exact_size_and_matches_jsite_storages`:
  counts {9, 10, 20, 13, 5, 13}, trailing {0, 0, 0, 3, 0, 3},
  storage matches == count on 6/6, byte-audit leftover == 4 × trailing.
- Field naming guardrail: the DTO field is `entries`, not `jsite_ids`;
  the storage correlation is recorded as evidence pending writer-side
  IDA confirmation. `trailing_slots` describes position, not
  semantics.

## Snapshot Effect

- Common unregistered list loses `/JSitesList` (×6),
  `/TaggedTxtData/Revision` (×5), and nested `JSite*/JSitesList`
  entries; distinct unregistered paths drop 51 → 38.
- Unregistered per fixture: 15/14/15/12/19/12 → **11/11/11/9/14/9**.
- Whole-file ratios: d06 0.7858 / nonascii 0.8260 / dwg0201 0.8882 /
  dwg0202 0.8519 / publish-a01 0.6652 / publish-dwg0202 0.8519.
- Remaining multi-fixture unregistered paths are all gated:
  `PSMspacemap` pages (IDA), `JSite*/\x01Ole` OLE payloads (demand).

## IDA Target Request (secondary)

- The `"OLEM"` writer and the meaning of the count-vs-slot mismatch
  (stale tail) on `dwg0202`-family fixtures.
- Confirmation that slot values are the `JSite<id>` storage ids
  (would let `entries` be renamed and surfaced in the model layer).
