# Authoritative PID Format Atlas And Confidence Ledger

> Date: 2026-06-19  
> Scope: SmartPlant / Smart P&ID `.pid` CFBF format atlas, confidence vocabulary, fixture reconciliation, and bundle confidence ledger policy  
> Status: authoritative evidence atlas for the current repository state. Phase 34-F status synchronized on 2026-07-10. This document does not claim the vendor PID format is fully decoded.

## Canonical Confidence Vocabulary

| Confidence class | Meaning | Read/export eligibility | Write eligibility |
|---|---|---|---|
| `Decoded` | Field boundaries and semantics are proven by parser output, bounded byte provenance, fixture ratchets, panic-safety coverage, and IDA or controlled fixture evidence where semantics require it. | May appear in stable decoded DTOs, schema, and decoded bundle files. | Only writable when a separate writer policy explicitly allows the exact surface. |
| `TypedAudit` | Stable byte envelope or DTO exists for audit, but semantic fields remain conservative, positional, or unnamed. | May appear in audit collections and confidence ledger rows. | Not writable. |
| `Probe` | Heuristic candidate, score, window, text hint, coordinate hint, or investigation-only output. | May appear in probe reports or probe bundle files. | Not writable. |
| `IdentifiedOnly` | Stream, storage, record family, or source domain is recognized, but the body is not structurally decoded. | May appear in raw inventories and unknown/identified audits. | Raw passthrough or explicit raw replacement only. No semantic write support. |
| `Unknown` | Unrecognized bytes, unmatched inventory item, or no stable classification. | Preserve in raw inventory. | Not writable except whole-stream byte replacement by an explicit caller-owned plan. |

Promotion to `Decoded` requires all of the following:

1. stream path and bounded byte range;
2. record kind or family plus fixture identity;
3. fixture ratchet with counts, distributions, or representative values;
4. panic-safety coverage for byte parser entry points;
5. IDA reader/writer evidence or controlled fixture evidence when semantics or writing are being named.

No atlas row may use "complete", "done", "mature", or "high confidence" without mapping the row to one of the five classes above.

## Source-Domain Separation

| Source domain | What it can prove | What it cannot prove |
|---|---|---|
| Raw `.pid` CFBF evidence | Container tree, stream inventory, byte ranges, parser traces, fixture ratchets, raw passthrough boundaries. | Native writer intent, unless paired with controlled fixture or IDA writer evidence. |
| IDA evidence | Native names, reader/writer loops, `IOContext::DoIO` call scope, positive and negative persistence evidence. | Fixture frequency or raw `.pid` coverage by itself. |
| Controlled fixture evidence | Operation-to-byte deltas and writer behavior for a named action. | Global vendor semantics without cross-fixture and parser ratchets. |
| MDF publish evidence | `_Data.xml` / `_Meta.xml` generation, MDF table coverage, publish parity, writer XML output shape. | Raw `.pid` Sheet, PSM, JSite, or Dynamic Attributes byte-layout decoding. |

MDF publish parity is a `PublishAdjunct` signal only. It must not be cited as proof that raw `.pid` Sheet, PSM, JSite, or Dynamic Attributes bytes are decoded.

## Atlas Rows

Every row below records status, parser/model reference, evidence, blocker, and writer policy.

| Atlas row | Source domain | Status | Parser / model reference | Evidence | Blocker | Writer policy |
|---|---|---|---|---|---|---|
| CFBF root and storage tree | Raw `.pid` | `Decoded` | `cfb::reader`, `cfb::tree`, `PidPackage` | CFB traversal, package parse, round-trip and diff surfaces | Vendor-specific root CLSID meaning is contextual | Passthrough round-trip and raw stream replacement only |
| Stream inventory | Raw `.pid` | `Decoded` | `StreamEntry`, `PidPackage.streams`, bundle `raw/streams.json` | `pid_inspect` report/JSON, export bundle raw index | Stream identity does not decode stream body | Inventory writable only as a consequence of package writer or raw replacement |
| Unknown stream payloads | Raw `.pid` | `Unknown` | `PidDocument.unknown_streams`, byte audit unregistered paths | Byte preview, magic, unregistered path lists | No stable parser or family identity | Preserve raw bytes; no semantic write |
| SummaryInformation | Raw `.pid` | `Decoded` | `streams::summary`, summary model | OLE property parser, tests, report/schema output | Non-string property writer details remain narrow | Summary string properties only where writer supports them |
| DocumentSummaryInformation | Raw `.pid` | `Decoded` | `streams::summary`, summary model | OLE property parser, tests, report/schema output | Section-specific non-string semantics are conservative | Summary string properties only where writer supports them |
| TaggedTxtData Drawing XML | Raw `.pid` | `Decoded` | tagged text XML parser, `DrawingMeta` | metadata edit and `--set-drawing-number` gates | XML encoding is byte replacement bounded | `set_xml_tag` / drawing-number edit only |
| TaggedTxtData General XML | Raw `.pid` | `Decoded` | tagged text XML parser, `GeneralMeta` | generic XML tag edit gate | XML encoding is byte replacement bounded | `set_xml_tag` only |
| TaggedTxtData storage shell | Raw `.pid` | `IdentifiedOnly` | `streams::tagged_text` storage detection | D06 inventory marks `TaggedTxtData` storage prefix with child-stream ownership | Unknown child streams if future fixtures add them | Preserve storage and child streams |
| DocVersion2 | Raw `.pid` + IDA | `Decoded` | `parsers::doc_version2`, `doc_version2_decoded` | Parser tests plus `OLECRT.dll::sub_1000A800` two 16-bit value evidence | Reserved bytes remain conservative | Read-only |
| DocVersion3 | Raw `.pid` | `Decoded` | `streams::version_history`, `version_history` | 48-byte record parser and fixture/report output | None currently known for read surface | Read-only |
| AppObject registry | Raw `.pid` | `Decoded` | `streams::app_object`, `app_object_registry` | CLSID/DLL registry parse and fixture output | Filler/resync bytes remain conservative | Read-only |
| JTaggedTxtStgList | Raw `.pid` | `Decoded` | `streams::tagged_txt_list`, `tagged_storages` | UTF-16 storage list parser and report/schema output | None currently known for read surface | Read-only |
| JSite storage prefix | Raw `.pid` | `IdentifiedOnly` | `streams::jsite`, `PidDocument.jsites` | Storage prefix inventory, nested JSite package inventory | Child stream ownership and symbol-instance semantics are not complete | Preserve storage and children |
| JSite JProperties symbol/GUID hints | Raw `.pid` | `Probe` | `parsers::jproperties`, symbol path extraction, GUID scanner | Symbol path and GUID scans across fixtures | Deep binary fields are not named | Read-only, no semantic write |
| JSite `\001Ole` payload | Raw `.pid` + IDA | `IdentifiedOnly` | raw child stream preservation | `OLECRT.dll` confirms embedded OLE/symbol-object layer | Direct child stream/body evidence is missing | Preserve raw bytes only |
| JSitesList entries | Raw `.pid` + IDA | `Decoded` | `parsers::jsites_list`, `JSitesListDecoded.entries` | `OLESITE.dll` names `"JSitesList"` and `"JSite"` and iterates count-bounded entries | Field naming stays conservative to avoid schema churn | Read-only |
| JSitesList trailing slots | Raw `.pid` + IDA | `TypedAudit` | `JSitesListDecoded.trailing_slots` | Tail/stale slot parse and byte-audit ranges | Stale/delete/compact writer semantics are not proven | Read-only |
| Dynamic Attributes known object and relationship surface | Raw `.pid` | `Decoded` | `streams::dynamic_attrs`, object inventory, object graph, crossref, import view | DA trailer/class/DrawingID/relationship tests, D06 object graph ratchet | Deeper record bodies remain mixed | Read-only semantic surface |
| Dynamic Attributes record envelopes and body chain | Raw `.pid` | `TypedAudit` | `decode_unclustered_da_body_records`, byte audit | Phase 29 body-chain walker, end-anchored chain, payload kept raw | `0x0089` semantic family and prologue counter semantics are IDA-gated | Read-only |
| `0x0089` / class 137 records | Raw `.pid` + IDA | `TypedAudit` | DA/PSMcluster0 extraction | `radsrvitem.dll` export boundary writes `RAD_OBJECT_TYPE = "137"` | Human class/family name and head field semantics are not proven | Read-only |
| Derived object graph and cross-reference views | Raw `.pid` derived | `Decoded` | `object_graph`, `crossref`, `import_view` | Tests and reports, including D06 relationship fallback | Source rows feeding the graph may be lower confidence | Export-only, no semantic write |
| PSMroots | Raw `.pid` | `Decoded` | `streams::psm_tables`, `PidDocument.psm_roots` | Parser tests/schema and fixture inventory | None currently known for read surface | Read-only |
| PSMclustertable | Raw `.pid` | `TypedAudit` | `streams::psm_tables`, `psm_cluster_table` | Count/name/header parse and byte-audit buckets | Per-record SmartPlant field semantics remain open | Read-only |
| PSMsegmenttable | Raw `.pid` | `TypedAudit` | `streams::psm_tables`, `psm_segment_table` | Segment flags and owner-candidate mapping | Flag and segment type semantics remain open | Read-only |
| PSMspacemap storage | Raw `.pid` + IDA | `IdentifiedOnly` | coverage prefix, raw child pages | `radsrvitem.dll` handle model `(segment_id << 13) | entry_index` | Raw page byte layout is not decoded | Preserve raw bytes |
| PSMspacemap page bytes | Raw `.pid` + IDA | `Unknown` | byte-audit unregistered paths | D06 unmatched page paths such as `/PSMspacemap/0x00000000` | Direct stream/page reader or controlled fixture map is missing | No semantic write |
| PSMcluster0 envelope/string surfaces | Raw `.pid` | `TypedAudit` | `streams::cluster`, `decode_psm_cluster0_body_records` | Header/string table and record-chain accounting | Payload semantics remain IDA-gated | Read-only |
| StyleCluster tail chain | Raw `.pid` | `TypedAudit` | `streams::cluster`, `decode_style_cluster_body_records` | End-anchored tail-chain walker | Prefix reader/writer is missing | Read-only |
| StyleCluster prefix | Raw `.pid` + IDA | `Probe` | prefix shape probes | Prefix characterization docs and negative/positive style/JStyle split | Native reader/writer path is missing | Read-only |
| Sheet stream shell and common chunks | Raw `.pid` | `Probe` | `sheet_probe`, `streams::cluster` | Chunk shape, text windows, coordinate windows | Full page transform and unknown record inventory remain open | Read-only |
| Sheet endpoint records | Raw `.pid` | `Decoded` | `SheetEndpointRecord`, `PidImportView.relationships` | Relationship endpoint links and tests | Endpoint topology does not prove CAD geometry | Read-only |
| GLine2d `0x3FE6` | Raw `.pid` | `Decoded` | `decode_primitive_lines`, `SheetPrimitiveLineDecoded` | Phase 14 cross-fixture ratchets and byte provenance | Semantic writer is not proven | Export decoded geometry only |
| igLine2d `0x0018` | Raw `.pid` | `Decoded` | `decode_iglines`, `SheetIgLine2dDecoded` | Phase 14 ratchets and byte provenance | Ordinary native reader confirmation can still enrich names | Export decoded geometry only |
| igLineString2d `0x0084` | Raw `.pid` | `Decoded` | `decode_iglinestrings`, `SheetIgLineString2dDecoded` | Phase 14 ratchets and byte provenance | Native point-list reader evidence can still enrich names | Export decoded geometry only |
| igPoint2d `0x005E` | Raw `.pid` | `Decoded` | `decode_igpoints`, `SheetIgPoint2dDecoded` | Phase 14 ratchets and byte provenance | Native reader evidence can still enrich names | Export decoded geometry only |
| igTextBox `0x004D` | Raw `.pid` | `Decoded` | `decode_igtextboxes`, `SheetIgTextBoxDecoded` | UTF-16LE parser, Phase 14 ratchets, byte provenance | Text placement and page transform remain separate | Export decoded text entity only |
| igSymbol2d `0x00CE` | Raw `.pid` | `Decoded` | `decode_igsymbols`, `SheetIgSymbol2dDecoded` | Phase 14 ratchets and byte provenance | Symbol reference/transform enrichment remains open | Export decoded symbol entity only |
| JStyleOverride `0x0030` | Raw `.pid` + IDA | `Decoded` | `decode_jstyle_overrides`, `SheetJStyleOverrideDecoded` | `style.dll::sub_1000F030` with 13 `IOContext::DoIO` calls and 64-byte payload | Individual field names and StyleCluster relation remain conservative | Read-only audit/decoded style record, not geometry write |
| igBoundary2d `0x0013` | Raw `.pid` | `Decoded` typed association; audit-only geometry policy | `decode_igboundaries`, `SheetIgBoundary2dDecoded`, `SheetGeometry.decoded_igboundaries` | Phase 34-D pins every byte on 20/20 records: `segment_count` groups of `0x67 + 4×f64`, `btf == 49 + 41n`, anchor and member trailer; 60/60 member OIDs resolve to same-stream `igLine2d` records with matching geometry | Current fixtures contain only boundaries that re-list member-line geometry; independent boundaries without resolvable members have not been observed | Export the typed association in decoded Sheet DTOs; do not emit a duplicate normalized polyline and do not write |
| igRectangle2d `0x0020` | Raw `.pid` | `IdentifiedOnly` family; body `Probe` | no production decoder; `probe_psm_undecoded_shapes` | Four records across three fixture paths, all in `/Sheet6615` or nested `/JSite204/Sheet6`; relaxed neighbor correlation completed | Two ownership families give contradictory field meanings and no stable two-extent rectangle layout; top-level projection is unproven | Preserve/audit only; no normalized geometry or writer surface |
| igSmartFrame2d `0x003D` | Raw `.pid` | `IdentifiedOnly` family; body `Probe` | no production decoder; Phase 34-C evidence note | Twelve records across all six fixtures, with page-frame-like scalars and no line-neighbor geometry correlation | Native reader and bounded field semantics are missing; page-size-like scalars do not prove a page transform | Structural evidence only; no drawable geometry or writer surface |
| Curve families `0x0059` / `0x0061` / `0x0063` / `0x007E` / `0x005D` | Raw `.pid` / `.sym` corpus | `IdentifiedOnly`; local evidence available | `probe_curve_family_corpus_scan`; no production decoder | Phase 34-E finds circle/arc/elliptical-arc/B-spline records in registered nested JSite streams and all five families in the backup `.sym` library (`616/279/44/50/55` records respectively) | Representative `.sym` fixtures must be extracted and each family still needs bounded field probes, validation rules, ratchets, schema and panic-safety; nested JSite ownership projection remains unproven | Read-only corpus evidence; no geometry emission or write support |
| GraphicGroup `0x00FA` | Raw `.pid` | `TypedAudit` | `decode_graphic_groups`, `SheetGraphicGroupDecoded` | Header plus raw variable tail ratchet | Payload child/reference semantics are not proven | Read-only audit |
| Sub-record `0x0010` | Raw `.pid` | `TypedAudit` | `decode_sub_records_0x0010`, `decode_attribute_fragments_0x0010` | Phase 18/19/20 GUID, leading-word, and payload audit | Discriminator/sub-kind is unresolved. `leading_word` is positional only | Read-only audit |
| Coordinate hints and inferred geometry | Raw `.pid` derived | `Probe` | normalized geometry, f64 pair/triple gates | Source notes and guardrailed coordinate context | Page transform, units, origin, scale, and bounds are not proven | Read-only probe/audit |
| Page transform | Raw `.pid` + controlled fixture pending | `Unknown` | `PidPageTransform::Unavailable` | Negative atlas and roadmap evidence | Source coordinate space, units, direction, origin, scale, bounds, and provenance are missing | Not writable |
| MDF load and SQLite staging | MDF publish | `Decoded` | `publish::mdf_load`, vendored `oxidized-mdf`, `publish::sqlite_load` | `publish_mdf_load` tests and A01/DWG publish gates | GPL distribution decision, DWG fixture availability | Publish-only, not `.pid` writer |
| `_Data.xml` writer | MDF publish | `Decoded` | `publish::xml_writer`, `PublishDrawing` | tag/interface/attribute/relationship parity gates | Narrow synthetic residual slots remain normalized | Produce publish XML only from explicit MDF/SQLite input |
| `_Meta.xml` writer | MDF publish | `Decoded` | `write_meta_xml` | Meta parity tests | DWG fixture soft-skip when absent | Produce publish XML only from explicit MDF/SQLite input |
| Bundle publish subtree | MDF publish | `IdentifiedOnly` until requested, `Decoded` for status/XML when run | `ExportBundlePublishPlan`, `export_bundle_publish_xml` | Manifest records publish input identity separately | No publish files without explicit publish input | Optional `publish/` subtree only |

## Real Fixture Inventory Reconciliation: `test-file/D06.pid`

Reconciliation source: `docs/specs/2026-06-08-pid-file-format-spec-kit/d06-coverage.json` and `d06-byte-audit.json`, generated from the repository's fixture inventory and byte-audit flow. D06 is a compact real PID fixture with 26 coverage entries and 45 byte-audit traces.

Coverage summary from the snapshot:

| D06 snapshot bucket | Count | Atlas reconciliation |
|---|---:|---|
| `FullyDecoded` | 7 | Mapped to `Decoded` rows for SummaryInformation, DocumentSummaryInformation, AppObject, DocVersion2, DocVersion3, JTaggedTxtStgList, and PSMroots. |
| `PartiallyDecoded` | 6 | Mapped to `TypedAudit` rows for Dynamic Attributes Metadata, PSMcluster0, PSMclustertable, PSMsegmenttable, StyleCluster, and Unclustered Dynamic Attributes. Decoded sub-surfaces are called out separately where proven. |
| `IdentifiedOnly` | 13 | Mapped to `IdentifiedOnly` rows for JSite storages, JSitesList storage shell, PSMspacemap storage, Sheet6 storage shell, and TaggedTxtData storage shell. |

Detailed reconciliation:

| D06 inventory item or byte-audit family | Snapshot status | Atlas row | Atlas class | Unmatched handling |
|---|---|---|---|---|
| `\005SummaryInformation`, `\005DocumentSummaryInformation` | `FullyDecoded` | SummaryInformation / DocumentSummaryInformation | `Decoded` | none |
| `AppObject`, `DocVersion2`, `DocVersion3`, `JTaggedTxtStgList` | `FullyDecoded` | registry rows | `Decoded` | none |
| `PSMroots` | `FullyDecoded` | PSMroots | `Decoded` | none |
| `Dynamic Attributes Metadata` | `PartiallyDecoded` | Dynamic Attributes metadata/envelope surfaces | `TypedAudit` | Deep schema bytes remain blockers, not omitted |
| `Unclustered Dynamic Attributes` | `PartiallyDecoded` | DA known object/relationship surface plus DA body chain | `Decoded` for known landmarks, `TypedAudit` for body chain | `0x0089` semantic names remain `TypedAudit` blockers |
| `PSMcluster0` | `PartiallyDecoded` | PSMcluster0 envelope/string surfaces | `TypedAudit` | Payload semantics remain blockers |
| `PSMclustertable`, `PSMsegmenttable` | `PartiallyDecoded` | PSM table rows | `TypedAudit` | Field semantics remain blockers |
| `StyleCluster` | `PartiallyDecoded` | StyleCluster tail chain and prefix rows | `TypedAudit` / `Probe` | Prefix remains `Probe` |
| `JSite145`, `JSite151`, `JSite1835`, `JSite204`, `JSite23`, `JSite2605`, `JSite300`, `JSite336`, `JSite3644` | `IdentifiedOnly` | JSite storage prefix / JProperties rows | `IdentifiedOnly` for storage, `Probe` for symbol/GUID hints | Deep child payloads are not silently omitted |
| `JSitesList` storage and `/JSitesList` trace | storage `IdentifiedOnly`, trace parsed | JSitesList entries and trailing slots | `Decoded` for entries, `TypedAudit` for trailing slots | Any stale/tail slots stay `TypedAudit` |
| `PSMspacemap` storage and page paths such as `/PSMspacemap/0x00000000` | storage `IdentifiedOnly`, page paths unregistered | PSMspacemap storage and page bytes | `IdentifiedOnly` for storage, `Unknown` for page bytes | Every page body remains explicit `Unknown` until direct page reader evidence exists |
| `Sheet6` storage and `/Sheet6` byte-audit trace | storage `IdentifiedOnly`, trace partially consumed | Sheet shell, decoded Sheet records, audit/probe Sheet rows | `IdentifiedOnly` for shell, `Decoded` / `TypedAudit` / `Probe` for record families | D06 leftover Sheet windows remain `Unknown` or `Probe`, never silently promoted |
| `TaggedTxtData` storage | `IdentifiedOnly` | TaggedTxtData storage shell | `IdentifiedOnly` | Unknown future children remain inventory-only |

Byte-audit reconciliation highlights:

- D06 snapshot: 45 traces, 69,579 total bytes, 54,676 consumed bytes, 14,903 leftover bytes, coverage ratio `0.78581180`, 11 unregistered paths.
- The listed unregistered `PSMspacemap` page paths are reconciled to `Unknown` page bytes, not hidden.
- `/Sheet6` leftover ranges are reconciled to Sheet shell/probe/unknown blockers unless they match one of the decoded or typed-audit record families.

## Confidence Ledger Reconciliation

The bundle confidence ledger is the cross-reference between exported files, this atlas, parser roadmap rows, and writer/export policy. Current code surface: `src/export_bundle.rs` writes `audit/confidence_ledger.json` with `source_path`, `bundle_path`, `family`, `confidence`, `evidence`, `blockers`, and `summary`.

| Bundle output | Source path/domain | Family | Ledger confidence | Atlas rows | Parser roadmap impact | Writer/export policy |
|---|---|---|---|---|---|---|
| `raw/streams.json` | `/` raw `.pid` | CFBF stream inventory | `Decoded` | CFBF root and stream inventory | none | inventory only |
| `raw/streams/*.bin` | raw CFB streams | raw stream bytes | `IdentifiedOnly` | Unknown stream payloads, stream inventory | unknown bytes stay backlog candidates | opt-in only, caller-owned bytes |
| `decoded/document.json` | `/` raw `.pid` aggregate | `PidDocument` aggregate | `IdentifiedOnly` aggregate | all atlas rows by child field | prevents aggregate file from overclaiming `Decoded` | export-only unless a child writer policy allows edits |
| `decoded/*.json` | decoded split views | metadata, structure, sheet, PSM, graph projections | `IdentifiedOnly` aggregate | child rows in this atlas | directs future parser promotions to row-specific gates | export-only |
| `decoded/sheets.json` `decoded_igboundaries` field | top-level `/Sheet*` `0x0013` records | typed igBoundary2d association | `Decoded` | igBoundary2d atlas row and Phase 34-D 20-record / 60-member ratchet | association is decoded, but duplicate drawable emission is intentionally suppressed | export-only; no semantic Sheet write-back |
| `geometry/decoded_entities.json` | `/Sheet*` decoded PSM records | GLine2d, igLine2d, igLineString2d, igPoint2d, igTextBox, igSymbol2d | `Decoded` | decoded Sheet record rows | future coverage can add new decoded families only through promotion gate | no semantic Sheet write-back |
| `geometry/audit_entities.json` | `/Sheet*` inferred/audit records | typed audit and inferred entities | `TypedAudit` | GraphicGroup, `0x0010`, coordinate hints | roadmap owns discriminator/payload/page-transform blockers | read-only |
| `geometry/probe_entities.json` | `/Sheet*` probe windows | heuristic geometry evidence | `Probe` | Sheet shell, coordinate hints, unknown Sheet windows | roadmap owns unknown-record inventory | read-only |
| `audit/confidence_ledger.json` | ledger artifact | confidence ledger | `Decoded` | this section | keeps outputs reconciled | read-only report |
| `writer/*.json` | writer policy | writer guidance | `Decoded` | writer policies in atlas rows | does not widen parser confidence | only whitelisted XML/Summary/raw replacement surfaces |
| `publish/status.json`, `publish/data.xml`, `publish/meta.xml` | explicit MDF/SQLite publish input | MDF publish | `Decoded` for status/XML generated by publish pipeline | MDF publish rows | no parser promotion for raw `.pid` | optional publish subtree, MDF-backed only |

The ledger never uses `Mixed` or `Raw` as confidence classes. Aggregates that contain multiple classes are represented as `IdentifiedOnly` at the file level and must be interpreted through their child atlas rows.

## Explicit Unknowns And Blockers

The current atlas has non-decoded rows, so the vendor PID format is not fully decoded. Open blockers:

| Blocker | Current class | Why it remains blocked |
|---|---|---|
| `PSMspacemap` raw page layout | `Unknown` for page bytes | IDA only proves handle math, not on-disk page layout. |
| `StyleCluster` prefix | `Probe` | Shape is characterized but native reader/writer evidence is missing. |
| `0x0010` discriminator | `TypedAudit` | `leading_word` is positional evidence, not a semantic sub-kind. |
| GraphicGroup `0x00FA` payload | `TypedAudit` | Header and tail are stable, but child/reference semantics are not proven. |
| JSitesList trailing slots | `TypedAudit` | Delete/compact/stale-slot writer semantics are not proven. |
| `0x0089` / class 137 DA heads | `TypedAudit` | Export boundary exists, but family name and head field semantics are not proven. |
| Page transform and coordinate units | `Unknown` | Source coordinate space, units, origin, direction, scale, bounds, and byte provenance are not all established. |
| Curve-family field layouts | `IdentifiedOnly` | Local records now exist, but circle/arc/ellipse/elliptical-arc/B-spline payload fields have not passed per-family decoder gates. |
| Nested JSite geometry ownership | `IdentifiedOnly` | Nested records are valid byte-layout evidence sources, but their projection into top-level drawing geometry is not proven. |
| Semantic Sheet writer | `Unknown` | Reader confidence and geometry JSON do not prove native write-back. |

The 2026-06-21 live `ida-pro-mcp` refresh in
`docs/analysis/2026-06-19-ida-evidence-baseline.md` rechecked the currently
reachable `sppid.dll` and `core.dll` instances. It found no direct raw
SmartPlant `.pid` stream reader/writer evidence for these blockers, so the
classes above stay unchanged.

Forbidden shortcuts:

- Do not infer `PSMspacemap` raw page layout from handle math alone.
- Do not name `0x0010.leading_word` as `sub_kind`.
- Do not treat GraphicGroup tail integers as child OIDs without proof.
- Do not treat JSitesList trailing slots as active IDs without delete/compact writer evidence.
- Do not make page transforms available from template dimensions or f64 hints alone.
- Do not emit `igBoundary2d` as a second polyline when its member references
  resolve to already-emitted `igLine2d` geometry.
- Do not treat a valid curve record in nested JSite storage as proof that it
  belongs in top-level normalized drawing geometry.
- Do not derive writer support from read support.
- Do not use MDF publish parity as raw `.pid` decode evidence.

## Evidence References

- Current IDA baseline, including the 2026-06-21 live MCP refresh:
  `docs/analysis/2026-06-19-ida-evidence-baseline.md`.
- Historical `JStyleOverride` IDA scope: `docs/analysis/2026-06-12-phase30-style-dll-jstyleoverride-ida.md`.
- Historical JSitesList IDA scope: `docs/analysis/2026-06-12-phase30-olesite-jsiteslist-ida.md`.
- Historical OLE/DocVersion2 scope: `docs/analysis/2026-06-13-phase31-olecrt-storage-entrypoints.md`.
- Historical `PSMspacemap` and `0x0089` scope: `docs/analysis/2026-06-12-phase30-radsrvitem-record-spacemap-ida.md`.
- Phase 34-D igBoundary2d grammar and typed association:
  `docs/analysis/2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md`.
- Phase 34-E local curve-family corpus evidence:
  `docs/analysis/2026-07-07-phase34e-missing-geometry-fixture-plan.md`.
- Fixture inventory source: `docs/specs/2026-06-08-pid-file-format-spec-kit/d06-coverage.json` and `d06-byte-audit.json`.
- Bundle contract: `docs/pid-export-bundle-contract.md`.
