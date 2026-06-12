# Phase 29 Slice C: Unclustered Dynamic Attributes Body Backlog

> Generated from `cargo run --release --example probe_phase29_da_body_triage`
> over the 6 local `.pid` fixtures (2026-06-11 session). Investigation-only:
> no parser semantics are promoted by this document. Candidate slice source:
> `docs/specs/2026-06-08-pid-file-format-spec-kit/phase29-candidate-slices.md`
> (Slice 29-C).

## Question

`/Unclustered Dynamic Attributes` carries the second-largest non-JSite
leftover family (111,120 bytes across 6 fixtures, ratio 0.2233). The
byte-audit currently claims only three landmark families (`P&IDAttributes`
class-name runs, the 31-byte record-trailer signature, `DrawingID\0` +
32-hex). Is the remaining body:

1. an unknown format that needs new reverse engineering,
2. a record chain matching the cluster-family envelope already proven for
   `/PSMcluster0` and `/StyleCluster`, or
3. opaque payload that should wait for IDA?

## Stream And Landmark Summary

| Fixture | Stream bytes | Leftover bytes | Leftover ranges | Trailers | Heuristic records | `0x89 0x00` markers |
|---|---:|---:|---:|---:|---:|---:|
| `d06` | 8,812 | 7,001 | 43 | 25 | 47 | 47 |
| `nonascii-process-1` | 11,989 | 9,323 | 63 | 30 | 68 | 68 |
| `dwg0201` | 49,821 | 38,662 | 256 | 159 | 231 | 231 |
| `dwg0202` | 34,364 | 26,568 | 180 | 102 | 169 | 170 |
| `publish-a01` | 3,711 | 2,998 | 18 | 9 | 22 | 22 |
| `publish-dwg0202` | 34,364 | 26,568 | 180 | 102 | 169 | 170 |

## Finding 1: The Body Is A Single End-Anchored `0x0089` Envelope Chain

A strict PSM-envelope walk (`u16 type_word` + `u32 bytes_to_follow`, zero
resync) starting at offset 8 reaches exactly the end of the stream on
**6/6 fixtures**:

| Fixture | Chain start | Records | Covered bytes | Stream ratio | End-anchored | Trailer-offset∩heads | `P&IDAttributes` at head+31 |
|---|---:|---:|---:|---:|---|---:|---:|
| `d06` | 8 | 47 | 8,804 | 0.9991 | yes | 25/25 | 26/47 |
| `nonascii-process-1` | 8 | 69 | 11,981 | 0.9993 | yes | 30/30 | 31/69 |
| `dwg0201` | 8 | 231 | 49,813 | 0.9998 | yes | 159/159 | 160/231 |
| `dwg0202` | 8 | 169 | 34,356 | 0.9998 | yes | 102/102 | 103/169 |
| `publish-a01` | 8 | 22 | 3,703 | 0.9978 | yes | 9/9 | 10/22 |
| `publish-dwg0202` | 8 | 169 | 34,356 | 0.9998 | yes | 102/102 | 103/169 |

- Every record in the chain masks to type code `0x0089` — the same type
  code that dominates the `/PSMcluster0` body chain.
- In `nonascii-process-1` the literal `0x89 0x00` marker count (68) is one
  lower than the chain record count (69): exactly one record head carries a
  non-zero high flag bit in its type word (`word & 0x3FFF == 0x0089` still
  holds). This mirrors the flagged `0x00CE` variants seen in the Sheet
  after-trace review and is recorded as a byte fact, not named.
- The only bytes outside the chain are the 8-byte prologue (below). There
  is no tail gap on any fixture.

## Finding 2: The Prologue Is The Cluster-Family Magic Plus A Record Counter

All 6 fixtures open with the same 8-byte prologue:

| Fixture | Prologue hex | u32 at +4 | Heuristic records | Chain records |
|---|---|---:|---:|---:|
| `d06` | `44 F5 90 6C 2F 00 00 00` | 47 | 47 | 47 |
| `nonascii-process-1` | `44 F5 90 6C 44 00 00 00` | 68 | 68 | 69 |
| `dwg0201` | `44 F5 90 6C E7 00 00 00` | 231 | 231 | 231 |
| `dwg0202` | `44 F5 90 6C A9 00 00 00` | 169 | 169 | 169 |
| `publish-a01` | `44 F5 90 6C 16 00 00 00` | 22 | 22 | 22 |
| `publish-dwg0202` | `44 F5 90 6C A9 00 00 00` | 169 | 169 | 169 |

- `44 F5 90 6C` is the known cluster-family magic `0x6C90_F544` (shared
  with `PSMcluster0`, `StyleCluster`, `Dynamic Attributes Metadata`,
  `Sheet*`). The DA stream is therefore not an outlier format: it is a
  minimal cluster-family member with an 8-byte header.
- The u32 at offset 4 equals the count of literal `0x89 0x00`-headed
  records on 6/6 fixtures, and equals the strict chain record count on 5/6
  (the `nonascii-process-1` flagged-head record is not included in the
  counter). Counter semantics stay unnamed pending IDA; the 5/6 vs 6/6
  split means the walker must not use it as a hard invariant.

## Finding 3: The 31-Byte "Trailer" Is The Next Record's Envelope Head

100% of signature-valid trailer offsets coincide with chain record heads
(417/417 cross-fixture). Layout reconciliation:

```text
chain head +0..2   89 00            = u16 type word (0x0089)
           +2..6   u32 bytes_to_follow ("size" in DaRecordTrailer)
           +6..10  u32 record_id
           +10..18 8 zero bytes
           +18..22 u32 field_x
           +22..24 FF FF
           +24..28 u32 class_id
           +28..31 14 00 00
           +31..   ASCII class name (`P&IDAttributes` on 433 records, ...)
```

The Phase 11/12 "31-byte record trailer" interpretation and the chain-head
interpretation describe the same bytes: what `extract_record_trailers`
reads as the trailer of record N is byte-identical to the envelope head +
fixed head-tail of record N+1. No existing decoded field changes meaning;
`record_start` (class-name offset) is simply `head + 31` for records whose
head matches the strict signature. The `P&IDAttributes`-at-head+31 counts
(trailers + 1 per fixture) confirm this: the `+1` is each stream's first
record, which has no preceding record to attribute a "trailer" to.

## Finding 4: Record Class And Attribute Census (Product Value)

Trailer `class_id` histogram (cross-fixture, 417 signature-valid heads):
`0xF6` (relationship) ×118, `0xEA` (drawing) ×71, `0x10D` ×62, `0x10A`
×46, `0xE7` ×28, `0x10B` ×19, `0xE9` ×17, plus a long tail.

Heuristic record class names: `P&IDAttributes` ×433, `MultiLinearPattern`
×103, `Symbol` ×78, `SymbolAttr` ×32, `_BranchPoint` ×19, plus 10 smaller
classes.

Attribute-name census (top of 68 distinct names): `DrawingID`,
`DrawingItemType`, `DrawingNo`, `Flag`, `ProjectNumber` (439 fields each),
`ModelID`, `ModelItemType` (433 each), `HasMultiLP` (103), `DefinitionName`
(78), `Class` / `Type` (20 each).

Product reconciliation:

- The object graph and the D06 relationship fallback already consume
  `ModelID` / `ModelItemType` / `DrawingID` / `Relationship.<GUID>`; the
  high-frequency attributes are not unexploited treasure.
- The census also exposes heuristic noise: `.sym` library paths leak into
  attribute names (for example `?|\Piping\Labels - ...sym` rows with only
  `Empty` values, and a mangled `DefinitionName` example), and 24-hex-char
  fragments appear as attribute names. These are artifacts of whole-stream
  scanning without record boundaries — exactly what byte-accurate chain
  boundaries would fix.

## Reconciliation With Current Parser Surface

| Surface | Today | After a chain walker |
|---|---|---|
| `scan_da_landmarks_with_trace` | Claims class names + 31-byte heads + DrawingID landmarks as `Decoded`; everything else leftover | Unchanged (`Decoded` landmark claims stay) |
| `parse_attribute_records` | Whole-stream heuristic scan; section bounds from `89 00` + u32 | Unchanged this slice; future slices can scope extraction per chain record |
| Byte-audit `/Unclustered Dynamic Attributes` | leftover 111,120 / ratio 0.2233 | leftover 0 (prologue `Probed` + envelope `Probed` + payload `Probed`, landmarks stay `Decoded`) |

## Decision: Parser-Only Next Step Is Justified

The body is not an unknown format and does not need IDA to account for its
bytes: it is the same cluster-family record chain already shipped twice
(Phase 29-F `/PSMcluster0`, Phase 29-G `/StyleCluster`), with a smaller
8-byte prologue.

### Parser backlog item (audit-only, shared-core template)

- Add `decode_unclustered_da_body_records` to
  `src/parsers/cluster_header.rs`, reusing the shared
  `decode_cluster_body_records` core with an 8-byte prologue
  (magic `0x6C90_F544` + u32 counter) and the Phase 29-G end-anchored
  full-coverage gate (≥1 record, chain must end exactly at the stream end,
  otherwise zero claim).
- Add `parse_unclustered_da_with_trace` and call it from the
  `/Unclustered Dynamic Attributes` branch in `src/byte_audit/aggregate.rs`
  alongside (not replacing) `scan_da_landmarks_with_trace`.
- Confidence assignment: prologue `Probed`, envelope `Probed`, payload
  `Probed`. The envelope does **not** get `Decoded`: the prologue counter
  invariant holds only 5/6 against the strict chain (StyleCluster
  precedent), and the counter semantics are unnamed.
- Unit tests (cluster_header module): canonical 2-record synthetic chain,
  prologue magic mismatch → zero claim, counter mismatch still claims
  (counter is reported, not gated), non-end-anchored chain → zero claim,
  truncated header → zero claim, empty stream → zero claim.
- Panic-safety: add the new entry point to `tests/parser_panic_safety.rs`.
- Cross-fixture ratchet (`tests/parse_real_files.rs`):
  `da_body_chain_is_end_anchored_across_fixtures` asserting chain start 8,
  records `{d06: 47, nonascii: 69, dwg0201: 231, dwg0202: 169,
  publish-a01: 22, publish-dwg0202: 169}`, and
  `/Unclustered Dynamic Attributes` consumed ratio 1.0.
- Regenerate the 12 snapshot JSONs; expected effect: family leftover
  111,120 → 0; per-fixture whole-file ratios rise accordingly.

### Named object / relationship benefit (Slice 29-C acceptance)

Byte-accurate record boundaries from the walker are the prerequisite for:

1. scoping `parse_attribute_records` per record, removing the `.sym`-path
   and hex-fragment attribute-name artifacts in the census above;
2. lifting the head fields already decoded by `extract_record_trailers`
   (`record_id`, `field_x`, `class_id`, relationship GUID pairing) from a
   trailer heuristic to a bounded per-record surface, which directly
   serves the object graph and the D06-style relationship fallback.

Neither follow-up is part of this slice; both must keep existing field
names and must not invent new ones.

### IDA target request (secondary)

When a JStyle/RAD host IDB becomes available:

- the `0x0089` record reader shared by `/PSMcluster0` and
  `/Unclustered Dynamic Attributes` (likely the same `PersistManager`
  path recovered in Phase 27);
- the meaning of the 8-byte DA prologue counter and why one
  `nonascii-process-1` record head carries a high flag bit;
- semantic names for head fields currently labeled `record_id` / `field_x`
  / `class_id` by fixture evidence.

## Guardrails

- The chain walk is byte-shape evidence; `0x0089` is a type code, not a
  semantic name.
- No new field names from byte positions: head fields keep their existing
  fixture-evidence names; the prologue counter stays unnamed.
- The flagged type word in `nonascii-process-1` is recorded, not modeled.
- `Decoded` confidence stays reserved for the existing landmark claims;
  the walker claims are `Probed` accounting only.

## Implementation Status (2026-06-11, same session)

The backlog item above shipped in the same session:

- `src/parsers/cluster_header.rs`: `UNCLUSTERED_DA_PROLOGUE_LEN` (8),
  `decode_unclustered_da_body_records` (magic gate + fixed chain start +
  end-anchored full-coverage gate, counter not gated), and
  `parse_unclustered_da_with_trace` (prologue + every record `Probed`;
  zero claim on gate failure). 7 new unit tests.
- `src/byte_audit/aggregate.rs`: the `/Unclustered Dynamic Attributes`
  branch now runs the walker **and** `scan_da_landmarks_with_trace` into
  one builder under parser name `parse_unclustered_da`; landmark
  `Decoded` claims are unchanged and overlaps are union-counted. One new
  aggregate test (synthetic fixture-shaped chain → full coverage); the
  landmark-only synthetic test keeps its 87-byte expectation because the
  walker correctly no-ops without the magic prologue.
- `tests/parser_panic_safety.rs`: `decode_unclustered_da_body_records`
  added to the adversarial matrix.
- `tests/parse_real_files.rs`:
  `da_body_chain_is_end_anchored_across_fixtures` pins chain start 8,
  records `{d06: 47, nonascii: 69, dwg0201: 231, dwg0202: 169,
  publish-a01: 22, publish-dwg0202: 169}`, all type codes masking to
  `0x0089`, parser name, and leftover = 0 on 6/6 fixtures.
- Snapshots regenerated: `/Unclustered Dynamic Attributes` leftover
  111,120 → 0 across the family; whole-file coverage ratios rise to
  `d06` 0.3793 / `nonascii` 0.6700 / `dwg0201` 0.5912 / `dwg0202`
  0.5923 / `publish-a01` 0.2733 / `publish-dwg0202` 0.5925.
- Per-record attribute scoping and head-field surfacing (the named
  object / relationship benefits) remain follow-up slices, not part of
  this walker.

## Implementation Status — Slice K Follow-Up (2026-06-11, same session)

Named benefit #1 (per-record attribute scoping) shipped as Phase 29
Slice K:

- `src/parsers/dynamic_attr_records.rs`: the section-body parser is
  factored out of `try_parse_record` (`parse_section_body`, byte-for-byte
  identical extraction) and a new
  `parse_attribute_records_chain_scoped` entry point parses each
  attribute section from its exact chain record bounds when the Slice C
  gate passes, falling back to the legacy scan otherwise. 4 new unit
  tests (flagged-head recovery, payload-marker immunity, fallback
  equivalence, class-name-less record skip).
- `src/streams/dynamic_attrs.rs`: the `/Unclustered Dynamic Attributes`
  document pipeline now uses the chain-scoped extraction;
  `streams/cluster.rs` keeps the legacy generic scan for non-DA
  cluster streams.
- Cross-fixture ratchet
  (`da_chain_scoped_attribute_extraction_matches_or_beats_legacy_scan`):
  chain-scoped ≥ legacy on 6/6 fixtures; `nonascii-process-1` recovers
  the flagged-head record (68 → 69, class `Symbol`); all other fixtures
  unchanged (47/231/169/22/169), so no existing baseline moved.
- Head-field surfacing (benefit #2) stays deferred: lifting
  `record_id` / `field_x` / `class_id` for non-signature records still
  needs IDA confirmation of the head-tail layout.
