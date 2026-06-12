# Data Model: Evidence-Graded PID Format Inventory

## Entry Template

Each format entry uses this conceptual model:

```text
name:
path_or_type_code:
layer:
status:
parser_surface:
model_surface:
evidence:
known_gaps:
next_action:
```

## Layers

| Layer | Description |
|---|---|
| `Container` | OLE / CFBF root, storages, streams, CLSID, timestamps, raw bytes. |
| `Metadata` | OLE property sets and XML metadata. |
| `Registry` | Doc version, AppObject, tagged storage list. |
| `PSMIndex` | PSM roots, cluster table, segment table, spacemap. |
| `Cluster` | Cluster-family binary streams and common headers. |
| `DynamicAttributes` | Object attributes, relationship records, DA trailers. |
| `JSite` | Symbol / object instance storage evidence. |
| `Sheet` | Sheet stream PSM / IGDS / RAD record families. |
| `Derived` | Object graph, cross reference, layout, normalized geometry. |
| `Writer` | Package and writer boundary. |
| `PublishAdjunct` | Backup / MDF / publish XML pipeline, not `.pid` binary format. |

## Container And Metadata Formats

| Name | Path / Signature | Layer | Status | Parser / Model | Gaps |
|---|---|---|---|---|---|
| CFBF root | OLE compound file | `Container` | `Decoded` | `cfb::reader`, `PidDocument.streams`, `PidPackage` | None for traversal; vendor-specific root CLSID semantics remain contextual. |
| Stream inventory | all streams | `Container` | `Decoded` | `StreamEntry`, package raw bytes | Unknown streams remain preserved, not decoded. |
| SummaryInformation | `\x05SummaryInformation` | `Metadata` | `Decoded` | `streams::summary` | DocumentSummary section 2 remains conservative for writer. |
| DocumentSummaryInformation | `\x05DocumentSummaryInformation` | `Metadata` | `Decoded` | `streams::summary` | Same as above. |
| Drawing metadata XML | `/TaggedTxtData/Drawing` | `Metadata` | `Decoded` | `DrawingMeta` | XML semantics outside known tags remain passthrough. |
| General metadata XML | `/TaggedTxtData/General` | `Metadata` | `Decoded` | `GeneralMeta` | Same as above. |

## Registry And Top-Level Stream Formats

| Name | Path / Signature | Layer | Status | Parser / Model | Gaps |
|---|---|---|---|---|---|
| `PSMroots` | magic `'root'` | `PSMIndex` | `Decoded` | `PidDocument.psm_roots` | No major current gap. |
| `PSMclustertable` | magic `'clst'` | `PSMIndex` | `PartiallyDecoded` | `PidDocument.psm_cluster_table` | Per-record SmartPlant field semantics. |
| `PSMsegmenttable` | magic `'stab'` | `PSMIndex` | `PartiallyDecoded` | `PidDocument.psm_segment_table` | Flag semantics and segment type meaning. |
| `PSMspacemap` | magic / evidence `'tseg'` | `PSMIndex` | `IdentifiedOnly` | coverage prefix only | Needs structure parser and IDA / fixture evidence. |
| `PSMcluster0` | magic `0x6C90_F544` | `Cluster` | `PartiallyDecoded` | cluster header + string table + audit-only body-chain walker (`decode_psm_cluster0_body_records`) | Body fully accounted: one continuous PSM-envelope record chain (`record_count - 2` invariant, 6/6 fixtures); record-type semantics still need IDA. See `2026-06-08-phase29-psmcluster0-leftover-triage.md`. |
| `StyleCluster` | magic `0x6C90_F544` | `Cluster` | `PartiallyDecoded` | cluster header + audit-only end-anchored tail-chain walker (`decode_style_cluster_body_records`) | Tail chain fully accounted (`Probed`); prefix characterized (12-byte opener + 532-byte constant boilerplate + 42-byte style slots) but IDA-gated, stays leftover. See `2026-06-08-phase29-stylecluster-{leftover-triage, prefix-characterization}.md`. |
| Dynamic Attributes Metadata | magic `0x6C90_F544` | `Cluster` | `PartiallyDecoded` | cluster metadata evidence | Full metadata schema. |
| Unclustered Dynamic Attributes | magic `0x6C90_F544` + DA binary records | `DynamicAttributes` | `PartiallyDecoded` | dynamic attrs, object graph, landmark trace + audit-only body-chain walker (`decode_unclustered_da_body_records`) | Body fully accounted (Phase 29 Slice C): 8-byte magic+counter prologue + single end-anchored `0x0089` envelope chain (all `Probed`; landmarks stay `Decoded`); the 31-byte "trailer" is the next record's envelope head. Record semantics and prologue counter still IDA-gated. See `2026-06-08-phase29-dynamic-attributes-body-backlog.md`. |
| `DocVersion2` | version log | `Registry` | `Decoded` | `doc_version2_decoded` | Reserved bytes remain probed. |
| `DocVersion3` | 48-byte records | `Registry` | `Decoded` | `version_history` | No major current gap. |
| `AppObject` | CLSID + UTF-16 path records | `Registry` | `Decoded` | `app_object_registry` | Filler / resync bytes are probed. |
| `JTaggedTxtStgList` | UTF-16 storage list | `Registry` | `Decoded` | `tagged_storages` | No major current gap. |
| `JSitesList` | magic `"OLEM"` + u32 count + u32 slot table | `Registry` | `PartiallyDecoded` | `parse_jsites_list` (header `Decoded`, logical table `Probed`) | Entry values match `JSite<id>` storage ids 6/6 (evidence, unnamed); stale trailing slots on `dwg0202`-family stay leftover; writer semantics IDA-gated. See `2026-06-08-phase29-jsiteslist-revision-tails.md`. |
| `TaggedTxtData/Revision` | 0-byte placeholder | `Metadata` | `IdentifiedOnly` | `revision_empty_stream` registration | Empty on 5/5 fixtures; future content would surface as leftover under the registered path. |

## Storage Prefix Formats

| Prefix | Layer | Status | Current Meaning | Gaps |
|---|---|---|---|---|
| `TaggedTxtData` | `Metadata` | `IdentifiedOnly` storage; decoded child streams | Drawing and general XML metadata. | Unknown child streams if present. |
| `JSite*` | `JSite` | `PartiallyDecoded` / `Probed` | Symbol path, GUIDs, JProperties evidence, OLE payload. | Full symbol instance model and object binding. |
| `Sheet*` | `Sheet` | mixed | Page / drawing data stream with typed, audit-only, and probe record families. | Ordinary geometry reader proof, page transform, text placement. |
| `PSMspacemap` | `PSMIndex` | `IdentifiedOnly` | Segment / page map evidence. | Full tseg parser. |

## Sheet / PSM Record Families: Current Parser Surface

| Type code | Name | Layer | Status | Parser / DTO | Evidence / Gaps |
|---:|---|---|---|---|---|
| `0x0010` | PSM sub-record / attribute fragment family | `Sheet` | `AuditOnly` | `decode_sub_records_0x0010`, `decode_attribute_fragments_0x0010` | GUID identity partially known; sub-kind discriminator unresolved. |
| `0x0018` | `igLine2d` | `Sheet` | `Decoded` | `decode_iglines`, `SheetIgLine2dDecoded` | IDA confirms type name; field layout still needs reader-level confirmation. |
| `0x0030` | `JStyleOverride` | `Sheet` | `Decoded` | `decode_jstyle_overrides`, `SheetJStyleOverrideDecoded` | Phase 16 IDA-confirmed; must not be treated as arc. |
| `0x004D` | `igTextBox` | `Sheet` | `Decoded` | `decode_igtextboxes`, `SheetIgTextBoxDecoded` | IDA confirms type/text semantics; raw offset mapping still fixture-backed. |
| `0x005E` | `igPoint2d` | `Sheet` | `Decoded` | `decode_igpoints`, `SheetIgPoint2dDecoded` | IDA confirms type name; ordinary reader not found in `radsrvitem.dll`. |
| `0x0084` | `igLineString2d` | `Sheet` | `Decoded` | `decode_iglinestrings`, `SheetIgLineString2dDecoded` | IDA confirms type name; point-list reader needs more IDA modules. |
| `0x00CE` | `igSymbol2d` | `Sheet` | `Decoded` | `decode_igsymbols`, `SheetIgSymbol2dDecoded` | IDA confirms type name; symbol reference / transform proof pending. |
| `0x00FA` | `GraphicGroup` / `GraphicPersist` | `Sheet` | `AuditOnly` | `decode_graphic_groups`, `SheetGraphicGroupDecoded` | Header and raw tail only; child/reference payload not named. |
| `0x3FE6` | `GLine2d` | `Sheet` | `Decoded` | `decode_primitive_lines`, `SheetPrimitiveLineDecoded` | SmartPlant extension wrapper; type-name evidence may live outside current mapper. |

## IDA-Confirmed Type Names Not Yet Covered By Parser

| Type code | IDA type name | Priority | Status | Next action |
|---:|---|---|---|---|
| `0x0006` | `igPointOnRelation2d` | P2 | Not covered | Determine if relation constraint only. |
| `0x000F` | `igParallelRelation2d` | P2 | Not covered | Locate relation reader and fixture frequency. |
| `0x0013` | `igBoundary2d` | P2 | Not covered | Locate reader. |
| `0x0015` | `igPerpendicularRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x0017` | `_TangentRelation2d` | P2 | Not covered | Resolve relation with `0x0085 igTangentRelation2d`. |
| `0x0019` | `igKeyPointRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x0020` | `igRectangle2d` | P1 | Not covered | High-value geometry decoder candidate. |
| `0x0021` | `igComplexString2d` | P2 | Not covered | Text / rich string investigation. |
| `0x003D` | `igSmartFrame2d` | P2 | Not covered | `sub_564464D0` candidate path. |
| `0x0040` | `igConcentricRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x0059` | `igCircle2d` | P1 | Not covered | High-value geometry decoder candidate. |
| `0x005D` | `igBSplineCurve2d` | P1 | Not covered | High-value geometry decoder candidate. |
| `0x0061` | `igArc2d` | P1 | Not covered | Do not confuse with retired `0x0030` arc interpretation. |
| `0x0063` | `igEllipse2d` | P1 | Not covered | High-value geometry decoder candidate. |
| `0x0069` | `igSymmetricRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x006A` | `igEqualRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x006B` | `igColinearRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x0077` | `igFixRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x007B` | `igGroup` | P2 | Not covered | Distinguish from `0x00FA GraphicGroup`. |
| `0x007E` | `igEllipticalArc2d` | P1 | Not covered | High-value geometry decoder candidate. |
| `0x0082` | `igHorizontalRelation2d` | P2 | Not covered | Locate relation reader. |
| `0x0085` | `igTangentRelation2d` | P2 | Not covered | Resolve relation with `_TangentRelation2d`. |
| `0x0115` | `igDimension` | P2 | Not covered | Also linked to Phase 20 parent alias evidence; handle carefully. |
| `0x0117` | `igBalloon` | P2 | Not covered | Annotation candidate. |
| `0x0118` | `igLeader` | P2 | Not covered | Annotation / leader candidate. |

## Derived Format Surfaces

| Name | Layer | Status | Description | Gaps |
|---|---|---|---|---|
| Object inventory | `Derived` | `PartiallyDecoded` | Dynamic Attributes based object list. | Record body semantics and unresolved relationship endpoints. |
| Object graph | `Derived` | `PartiallyDecoded` | Objects and relationships, including D06 fallback path. | Some relationships unresolved. |
| Cross reference graph | `Derived` | `PartiallyDecoded` | Cluster, symbol, object, endpoint provenance. | Needs unified canonical graph contract. |
| Layout model | `Derived` | `PartiallyDecoded` | Layout-first readable view. | Mixed decoded/probed/inferred sources. |
| Normalized geometry | `Derived` | `Inferred` + decoded entities | Emits decoded primitives plus inferred points/lines with provenance. | Page transform unavailable; text/symbol placement not fully proven. |
| Coordinate context | `Derived` | guardrailed | `page_dimensions_mm` available where known; `PidPageTransform` unavailable. | Need source-to-page transform record. |

## Writer And Publish Boundary

| Name | Layer | Status | Description | Gaps |
|---|---|---|---|---|
| `PidPackage` | `Writer` | `Decoded` boundary | Preserves raw stream bytes and storage metadata. | Unknown streams are intentionally passthrough. |
| `PidWriter` | `Writer` | `PartiallyDecoded` write surface | XML metadata, summary, stream replacement, sheet patch. | Semantic geometry editing not productized. |
| Round-trip / diff | `Writer` | `Decoded` boundary | Byte-level package diff and verification. | Deep semantic edit verification depends on future decoders. |
| Backup MTF extraction | `PublishAdjunct` | `Decoded` for supported envelope | Produces MDF from backup. | Not part of `.pid` binary format. |
| MDF publish XML | `PublishAdjunct` | high maturity for A01 | Rust MDF loader to `_Data.xml` / `_Meta.xml`. | DWG fixture / enrichment gates remain separate. |

## Fixture Snapshot Matrix

Generated on 2026-06-08 from the 6 local `.pid` fixtures currently present in
the working tree. Regenerated 2026-06-11 after the Phase 29 Slice C
`/Unclustered Dynamic Attributes` audit-only body-chain walker and the nested
follow-up that dispatches one-level nested `JSite*` `PSMcluster0` /
`StyleCluster` / DA twins to the same walkers (on top of the Slice B
`/PSMcluster0` and `/StyleCluster` walkers, Phase 29-B Sheet byte-audit trace
integration, Sheet common header tracing, and nested JSite
cluster-header-only tracing).

Snapshot naming convention:

- `<fixture-id>-coverage.json`
- `<fixture-id>-byte-audit.json`

Coverage summary:

| Fixture id | Coverage entries | `FullyDecoded` | `PartiallyDecoded` | `IdentifiedOnly` |
|---|---:|---:|---:|---:|
| `d06` | 26 | 7 | 6 | 13 |
| `nonascii-process-1` | 25 | 6 | 6 | 13 |
| `dwg0201` | 37 | 7 | 6 | 24 |
| `dwg0202` | 31 | 7 | 6 | 18 |
| `publish-a01` | 22 | 7 | 6 | 9 |
| `publish-dwg0202` | 31 | 7 | 6 | 18 |

Byte-audit summary:

| Fixture id | Traces | Total bytes | Consumed bytes | Leftover bytes | Coverage ratio | Unregistered paths |
|---|---:|---:|---:|---:|---:|---:|
| `d06` | 45 | 69,579 | 54,676 | 14,903 | 0.78581180 | 11 |
| `nonascii-process-1` | 44 | 211,094 | 174,374 | 36,720 | 0.82604903 | 11 |
| `dwg0201` | 58 | 223,326 | 198,351 | 24,975 | 0.88816800 | 11 |
| `dwg0202` | 42 | 206,431 | 175,856 | 30,575 | 0.85188760 | 9 |
| `publish-a01` | 61 | 63,211 | 42,051 | 21,160 | 0.66524816 | 14 |
| `publish-dwg0202` | 42 | 206,579 | 175,982 | 30,597 | 0.85188717 | 9 |

The `Traces` column counts every per-stream trace in the byte-audit JSON,
including nested JSite header-only traces.

Interpretation:

- The snapshot covers all local `.pid` fixtures currently discovered by filename.
- Sheet decoder trace integration and common header tracing improve `/Sheet6`
  coverage substantially, and the Phase 29 Slice B body-chain walkers bring
  `/PSMcluster0` to 0 leftover bytes (envelope `Decoded`, payload / prologue
  `Probed`) and `/StyleCluster` to a 12,300-byte prefix-only leftover (tail
  chain fully `Probed`) on all 6 fixtures.
- The Phase 29 Slice C walker brings `/Unclustered Dynamic Attributes` to 0
  leftover bytes on all 6 fixtures (prologue + chain `Probed`, landmark
  claims `Decoded`).
- The nested follow-up dispatches one-level nested `JSite*`
  `PSMcluster0` / `StyleCluster` / `Unclustered Dynamic Attributes` twins to
  the same walkers (probe: 23/23 streams end-anchored; nested `PSMcluster0`
  keeps the `record_count - 2` invariant on 11/11), and the Slice L
  registry dispatch reuses the top-level parsers for nested
  `PSMclustertable` / `PSMroots` / `PSMsegmenttable` / `DocVersion2/3` /
  `AppObject` / summary-pair children (probe: 68 streams, 98.4%
  consumed; nested `PSMroots` keeps the same 4-byte tail as its
  top-level twin).   The `JSite*` family leftover drops 325,843 → 66,778;
  whole-file ratios reach 0.664–0.888.
- The Slice M closeout registers `/JSitesList` (top-level + nested;
  `"OLEM"` slot table whose logical entries match `JSite<id>` storages
  6/6) and the 0-byte `/TaggedTxtData/Revision` placeholder. Distinct
  unregistered paths drop 51 → 38 (9–14 per fixture); every remaining
  multi-fixture unregistered path is IDA-gated (`PSMspacemap` pages) or
  demand-gated (`\x01Ole` payloads).
- Whole-file ratios remain below 1.0 because the format still carries many
  preserved raw streams, JSite payloads, Sheet unknowns, and audit/probe
  regions.
- These numbers are fixture-specific snapshots, not a global product coverage
  guarantee across private or unavailable customer files.

## Completion Classification

| Entry group | Classification | Reason |
|---|---|---|
| CFBF container and stream inventory | `Complete` | Traversal and raw preservation are stable. |
| Summary / DocumentSummary / TaggedTxtData XML | `Complete` | Decoded and writer-supported in narrow scopes. |
| DocVersion / AppObject / JTaggedTxtStgList | `Complete` | Typed parser and schema surfaces exist. |
| `PSMroots` | `Complete` | Decoded root table. |
| `PSMclustertable` / `PSMsegmenttable` | `NeedsIDA` | Structure exists, but field semantics and flags need stronger evidence. |
| `PSMspacemap` | `NeedsParser` + `NeedsIDA` | Identified only; `tseg` parser and semantics pending. |
| Dynamic Attributes object graph | `NeedsParser` | Object / relationship foundations exist; deep record body fields remain partial. |
| JSite storages | `NeedsParser` | Symbol evidence exists; full symbol-instance model is not complete. |
| Decoded P0 Sheet types | `NeedsIDA` | Parser exists, but ordinary field readers need IDA confirmation outside `radsrvitem.dll`. |
| P1 high-value geometry types | `NeedsParser` + `NeedsIDA` | IDA names exist; no typed decoders yet. |
| Relation / annotation IGDS types | `NeedsParser` + `NeedsFixture` | Need reader evidence and fixture frequency. |
| `0x0010` | `Blocked` | Needs JStyle/RAD host IDB for discriminator and Read/DoIO semantics. |
| `0x00FA GraphicGroup` | `Blocked` | Needs child/reference payload semantics. |
| Normalized geometry page transform | `Blocked` | Needs source-to-page transform record evidence. |
| Text placement | `Blocked` | Needs source-proven text extraction and placement evidence. |
| Writer semantic geometry edits | `Blocked` | Requires deeper decoded geometry and transform contracts. |
| Publish XML A01 | `Complete` for current delivery contract | Adjunct pipeline, not `.pid` binary format. |
| Publish XML DWG | `NeedsFixture` | Fixture / enrichment gates remain separate. |

## Promotion Rules

1. `IdentifiedOnly` to `PartiallyDecoded` requires a bounded parser surface and
   tests.
2. `PartiallyDecoded` to `Decoded` requires stable field semantics, schema /
   model exposure where relevant, and fixture or IDA-backed tests.
3. `AuditOnly` to `Decoded` requires real semantic names, not just stable byte
   layout.
4. `Inferred` geometry can be useful to downstream consumers, but must preserve
   provenance and must not imply vendor-confirmed coordinate transforms.
