# Phase 29 Slice B: PSMcluster0 Leftover Triage

> Generated from `cargo run --example probe_phase29_psmcluster0_body_triage`
> (run 2026-06-10). Investigation-only report; no parser semantics are
> promoted. Scope and acceptance criteria come from
> `docs/specs/2026-06-08-pid-file-format-spec-kit/phase29-candidate-slices.md`
> Slice 29-B.

## Question

`snapshot-priority-backlog.md` ranks `/PSMcluster0` as the second-largest
leftover family (193,173 leftover bytes across 6 fixtures, coverage ratio
0.0042) despite a decoded 16-byte cluster header and a decoded indexed
UTF-16LE string table. What is the post-string-table body, and is the next
step parser-only or IDA-gated?

## Header And Leftover Summary

| Fixture | Stream bytes | `record_count` | `stream_type` | `body_len` | `flags` | Leftover start | Leftover bytes |
|---|---:|---:|---:|---:|---:|---:|---:|
| `d06` | 6,201 | 62 | `0x0075` | 113 | `0x0001` | 135 | 6,066 |
| `nonascii-process-1` | 78,882 | 444 | `0x0075` | 113 | `0x0001` | 135 | 78,747 |
| `dwg0201` | 28,515 | 196 | `0x0075` | 113 | `0x0001` | 135 | 28,380 |
| `dwg0202` | 37,430 | 231 | `0x0075` | 113 | `0x0001` | 135 | 37,295 |
| `publish-a01` | 5,399 | 45 | `0x0075` | 113 | `0x0001` | 135 | 5,264 |
| `publish-dwg0202` | 37,556 | 233 | `0x0075` | 113 | `0x0001` | 135 | 37,421 |

Observations:

- All 6 fixtures share `stream_type = 0x0075`, `body_len = 113`,
  `flags = 0x0001`, and the same string-table end offset (135), i.e. the
  prefix layout up to the leftover region is byte-stable across fixtures.
- `body_len = 113` is **constant** while stream sizes vary 6K–79K. The
  field name `body_len` in `ClusterHeader` is a historical guess and does
  not describe the body length of this stream family. Do not rename
  without IDA evidence; recorded here as a naming caveat.
- `record_count` varies per fixture and is corroborated below.

## Finding 1: The Body Is A Single Continuous PSM-Envelope Record Chain

A strict walk using the same 6-byte record envelope as the `Sheet*` family
(`u16 type_word` where `type_code = type_word & 0x3FFF`, then
`u32 bytes_to_follow`) explains essentially the whole leftover region in
every fixture:

| Fixture | Chain start | Records | Bytes covered | Ratio of leftover | Resync bytes |
|---|---:|---:|---:|---:|---:|
| `d06` | 145 | 60 | 6,056 | 0.9984 | 10 |
| `nonascii-process-1` | 145 | 442 | 78,737 | 0.9999 | 10 |
| `dwg0201` | 145 | 194 | 28,370 | 0.9996 | 10 |
| `dwg0202` | 145 | 229 | 37,285 | 0.9997 | 10 |
| `publish-a01` | 145 | 43 | 5,254 | 0.9981 | 10 |
| `publish-dwg0202` | 145 | 231 | 37,411 | 0.9997 | 10 |

- The walk forms **one** chain per fixture: it starts at offset 145 and
  runs to end-of-stream without a single resync inside the chain.
- The only unexplained bytes are a fixed **10-byte prologue** at
  `135..145`, identical in all 6 fixtures:
  `00 00 01 00 00 00 00 00 00 00`.

## Finding 2: `record_count` Invariant

`chain_records == header.record_count - 2` holds in **6 of 6** fixtures
(60/62, 442/444, 194/196, 229/231, 43/45, 231/233). This is the first
cross-fixture corroboration of the `record_count` header field for the
`PSMcluster0` family. The off-by-2 is unexplained; plausible candidates
(string table counted as records, prologue encoding two null records) are
hypotheses only.

## Finding 3: Type-Code Distribution Is Stable Across Fixtures

Chain type histograms (top entries):

| Fixture | Histogram |
|---|---|
| `d06` | `0x0089`×13, `0x0003`×12, `0x0081`×12, `0x00EC`×6, `0x004A`×2, `0x004F`×2, singletons `0x0002 0x0036 0x0037 0x0042` |
| `nonascii-process-1` | `0x0089`×377, `0x0003`×28, `0x0081`×14, `0x00EC`×7, `0x004A`×2, `0x004F`×2, singletons `0x0002 0x0036 0x0037 0x0042` |
| `dwg0201` | `0x0089`×122, `0x0003`×26, `0x00EC`×17, `0x0081`×12, `0x004A`×2, `0x004F`×2, singletons `0x0002 0x0019 0x0036 0x0037` |
| `dwg0202` | `0x0089`×172, `0x0003`×21, `0x0081`×11, `0x00EC`×11, singletons `0x0002 0x0036 0x0037 0x0042 0x004A 0x004C` |
| `publish-a01` | `0x0089`×12, `0x0081`×11, `0x0003`×2, `0x004A`×2, `0x004F`×2, `0x00EC`×2, singletons `0x0002 0x0036 0x0037 0x0042` |
| `publish-dwg0202` | `0x0089`×172, `0x0003`×23, `0x0081`×11, `0x00EC`×11, singletons `0x0002 0x0036 0x0037 0x0042 0x004A 0x004C` |

- Every fixture opens its chain with `0x0002` and shares a small stable
  set of one-off types plus four recurring types
  (`0x0089` dominant, then `0x0003`, `0x0081`, `0x00EC`).
- **Namespace warning**: these type codes live in the `PSMcluster0`
  stream, not in a `Sheet*` stream. Mapping them through the Phase 27
  Sheet/IGDS table (e.g. `0x0019 = igKeyPointRelation2d`) is **not**
  justified without IDA evidence that both families share one type-code
  namespace. No name from the ig* table is claimed for any code above.

## Finding 4: Payload Texture Is Property/Catalog-Like, Not Geometry-Like

Bounded windows over record payloads show:

- ASCII (single-byte) name strings: `_SmartText`, `FormatString`,
  `ModelItemType`, `ModelID`, `Flag`, `PropertyFormat`, `ItemFilter`,
  `Equipment`, `ItemProperty`, `_TagSequenceNo`, `_ToFromText`,
  `PropertyCase…ShortText`, plus GB-flavored unit text
  (`Thickness-mm（hidden unit）`).
- UTF-16LE runs: `BsplineExtensionMode`, `Section1`, `Sheet1`, `Sketch`,
  `Default`, `NotClaimed`.
- An 8-byte-stride id/value table shape right after the prologue
  (`u16-id`-like, `u32-value`-like, trailing `01 00`), with
  fixture-varying values.

This is consistent with a per-drawing property / format-definition
catalog rather than drawing geometry. No field names are promoted from
this texture.

## Reconciliation With Current Parser Surface

`src/parsers/cluster_header.rs::parse_psm_cluster0_with_trace` currently
consumes: 16-byte header (`Decoded`) + heuristic locator gap (`Probed`) +
indexed string table (`Decoded`). The walker stops at the string-table
sentinel (offset 135), which is exactly where the leftover region and the
record chain begin. Nothing in `streams/cluster.rs` reads past the table.

## Decision: Parser-Only Next Step Is Justified

Per Slice 29-B acceptance criteria, this report files a parser backlog
item (primary) and an IDA target request (secondary):

### Parser backlog item (audit-only, Phase 18 template)

Add an audit-only record-chain walker for `/PSMcluster0` bodies on the
`GraphicGroup` / `0x0010` template:

- `decode_cluster_body_records` + `decode_cluster_body_record_at` in
  `src/parsers/cluster_header.rs` (or a sibling module): 6-byte envelope
  (`type_code`, `bytes_to_follow`) decoded, payload kept raw, full
  provenance, **no semantic type names**, no `PidGraphicKind` emission.
- Prologue `135..145` traced as `Probed` (byte position known, semantics
  unknown); record envelopes traced `Decoded`; payloads traced
  `AuditOnly`-equivalent confidence per existing byte-audit vocabulary.
- Exact test targets:
  - unit tests: canonical chain, truncated header, truncated payload,
    `type_code == 0` stop, oversized `bytes_to_follow` stop, panic-safety
    entries in `tests/parser_panic_safety.rs`;
  - cross-fixture ratchet: `chain_records == header.record_count - 2` on
    all 6 local fixtures (soft-skip absent fixtures);
  - byte-audit ratchet: `/PSMcluster0` consumed ratio rises from ~0.004
    to ≥ 0.99 on the 6-fixture snapshot matrix.
- Expected accounting impact: ~193K leftover bytes → ~190K accounted
  (family-wide), moving `PSMcluster0` out of the top leftover families.

### IDA target request (secondary)

When a SmartPlant host IDB beyond `radsrvitem.dll` becomes available
(`style.dll`, `J2DSrv.dll`, `sppid.dll`, `XCeedRAD.dll`,
`smartplantpid.exe`):

- identify the reader for cluster `stream_type 0x0075`;
- resolve whether `PSMcluster0` record type codes share the Sheet/IGDS
  namespace or form a separate catalog namespace;
- name the dominant types `0x0089`, `0x0003`, `0x0081`, `0x00EC` and the
  opener `0x0002`;
- explain the 10-byte prologue and the `record_count - 2` offset.

## Guardrails

- No `Decoded` promotion for record payloads: envelope walk + invariant
  is structure evidence, not field semantics.
- Do not map `PSMcluster0` type codes through the Sheet ig* table without
  IDA confirmation of a shared namespace.
- Text snippets are bounded human-review samples, not extracted product
  data.
- `ClusterHeader.body_len` naming caveat stands until IDA evidence.

## Implementation Status (2026-06-10, same session)

The parser backlog item above shipped as
`src/parsers/cluster_header.rs::{decode_cluster_body_record_at,
decode_cluster_body_records, decode_psm_cluster0_body_records}` plus the
`parse_psm_cluster0_with_trace` extension (prologue `Probed`, envelope
`Decoded`, payload `Probed`, full-coverage gate — partial chains claim
nothing). The first two carry stream-neutral names because the
`/StyleCluster` follow-up walker reuses the same record core.

Test results:

- 8 new unit tests in `parsers::cluster_header` (canonical record,
  zero-type-code / oversized-`bytes_to_follow` / truncated rejections,
  chain stop, full-coverage gate, trace integration both ways) — pass.
- `tests/parser_panic_safety.rs` entries for all three new fns — pass.
- Cross-fixture ratchet
  (`tests/parse_real_files.rs::psmcluster0_body_chain_matches_record_count_invariant`):
  6/6 fixtures decode, `record_count - 2` invariant holds, byte-audit
  consumed ratio = 1.0000 on every fixture.
- 6-fixture snapshot matrix regenerated: `/PSMcluster0` leftover drops
  from 193,173 bytes family-wide to **0**; whole-file ratios improve to
  0.168–0.585 (see `data-model.md` / `snapshot-priority-backlog.md`).
