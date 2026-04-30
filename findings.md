# 发现与决策：PID 解析开发方案

## 当前支持范围
- 项目定位：Rust 实现的 SmartPlant / Smart P&ID `.pid` 文件解析器，不是通用 DWG/DXF/PDF P&ID 解析器。
- 公共入口：
  - `PidParser::parse_file(path)`：输出结构化 `PidDocument`。
  - `PidParser::parse_package(path)`：输出带 raw stream 的 `PidPackage`，用于 writer / diff / round-trip。
  - `PidPackage::from_bytes(bytes)`：支持内存字节解析。
- CLI：
  - `pid_inspect`：报告、JSON、schema、coverage、byte-audit、probe、mermaid、round-trip、diff。
  - `pid_backup_extract`：从 SmartPlant backup 剥离 MTF envelope 得到 MDF。
  - `pid_publish_xml`：从 MDF / legacy sqlite 生成 `_Data.xml` / `_Meta.xml`。

## 稳定能力
- CFB/OLE 容器读取、树结构与 stream inventory。
- OLE Summary / DocumentSummary。
- `TaggedTxtData/Drawing` 与 `TaggedTxtData/General` XML。
- `PSMroots`。
- `DocVersion2` / `DocVersion3`。
- `AppObject`。
- `JTaggedTxtStgList`。
- JSite 符号路径、GUID、JProperties 文本线索。
- Dynamic Attributes 对象清单、对象关系图基础能力。
- Cross-reference graph 与 layout-first 可读模型。
- Writer passthrough round-trip、metadata XML、Summary、任意 stream replacement、experimental Sheet patch。

## Partial / Probe 能力
- `SheetGeometry` / `SheetText` / `SheetEndpoint` / `SheetCoordinateHintDto` 已作为 schema DTO 入口落地到 `SheetStream.geometry`，当前是稳定 contract surface，不代表完整 CAD geometry decoded；`sheet_probe` 的 text runs、coordinate hints 与 endpoint records 已归一化填充。
- Sheet geometry synthetic 回归确认 endpoint 同步会保留先前 probe 归一化出的 text 与 coordinate hint，不会覆盖整个 `SheetGeometry`。
- Phase 3 当前 DTO 起步范围已完成：Sheet text、coordinate hint、endpoint 三类证据进入稳定 DTO；未命名字节仍停留在 probe 层，不升级为 decoded。
- Phase 4 已开始：`PidImportView.relationships` 从 cross-reference endpoint links 带出 sheet path、sheet offset 与 source/target `field_x`，作为 canonical edge 的轻量 provenance。
- `PSMclustertable`、`PSMsegmenttable`：已部分结构化，但 record 字段语义与关联关系未完全闭环。
- `PSMclustertable` decoded record 已暴露候选字段之外的 `unknown_prefix_bytes`，便于真实 fixture 横向对比，避免把保留位/未知常量误命名为稳定语义。
- `PSMsegmenttable` entry 已暴露保守候选 owner cluster index/name；只有 segment entry 数量与 cluster table entry 数量完全一致时才填充，数量不一致或 cluster table 缺失时保持 `None`。
- 文本报告会在 segment 行显示 `owner_candidate=index:name`，同时保留 probe `owner_hint`，用于区分结构化候选字段与旧 probe 线索。
- Coverage inventory 对 `PSMsegmenttable` 的说明已更新为 `segment flags + owner candidate mapping; SmartPlant field semantics still pending`，保持 partial decoded 评级。
- JSON schema 已通过回归测试确认包含 `PsmSegmentEntry`、`candidate_owner_cluster_index` 和 `candidate_owner_cluster_name`。
- 真实 fixture soft-skip 测试已扩展：当 segment/cluster entry 数量 1:1 时，结构化 candidate owner 必须与旧 probe `owner_cluster_hint` 的顺序映射一致；数量不一致时二者都必须为空。
- Byte-audit aggregate 已通过 synthetic `/PSMclustertable` 测试锁住 decoded/probed/leftover 分桶：header/name 为 decoded，record prefix 为 probed，trailing garbage 为 leftover。
- Candidate 字段不升级为 `TraceConfidence::Decoded`：虽然 `decoded_records` 暴露了稳定字节布局候选，但 SmartPlant 业务语义仍未完全证明；byte-audit 中 prefix 继续整体归入 `Probed`，避免 coverage 指标误导下游。
- Phase 2 当前执行范围已完成：PSM cluster unknown prefix、PSM segment owner candidate、report、coverage、schema、真实 fixture、byte-audit aggregate 与 confidence 决策均有回归覆盖。
- `PSMcluster0`、`StyleCluster`、`Dynamic Attributes Metadata`：公共 header / string table / 部分探测已具备。
- `Unclustered Dynamic Attributes`：对象/关系基础可用，但 record body 深层字段仍有 leftover。
- `Sheet*`：已能识别 text run、endpoint record、coordinate hint；完整图元、几何、标注语义未完成。

## 开发优先级判断
1. PSM table 补齐是高优先级，因为它能提升 cluster/segment 权威索引，并直接服务 crossref 与 layout。
2. Sheet geometry 是第二优先级，因为它决定下游 CAD “可读整图”的质量上限。
3. canonical semantic graph 应在 PSM / Sheet 事实层更稳后再收敛，否则会把 probe 结果固化进 API。
4. Publish XML A01 主线应保持现有 parity gates，DWG 侧作为独立闭环推进。

## 风险
- 真实 `.pid` / MDF fixture 可能私有，测试会 soft-skip；需要明确哪些门禁是 hard gate，哪些是 local-only gate。
- `vendor/oxidized-mdf` 为 GPL-3.0；对外分发二进制时需要合规方案。
- Sheet 深层结构仍处逆向阶段，短期不应承诺完整几何编辑。
- `PidDocument` 字段变更会影响 JSON schema，必须同步 schema/golden 测试。

## 关键文件
- `README.md`
- `docs/prd-pid-parse-current-state.md`
- `docs/sppid/v0.10.x-status.md`
- `docs/architecture-guide.md`
- `src/api.rs`
- `src/cfb/reader.rs`
- `src/model.rs`
- `src/streams.rs`
- `src/import_view.rs`
- `src/bin/pid_inspect.rs`
- `src/bin/pid_publish_xml.rs`

## H7CAD PID 真实几何显示最新结论
- H7CAD 当前可安全显示 PID 中的 `Inferred Point`，来源是 Sheet coordinate hints，并保留 byte provenance。
- 当前不应渲染 endpoint line：endpoint records 只证明 relationship/object `field_x` 语义连接，不证明 CAD 坐标。
- `/Sheet6` object-coordinate mapping 经过 field-x window、stable chunk shape、stable marker、coordinate-quality filters 后仍无 promotable candidate，最终 feature report 保持 `max_score=45`、`promotable=0`。
- `GraphicIdentityNearby` 路线已进一步验证：
  - identity report：`fields=57`、`windows=6025`、`identities=425`、`same_object=11`、`wrong_object=414`。
  - identity scoring：`identity_supported=0`、`max_score=45`、`over_threshold=0`。
  - 结论：same-object identity 有真实信号，但没有与非端点 feature scoring candidate 相交，仍不能填充 `SheetObjectGeometryHint`。
- PR 拆分建议：
  - PR1：normalized geometry contract。
  - PR2：H7CAD inferred point rendering。
  - PR3：Sheet6 evidence guardrails + `SheetObjectGeometryHint` 空基线。
  - PR4：field-x window / feature / coordinate-quality investigation。
  - PR5：GraphicIdentityNearby identity index / scanner / scoring investigation。

## Text placement 证据路线结论
- `Text placement` 已作为 line 之前的低风险路线推进：先调查 text run 与 nearby coordinate，不改变 H7CAD 行为。
- Phase A 已实现 `sheet_text_window_candidates`，只输出 investigation-only candidate；`/Sheet6` report：
  - `text_runs=9`
  - `coordinates=64`
  - `candidates=121`
  - `same_chunk=25`
  - `coordinate_quality_passed=2`
- Phase B 已实现 text-quality filter 与 scoring；收紧后 `/Sheet6` report：
  - `text_quality_passed=0`
  - `max_score=-50`
  - `over_threshold=0`
  - normalized geometry 仍无 `PidGraphicKind::Text`
- 关键风险：当前 top text run 多为疑似二进制误识别的 CJK/韩文字符串；`" 060101럀"` 这类“数字 + Hangul 尾字”已被 filter 拒绝。
- 结论：当前 `/Sheet6` 仍不能 promotion 为 `Text + Inferred`；后续需要更多真实 fixture 或改进 text extraction 后再继续。
