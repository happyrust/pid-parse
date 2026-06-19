# PID Parser Roadmap And Promotion Gates

> Date: 2026-06-19  
> Scope: unresolved SmartPlant / Smart P&ID `.pid` parser families and the gates required before any promotion to `Decoded` confidence.  
> Status: roadmap artifact. This document does not implement new parsers and does not widen writer support.

## Evidence Baseline

This roadmap inherits the canonical confidence classes from
`docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`:
`Decoded`, `TypedAudit`, `Probe`, `IdentifiedOnly`, and `Unknown`.

Promotion to `Decoded` is allowed only when all required proof exists:

1. exact source stream path;
2. bounded byte range for every decoded field and record;
3. record family or kind, including type code when applicable;
4. fixture identity and cross-fixture ratchets;
5. panic-safety coverage for every new public byte parser entry point;
6. IDA reader evidence or controlled fixture evidence when semantic names are claimed;
7. separate IDA writer evidence or controlled write fixture evidence before any writer surface changes.

Current IDA impact is conservative. `sppid.dll` and `core.dll` searches
in `docs/analysis/2026-06-19-ida-evidence-baseline.md` found no direct
raw `.pid` persistence path for the unresolved families below. Historical
IDA evidence remains scoped to `JStyleOverride`, `JSitesList.entries`,
`DocVersion2` / embedded OLE, `0x0089` export behavior, and the
`PSMspacemap` handle model. It does not prove writer support.

## Promotion Checklist For Any New Decoded Binary Parser

A parser change may be proposed only after a plan or red test names:

- stream path, for example `/Sheet6` or `/PSMspacemap/0x00000000`;
- bounded byte range, using half-open offsets such as `0x120..0x142`;
- record family and kind, for example `0x00FA GraphicGroup`;
- fixture identity, including filename and optional soft-skip reason;
- representative byte examples and expected decoded values;
- cross-fixture counts or distribution ratchets;
- byte-audit classification for decoded, audit/probe, and leftover ranges;
- malformed, truncated, and adversarial panic-safety cases;
- schema or public DTO checks when any public output changes;
- rollback criteria if fixture drift, IDA evidence, or controlled fixtures reject the proposed semantics.

Promotion must be rejected when exact byte ranges are absent, when a
single fixture is the only semantic evidence, or when only a marker,
magic value, or type code is known.

## Roadmap Backlog

| ID | Family | Current status | Evidence already known | Evidence needed for `Decoded` promotion | Blocker | Negative closeout | Forbidden shortcut | Byte-audit policy | Test plan and rollback | Writer policy |
|---|---|---|---|---|---|---|---|---|---|---|
| `ROADMAP-PSMSPACEMAP` | `PSMspacemap` storage and page bytes | Storage `IdentifiedOnly`; page bytes `Unknown` | Historical `radsrvitem.dll` proves handle model `handle = (segment_id << 13) \| entry_index`, entry range `0..0x1FFF`, and segment capacity `0x2000`. Fixture inventory shows common unregistered paths such as `/PSMspacemap/0x00000000`, `/0x00002000`, `/0x00004000`, and `/0x00006000`. | Direct persisted page reader/writer path or controlled fixture map tying raw page bytes to segment fields, plus stream path, page byte ranges, segment id, entry index ranges, fixture ratchets, and panic-safety coverage. | Handle math is in-memory evidence only; no on-disk page layout is proven. Current `sppid.dll` and `core.dll` searches found no direct page reader. | If no direct page reader/writer or controlled fixture can connect page bytes to entries, close the slice as `IdentifiedOnly` storage plus `Unknown` page bytes and preserve all page bodies as leftover. | Do not infer raw page layout from handle math, page name, or `tseg`-like magic alone. Do not mark a page consumed because its storage path matches a segment id. | Raw page bodies remain unregistered or explicit leftover until bounded fields are proven. A future audit parser may claim only the bytes it structurally validates; unknown page tails must stay leftover. | Add fixture ratchets for every available page path, controlled malformed pages for bounds checks, and `tests/parser_panic_safety.rs` coverage if a public page parser is introduced. Roll back to leftover if any fixture page violates the claimed layout. | Read-only. No semantic page writer or compaction until separate writer evidence exists. |
| `ROADMAP-STYLECLUSTER` | `StyleCluster` prefix and tail-chain family | Tail chain `TypedAudit`; prefix `Probe` | Phase 29 proves end-anchored tail-chain accounting. Prefix is mapped as 12-byte opener, 520-byte constant GUID-bearing boilerplate, and fixture-specific 42-byte style-slot texture. Current IDA baseline found no direct `StyleCluster` reader/writer in `sppid.dll` or `core.dll`. | Native reader/writer path, preferably `style.dll` stream type `0x005A`, or controlled fixture edits that identify the prefix count, GUID entries, style-slot layout, and tail record type names. Need byte ranges per field and cross-fixture distribution ratchets. | GUID region is not a uniform-stride array and the prefix cannot be parsed by template equality without overclaiming. | If no native path or controlled fixture resolves the prefix, keep the prefix as `Probe` / leftover with the existing shape notes. Tail chain remains audit-only. | Do not treat embedded GUID byte patterns as registry semantics. Do not create hardcoded template comparators and call them parsers. Do not name 42-byte slot fields from position alone. | Tail-chain bytes may be audit/probe-accounted where the current walker validates an end-anchored chain. Prefix bytes remain leftover unless each subrange has a stable structural rule. | Ratchet prefix lengths, common constant prefix length, style-slot offsets, and tail-chain counts. Add unit tests for malformed headers and non-end-anchored chains. Roll back field names if IDA or controlled fixtures reject them. | Read-only. Style parsing does not authorize style writing. |
| `ROADMAP-0010` | PSM sub-record `0x0010` and attribute fragments | `TypedAudit` | Phase 18/19/20 exposes stable PSM headers, raw payloads, `leading_word: Option<u16>` as a positional field, cross-fixture totals, and RAD type-table identity for GUID `1D1928C0-0000-0000-C000-000000000046`. No human class name, `Read` / `Load` / `DoIO`, or discriminator offset is proven. | Concrete reader or controlled fixture evidence proving the discriminator, sub-kind enum, semantic payload fields, and relationship to JStyleOverride offsets. Must include record byte ranges, fixture identities, counts/distributions, and panic-safety cases. | The likely discriminator is unresolved. `leading_word` distribution is not enough because several size buckets are heterogeneous. | If no reader or controlled fixture proves a discriminator, keep `0x0010` as `TypedAudit` with raw payload and positional fields only. | Do not rename `leading_word` to `sub_kind`. Do not promote size buckets or first payload word to business semantics. Do not emit geometry or writer instructions from `0x0010`. | Header and bounded payload may be audit-accounted. Decoded ranges are limited to proven envelope fields; payload field guesses remain audit/probe or leftover. | Ratchet total records, size buckets, `leading_word` distribution, referenced source ranges, and provenance shape. Add adversarial payload tests and panic-safety matrix coverage for any new entry point. Roll back semantic names on bucket drift or IDA mismatch. | Read-only. No writer scope. |
| `ROADMAP-GRAPHICGROUP-00FA` | `GraphicGroup 0x00FA` payload | `TypedAudit` | Conservative decoder validates PSM type `0x00FA`, bounded payload length, `oid`, `parent_ref`, small payload word, `sub_type_word`, and raw tail. Parser-level ratchet is 352 records across four fixtures. Tail often contains words that match nearby geometry OIDs, but offsets vary by size and subtype. | IDA reader evidence or controlled fixture edits proving child/reference payload semantics, candidate OID offsets by bucket, flag/sentinel meanings, and whether tail references are active children. Need exact byte ranges and cross-fixture bucket ratchets. | Tail mixes OID-like words with flags, sentinels, and scalar-looking values. `parent_ref == 6` is strong local evidence but not universal proof. | If child/reference offsets cannot be proven, keep stable header fields and raw tail as `TypedAudit`; candidate OID extraction may remain probe-only. | Do not expose `child_oids` or treat tail integers as child OIDs without proof. Do not emit decoded geometry from `GraphicGroup`. | Header fields may be audit-accounted. Tail ranges stay audit/probe or leftover unless a bucket-specific structural rule is proven. | Ratchet counts per fixture, size/subtype buckets, stable header fields, and candidate offset distributions. Add malformed size and truncation tests. Roll back any child/reference field on cross-fixture contradiction. | Read-only. Group structure does not authorize Sheet semantic write-back. |
| `ROADMAP-JSITESLIST-TRAILING` | `JSitesList.trailing_slots` | Entries `Decoded`; trailing slots `TypedAudit` | `/JSitesList` layout is `"OLEM"` + u32 count + u32 slot table. Logical entries match `JSite<id>` storages on 6/6 fixtures. `dwg0202` and `publish-dwg0202` carry three slots beyond `count` that repeat logical values. Historical `OLESITE.dll` confirms count-bounded `JSite` entries. | Delete/compact/stale-slot writer evidence or controlled before/after fixture proving whether trailing slots are inactive, reusable, deleted, or active under another count. Need byte ranges for header, entries, and trailing slots. | Active-vs-stale semantics of slots beyond count are not proven. | If no delete/compact writer evidence exists, keep trailing slots as `TypedAudit` and leave their byte ranges unconsumed or audit-only according to byte-audit policy. | Do not treat trailing slots as active entries. Do not rename them to deleted, free-list, or reusable slots without writer proof. | Header and first `count` logical entries are decoded. Slots beyond `count` are audit/tail ranges and must remain distinguishable from decoded entries. | Ratchet counts `{9,10,20,13,5,13}`, trailing counts `{0,0,0,3,0,3}`, storage-match counts, and byte-audit leftover equal to `4 * trailing_slots`. Roll back if controlled deletion shows different semantics. | Read-only. No JSite list compaction, deletion, or active-entry rewrite. |
| `ROADMAP-DA-0089` | `0x0089` / class 137 Dynamic Attribute and cluster heads | `TypedAudit` | `/Unclustered Dynamic Attributes` is an end-anchored `0x0089` envelope chain with 8-byte prologue. The old 31-byte trailer aligns with the next record head. Historical `radsrvitem.dll` confirms `0x0089` is a runtime/persisted type and exports generic `RAD_OBJECT_TYPE = "137"`, but the type-name table does not map it to a semantic name. | Concrete DA/cluster reader path proving prologue counter semantics, head field names, class/family name, payload boundaries, and relation to existing object graph fields. Controlled fixture edits can supplement reader proof. | Current export path confirms type existence but not payload semantics. One fixture has a flagged head that the prologue count does not include, so counter semantics are not a hard invariant. | If no reader path is found, keep envelope and existing landmark claims separate: landmarks may remain decoded where already proven, envelope/payload stays audit/probe. | Do not call `0x0089` a named business record family from `RAD_OBJECT_TYPE = "137"`. Do not rename `record_id`, `field_x`, or `class_id` beyond current fixture-evidence names without proof. | Prologue and envelope ranges may be audit/probe-accounted; decoded landmark ranges remain separate. Payload bytes are not consumed as decoded just because the envelope is valid. | Ratchet chain start `8`, counts by fixture, end anchoring, flagged-head behavior, class-id histograms, and `P&IDAttributes` alignment. Add malformed chain tests and panic-safety for new public entry points. Roll back semantic names if IDA maps fields differently. | Read-only. DA semantic edits remain forbidden. |
| `ROADMAP-PAGE-TRANSFORM` | Page transform / coordinate units | `Unknown`; coordinate hints are `Probe` | Sheet probes find normalized f64 coordinate-domain evidence and candidate marker groups. Existing page-transform evidence finds no source-backed record containing complete width/height, units, origin, scale, direction, bounds, or matrix. Phase 24 negative evidence records zero page-dimension scalar matches in top candidates. | A bounded source record, IDA reader, or controlled fixture proving coordinate space, units, direction, origin, scale, bounds, and provenance. Must distinguish raw source coordinates from page coordinates and prove transform direction. | Template names, page dimensions, isolated f64 pairs, and normalized coordinate hints do not identify a transform. | Until every required component is proven, preserve `PidPageTransform::Unavailable`. Future work may add evidence DTOs only as `Probe` or `TypedAudit`. | Do not make page transforms available from template dimensions, page size, normalized f64 hints, candidate marker support, or inferred bounds alone. Do not treat probe geometry as decoded geometry. | Coordinate windows and page metadata candidates stay probe/audit ranges. No whole Sheet stream or marker range is consumed as decoded solely because it contains plausible numbers. | Ratchet source byte ranges, complete component set, unit and direction checks, fixture coverage, and regression proving no coordinate-space conflation. Roll back to `Unavailable` on missing component or fixture contradiction. | Not writable. Page transform availability does not imply geometry write support. |

## Byte-Audit Strategy

Future parsers must declare ranges in three non-overlapping classes:

1. **Decoded ranges**: every byte contributes to a proven field with
   semantics, stream path, record kind, and fixture identity.
2. **Audit/probe ranges**: envelope bytes, counters, headers, raw
   payloads, or heuristic windows that are useful for investigation but
   not semantic decoded output.
3. **Leftover ranges**: bytes not structurally validated, tail bytes,
   padding, stale slots, raw page bodies, or payload fragments whose
   semantics remain unknown.

Recognizing a stream name, storage path, magic value, header, type word,
or count never permits marking an entire stream consumed. Unknown tails
must stay visible. A parser that can identify a header but not validate
the body should claim only the header range, or no range if even the
header rule is not stable across fixtures.

## Fixture Ratchets

Each promotion slice must include fixture ratchets before merging:

- counts per fixture and per stream path;
- distribution checks for size buckets, type words, or leading positional fields;
- representative decoded field values with byte ranges;
- provenance shape, including stream path, record offset, and record kind;
- optional fixture soft-skips with explicit messages;
- byte-audit deltas showing decoded, audit/probe, and leftover movement.

Single-fixture evidence can open an investigation but cannot promote
business semantics. Optional proprietary fixtures may soft-skip only if
the skip is explicit and the required minimum available fixture set is
documented in the test.

## Panic-Safety Plan

Every new public byte parser entry point must be added to
`tests/parser_panic_safety.rs`. The local parser module must also include
focused unit tests for:

- empty input;
- truncated header;
- declared length beyond input length;
- integer overflow bait in offset or length math;
- malformed type code or magic;
- non-end-anchored chain where end anchoring is required;
- adversarial payload with valid header and invalid body.

The desired failure mode is `None`, empty output, or a recoverable error.
No parser should panic, unwrap untrusted values, or slice without an
upstream bounds check.

## Forbidden Shortcuts

These shortcuts are forbidden and should be testable through file
inspection, unit tests, or fixture ratchets:

- marker-only semantic promotion;
- single-fixture overgeneralization;
- hiding unknown tails as consumed bytes;
- naming positional bytes as business semantics;
- deriving writer support from reader support;
- making page transforms available from page size, template dimensions, or f64 hints alone;
- treating probe geometry as decoded geometry;
- inferring `PSMspacemap` page layout from handle math alone;
- naming `0x0010.leading_word` as `sub_kind`;
- treating `GraphicGroup` tail integers as child OIDs without proof;
- treating `JSitesList.trailing_slots` as active entries without delete/compact writer evidence;
- using MDF publish parity as raw `.pid` parser evidence.

## IDA And Writer Boundary

IDA evidence informs priority and closeout decisions, but it does not
widen writer scope unless it proves a native writer path for the exact
surface, or a controlled write fixture proves a bounded write operation.

Current roadmap implications:

- `sppid.dll` negative evidence means unresolved families are not
  promoted from application-glue strings or absence of hits.
- `core.dll` remains broad platform evidence only, not raw `.pid`
  byte-layout proof.
- historical `style.dll` evidence keeps `0x0030` as `JStyleOverride`;
  it does not decode `StyleCluster`, `0x0010`, or `GraphicGroup`.
- historical `OLESITE.dll` evidence supports count-bounded
  `JSitesList.entries`; it does not define trailing-slot writer behavior.
- historical `radsrvitem.dll` evidence supports `0x0089` existence and
  `PSMspacemap` handle math; it does not decode DA payloads or raw
  spacemap page bytes.

No parser promotion in this roadmap authorizes geometry JSON write-back,
Sheet semantic write-back, DA semantic edits, JSite compaction, or MDF
publish output as raw `.pid` evidence.

## References

- `docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`
- `docs/analysis/2026-06-19-ida-evidence-baseline.md`
- `docs/analysis/2026-06-12-phase30-radsrvitem-record-spacemap-ida.md`
- `docs/analysis/2026-06-08-phase29-stylecluster-prefix-characterization.md`
- `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
- `docs/analysis/2026-05-14-psm-0x00fa-graphic-group-layout.md`
- `docs/analysis/2026-06-08-phase29-jsiteslist-revision-tails.md`
- `docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`
- `docs/analysis/2026-05-09-page-transform-evidence.md`
- `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md`
