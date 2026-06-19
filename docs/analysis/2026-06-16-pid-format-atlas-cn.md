# PID Format Atlas（Phase 32-A）

> 日期：2026-06-16  
> 范围：SmartPlant / Smart P&ID `.pid` CFBF 文件格式分析索引  
> 状态：planning atlas；不改变 Rust parser confidence  
> 上游计划：`docs/plans/2026-06-16-phase32-full-pid-analysis-and-file-export-plan-cn.md`
> 权威后继：`docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`

---

## 1. Confidence 词汇表

| Confidence | 含义 | 可以进入稳定 API？ | 可以写回？ |
|---|---|---:|---:|
| `Decoded` | 字段边界和语义已由 parser + fixture + byte provenance 证明，必要时有 IDA / controlled fixture 证据 | 是 | 仅限 writer 明确支持的字段 |
| `TypedAudit` | 字节 envelope 稳定，DTO 可审计，但语义字段仍保守或未命名 | 可进入 audit collection | 否 |
| `Probe` | 启发式候选、shape、window、score、text/coord hint | 只进入 probe/report | 否 |
| `IdentifiedOnly` | stream/storage 名称或 family 已识别，但 body 未解析 | 只进入 inventory | 否 |
| `Unknown` | 未识别或无稳定归类 | 只保留 raw bytes | 否 |

Phase 32 的核心约束：`Probe` / `TypedAudit` 到 `Decoded` 的升级必须同时满足 byte-range provenance、fixture ratchet、panic-safety，以及必要时 IDA reader/writer 或 controlled fixture 双证据。

---

## 2. CFBF / 顶层结构

| Path / Family | 当前状态 | Parser / Model | Evidence | 下一步 |
|---|---|---|---|---|
| Root Entry / storage tree | `Decoded` | `cfb::reader`, `cfb::tree`, `PidPackage` | round-trip writer、diff、CLSID/timestamp/state bits 保留 | 作为 bundle raw manifest 基础 |
| stream inventory | `Decoded` | `StreamEntry`, `PidPackage.streams` | `pid_inspect` report/json/schema | 加入 export bundle `raw/streams.json` |
| unknown streams | `IdentifiedOnly` / `Unknown` | `PidDocument.unknown_streams` | byte preview + magic | 进入 `audit/unknown_streams.json` |

---

## 3. 元数据 / XML / Registry

| Path / Family | 当前状态 | Parser / Model | Evidence | Blocker |
|---|---|---|---|---|
| `\005SummaryInformation` | `Decoded` | OLE property parser / summary model | report/schema/round-trip gates | writer 支持面仍需逐字段声明 |
| `\005DocumentSummaryInformation` | `Decoded` | OLE property parser | report/schema | 同上 |
| `TaggedTxtData/Drawing` | `Decoded` | tagged text XML | metadata edit + `--set-drawing-number` | XML encoding 保持 byte replacement 边界 |
| `TaggedTxtData/General` | `Decoded` | tagged text XML | generic XML tag edit | 同上 |
| `DocVersion2` | `Decoded` | doc registry parser | `OLECRT.dll::sub_1000A800` 读取两个 16-bit values | 可在 docs 引用 native evidence |
| `DocVersion3` | `Decoded` | doc registry parser | existing parser/tests | 无 |
| `AppObject` | `Decoded` | doc registry parser | CLSID/DLL registry parse | 无 |
| `JTaggedTxtStgList` | `Decoded` | doc registry parser | structure parse + report/schema | 无 |

---

## 4. JSite / OLE / Symbol 路径

| Path / Family | 当前状态 | Parser / Model | Evidence | Blocker |
|---|---|---|---|---|
| `JSite<N>/JProperties` | `Decoded` / `Probe` mix | JSite parser, symbol path, GUID scanner | symbols/GUID/text extraction | 深层 binary fields 未全命名 |
| `JSite<N>/\001Ole` | `IdentifiedOnly` | raw storage retained | `OLECRT.dll` confirms embedded OLE/symbol-object layer | 需要 direct child stream/body evidence |
| `JSitesList.entries` | `Decoded` naming/evidence, conservative field name | `JSitesListDecoded.entries` | `OLESITE.dll`: `"JSitesList"`, `"JSite"`, JOLEMembassy persistence iterates entries | JSON 字段名不改，避免 schema churn |
| `JSitesList.trailing_slots` | `TypedAudit` / conservative tail | `JSitesListDecoded.trailing_slots` | parsed as tail/stale slots | stale/delete/compact writer semantics 未证明 |
| `SymbolInformationCluster` | `IdentifiedOnly` for embedded symbol path | currently evidence doc only | `OLECRT.dll::sub_100017C0` opens wide name | 尚未作为 top-level `.pid` stream contract |

---

## 5. Dynamic Attributes / Object Graph

| Path / Family | 当前状态 | Parser / Model | Evidence | Blocker |
|---|---|---|---|---|
| `Unclustered Dynamic Attributes` records | `Decoded` for known object/relationship surface, partial body | DA parser, object inventory, object graph | DA trailer / class name / DrawingID / relationship identity tests | record body 深层字段仍有 leftover |
| DA 31-byte trailer | `Decoded` for record_id/field_x/class_id pattern | DA parser / graph builder | relationship endpoint tests | 部分 semantic family naming gated |
| `0x0089` / class 137 records | `TypedAudit` / `Probe` | DA/PSMcluster0 extraction | `radsrvitem.dll` export boundary writes `RAD_OBJECT_TYPE="137"` | human class/family name 与 head field semantics 未证明 |
| object graph | `Decoded` derived view with mixed source confidence | `object_graph`, `crossref`, `import_view` | tests + reports | canonical graph still distributed across views |

---

## 6. PSM Tables / Cluster Families

| Path / Family | 当前状态 | Parser / Model | Evidence | Blocker |
|---|---|---|---|---|
| `PSMroots` | `Decoded` | PSM roots parser | parser tests/schema | 无 |
| `PSMclustertable` | `PartiallyDecoded` | PSM table parser, byte-audit buckets | count/name/header decoded, unknown prefix audited | per-record fields / layout relationship not fully closed |
| `PSMsegmenttable` | `PartiallyDecoded` | segment flags + candidate owner mapping | 1:1 candidate owner tests | field semantics still pending |
| `PSMspacemap` handle model | analysis evidence only | docs/analysis | `radsrvitem.dll`: `(segment_id << 13) | entry_index`, capacity `0x2000` | raw page byte layout未证明 |
| `PSMcluster0` | `PartiallyDecoded` | cluster header/string table/probe | parser + byte-audit | record body deep semantics |
| `StyleCluster` | `Probe` / `TypedAudit` | prefix shape probes | style/JStyle IDA negative/positive split | prefix reader/writer missing |

---

## 7. Sheet / Geometry / PSM Record Types

| Family | 当前状态 | Parser / Model | Evidence | Blocker |
|---|---|---|---|---|
| Sheet header / chunks | `Probe` / partial | `sheet_probe`, `streams::cluster` | chunk shape, coordinate/text windows | full page transform unavailable |
| endpoint records | `Decoded` for relationship provenance, not CAD geometry | `SheetEndpointRecord`, `PidImportView.relationships` | relationship endpoint links | endpoint topology 不证明 CAD 坐标 |
| coordinate hints | `Probe` / `Inferred` | normalized geometry | f64 pair/triple gates, source notes | page transform/units 未 decoded |
| GLine2d `0x3FE6` | `Decoded` | typed PSM decoder | Phase 14 ratchets | 无 |
| `igLine2d 0x0018` | `Decoded` | typed PSM decoder | Phase 14 ratchets | 无 |
| `igLineString2d 0x0084` | `Decoded` | typed PSM decoder | Phase 14 ratchets | 无 |
| `igPoint2d 0x005E` | `Decoded` | typed PSM decoder | Phase 14 ratchets | 无 |
| `igTextBox 0x004D` | `Decoded` | typed PSM decoder | UTF-16LE decoder + ratchets | text placement/page transform remains separate |
| `igSymbol2d 0x00CE` | `Decoded` | typed PSM decoder | Phase 14 ratchets | symbol reference semantics still may need enrichment |
| `JStyleOverride 0x0030` | `Decoded` for 64-byte payload, field semantics conservative | `decode_jstyle_overrides` | `style.dll::sub_1000F030` 13 `IOContext::DoIO` calls | individual field names / StyleCluster relation |
| GraphicGroup `0x00FA` | `TypedAudit` | audit-only decoder | header + raw_variable_tail ratchet | payload child/reference list not proven |
| sub-record `0x0010` | `TypedAudit` | raw sub-record decoder + `leading_word` | Phase 18/19/20, GUID / leading_word / payload audit | discriminator/sub-kind unresolved |

---

## 8. Publish XML / MDF Parallel Pipeline

| Input / Family | 当前状态 | Parser / Model | Evidence | Blocker |
|---|---|---|---|---|
| `Export.mdf` load | mature | `publish::mdf_load` + vendored `oxidized-mdf` | tests/publish_mdf_load | GPL distribution decision |
| SQLite staging | mature | `publish::sqlite_load` | A01 / TEST02 gates | DWG enrichment fixture-dependent |
| `_Data.xml` writer | high maturity | `publish::xml_writer` | tag/interface/attr/rel parity | synthetic residual slots narrow-normalized |
| `_Meta.xml` writer | high maturity | meta parity tests | A01/DWG soft gates | DWG fixture soft-skip |
| `.pid.bundle/publish/` | planned | Phase 32 contract | MDF-backed opt-in only | must not imply `.pid` raw decode source |

---

## 9. Phase 32-D Gated Backlog

| Topic | Continue when | Negative closeout when | Forbidden shortcut |
|---|---|---|---|
| `PSMspacemap` raw page | direct stream/page reader/writer found, or controlled fixture maps bytes to handle model | all available modules lack stream reader | marking page bytes decoded from handle math alone |
| StyleCluster prefix | `StyleCluster` storage open/read/write path found | style/J2D/SPPID modules remain negative | naming constants from shape only |
| `0x0010` discriminator | branch/read loop proves sub-family discriminator | only leading_word/size histograms exist | naming `sub_kind` from partial histogram |
| GraphicGroup payload | child/reference list writer or controlled fixture proves list | only raw tail and header known | naming child OIDs from apparent integers |
| JSitesList stale tail | delete/compact writer path found | only entries iteration known | treating trailing slots as active ids |
| `0x0089` / DA heads | class/family reader evidence found | only export `RAD_OBJECT_TYPE=137` boundary exists | giving human semantic names to head fields |

---

## 10. Bundle Mapping Targets

Phase 32-B should map atlas confidence into files:

| Atlas class | Bundle destination |
|---|---|
| `Decoded` stable facts | `decoded/*.json` |
| decoded geometry entities | `geometry/decoded_entities.json` |
| `TypedAudit` collections | `geometry/audit_entities.json` or `audit/confidence_ledger.json` |
| `Probe` windows/scores/hints | `geometry/probe_entities.json` / `audit/` |
| `IdentifiedOnly` | `raw/streams.json` + `audit/unknown_streams.json` |
| `Unknown` | raw inventory only; raw bytes opt-in |

---

## 11. Immediate Next Step

Proceed to Phase 32-B:

1. Define `.pid.bundle/` manifest schema.
2. Define path escaping for stream names.
3. Define default vs opt-in outputs.
4. Define CI smoke fixture expectations.
5. Keep parser behavior unchanged until the contract is reviewed.
