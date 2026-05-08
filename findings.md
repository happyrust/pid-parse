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

## 2026-05-06 下一阶段开发计划结论
- 新计划文件：`docs/plans/2026-05-06-pid-parse-development-plan-cn.md`。
- 当前主判断：`.pid` 容器/metadata/object graph/crossref/layout/writer/publish XML 已经可作为工程骨架使用；Sheet 深层几何仍是最大未闭环区域。
- 当前几何状态：5 fixture / 3 sheet 横向扫描已有最小 object-coordinate promotion，最新实测为 `identity_supported=44`、`identity_over_threshold=28`、`promotable=5`、`object_geometry_hint_count=5`、`text_over_threshold=0`；Text/Symbol 仍无 promotion。
- 下一阶段顺序：
  1. fixture baseline hardening。
  2. Sheet record grammar reverse engineering。
  3. object-coordinate promotion gate hardening。
  4. Text/Symbol source-proven rendering。
  5. canonical graph integration。
  6. publish XML gate closure。
- 关键决策：Phase 9A 必须先扩 fixture 到 8-12 个，再扩大 Line/Text/Symbol promotion；否则会把当前 probe 噪声固化进 H7CAD UI 或 normalized graph。
- promotion 铁律：relationship endpoint 只证明语义连接，不证明 CAD 坐标；`SheetObjectGeometryHint` 必须与 promotion gate output 对齐，且每个 hint 都要保留 stream/offset/field_x/confidence/reason。
- Phase 9A 首个实现切片：`tests/parse_real_files.rs` 新增 `geometry_fixture_cases()` 显式 registry 与 `GEOMETRY_FIXTURE_TARGET_MIN_AVAILABLE=8`，现有 inventory 已复用 registry 并输出 fixture category。
- Phase 9A 第二个实现切片：`geometry_fixture_availability_summary()` 输出 registered / target_min_available / available / missing，锁定当前 registry 与 8+ fixture 目标之间的缺口。
- Phase 9A 第三个实现切片：`geometry_fixture_availability_report_line()` 已接入 inventory 输出；当前报告头为 `registered=5, target_min_available=8, available=5, missing=[]`。
- Phase 9C 首个实现切片：`populate_object_geometry_hints()` 的 note 已从单纯 `score=N` 升级为包含 `identity=graphic_nearby` 与 `stable_shape=...` 的 promotion gate 摘要；`promoted_object_geometry_hints_explain_promotion_gate` 锁定 offset、position 与 note provenance。
- Phase 9C 第二个实现切片：`normalized_geometry_projection_preserves_promoted_hint_source_notes` 锁定 `build_normalized_geometry()` 会把 promoted hint 的 `score/identity/stable_shape` note 复制到 `PidGraphicProvenance.note`；生产代码已具备该行为，本轮只补回归。
- Fixture 扩容复查：本地 `test-file` 下当前只找到 5 个 `.pid` fixture，均已在 `geometry_fixture_cases()` registry 中；Phase 9A 的 8-12 fixture 目标需要新增外部真实 PID 样本后才能继续。
- Phase 9A fixture 扩展方案已补充到 `docs/plans/2026-05-06-phase-9a-fixture-expansion-plan-cn.md`；下一步需要新增真实 `.pid` fixture，或确认先提交当前 5-fixture 基线。

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

## 多 fixture geometry evidence inventory
- 已新增 investigation-only 横向报告 `available_pid_fixtures_geometry_evidence_inventory_stays_probe_only`，覆盖当前可用的 5 个 PID fixture，包括非 ASCII 文件名 fixture。
- 当前报告结果：
  - `fixtures=5`
  - `sheets=3`
  - `windows=6337`
  - `record_shape_classes=328`
  - `identities=437`
  - `same_object=17`
  - `wrong_object=420`
  - `identity_supported=0`
  - `max_identity_score=45`
  - `identity_over_threshold=0`
  - `text_candidates=578`
  - `text_over_threshold=0`
- top aggregate record shapes 当前为 `(12,-18)`、`(14,38)`、`(68,5)`，分别累计 support 4/4/4；这些是 grammar 复查入口，仍未达到 source-proven promotion gate。
- per-fixture / per-sheet 明细已输出：
  - `DWG-0201GP06-01.pid /Sheet6`：`field_xs=57`、`windows=6025`、`record_shape_classes=272`、`same_object=11`、`wrong_object=414`。
  - `DWG-0202GP06-01.pid /Sheet6`：`field_xs=28`、`windows=156`、`record_shape_classes=28`、`same_object=3`、`wrong_object=3`。
  - `工艺管道及仪表流程-1.pid /Sheet6`：无 endpoint field_x，当前只贡献 text candidates。
  - A01 publish fixture 多个 JSite Sheet 暂无 endpoint field_x，当前只贡献 text candidates / skip 明细。
- 结论：多 fixture 横向扫描增加了样本覆盖，但仍没有 source-proven geometry promotion 证据；`object_geometry_hints` 继续保持空基线。

## H7CAD 工作树状态
- `D:/work/plant-code/cad/H7CAD-pid-real-geometry-display` 包含 H7CAD inferred point 渲染实现：`.pid` 打开后保留拓扑预览，并叠加 `PID_GEOMETRY_POINTS`。
- `D:/work/plant-code/cad/H7CAD` 主工作树当前仍是旧链路：`open_pid -> derive_layout -> pid_document_to_preview`，未接 `build_normalized_geometry` / `geometry_stats`。
- `normalized_geometry_real_fixture_renders_expected_points` 在 geometry 工作树通过，基线为 `normalized=132`、`inferred_points=64`、`probe_unknowns=68`、`rendered=64`、`point_layer=64`。

## Top candidate record dump helper
- 已新增 `top_field_x_candidate_record_dumps` 和 `top_text_candidate_record_dumps`，输出 rank、score、reasons、offset 以及 bounded hex byte windows，服务 Sheet record grammar 人工审查。
- helper 只做 investigation dump，不填充 `SheetObjectGeometryHint`，不改变 Line/Text/Symbol promotion gate。
- `sheet6_top_candidate_record_dump_stays_investigation_only` 使用真实 `/Sheet6` fixture 验证 dump 非空、byte window 有界，并继续断言 `object_geometry_hints=0`。

## Phase 10 关键发现：f64 Pair 坐标候选突破 Endpoint Line 零线困局
- Phase 9C 诊断链揭示当前 5 个 fixture 的 `inferred_lines=0` 根因：known object field_x 的 `nearest_coordinate` 搜索未覆盖 repeated record shape 中的 f64 pair。
- `DWG-0201GP06-01.pid /Sheet6` 的 `field_x=630..639` 诊断：
  - 每个 field_x 的 marker `5E 00 22 00 00 00 <field_x-le>` 前 22 字节处均可解析出有限 f64 pair。
  - 坐标值呈连续递增：`x ∈ [0.082, 0.244]`，`y` 稳定在 `0.224561`，非随机噪声。
  - `RepeatedF64PairBeforeField` 已作为 diagnostic reason 进入 score explainability。
- 下一步（Phase 10）：将 f64 pair 从诊断 reason 升级为保守坐标候选源，作为 `nearest_coordinate` 的 fallback，期望至少让一个 fixture 产生 `inferred_lines > 0`。
- 关键约束：f64 pair 坐标值域可能存在单位转换/坐标系偏移，在 multi-fixture 横向验证前保持 `Inferred` confidence，不升级为 `Decoded`。
- 方案文件：`docs/plans/2026-05-09-phase-10-f64-coordinate-source-endpoint-line-plan-cn.md`。
- 实现结果（Slice 1-3）：
  - `SheetFieldXF64PairShape` 扩展携带 `x, y`；`SheetFieldXWindowScore` 新增 `f64_pair_candidate`。
  - 新增替代 promotion gate：`ObjectFieldResolves + RepeatedF64PairBeforeField(support >= 3)` → f64 pair position。
  - `DWG-0201GP06-01.pid` promotable 从 5→16，inferred_points 从 69→80。
  - `DWG-0202GP06-01.pid` promotable 从 0→2。
  - 但 `inferred_lines` 仍为 0：endpoint pair 需双端 promoted，当前 `only_a=5, only_b=39`，无一对完全重合。
  - 结论：f64 pair gate 有效扩展了单对象定位覆盖，但 endpoint line 需要进一步扩展对端覆盖。
- Phase 10B 实现（f64 triple pattern）：
  - 发现 endpoint_a field_xs 使用 `FA 00 XX 00 00 00` marker（vs 原有 `5E 00 22 00 00 00`），前有 3 个 f64 值。
  - 新增 `repeated_f64_triple_candidate_before_field_x` extraction helper。
  - 最终结果：`DWG-0201GP06-01.pid` 产生 **34 条 inferred lines**，`DWG-0202GP06-01.pid` 产生 **3 条 inferred lines**。
  - 3 个 fixture 现在是 line-producing fixture。
  - promotable 从 5→总计远超 20（含 triple pattern 覆盖）。
- Phase 11 Slice 2 进一步扩展：
  - 发现第三种 marker `CE 00 XX 00 00 00`（2 f64 + 8 零字节 + marker），覆盖低编号 field_x。
  - `fully_promoted` 34→49/59（83.1%），`inferred_lines` 34→49，`neither` 1→0。
  - 三种 marker 现在覆盖：`5E 00 22`（pair）、`FA 00`（triple-xy23）、`CE 00`（triple-xy12）。
  - 剩余 10 对 gap 分析：4 对 endpoint_b=0（null 终止点，永远无线）；6 对中 4 个 missing field_x（659, 671, 35, 68）不在 object_field_xs 中，无法通过 `ObjectFieldResolves` 条件。
  - 结论：当前 52 个 promoted 对象覆盖了所有 object graph 中可图形化的 endpoint 对象；剩余 gap 来自非对象图成员，属于 scope 边界。
- Phase 11 坐标值域分析：
  - f64 坐标域：`x ∈ [0.082, 0.475], y ∈ [0.000, 0.275]`，确认为 0-1 范围归一化页面坐标。
  - i32 坐标域：`x ∈ [0, 983056], y ∈ [-327679, 983056]`，不同坐标系（可能是 twips/EMU）。
  - 模板信息：`Template = XIONGANA2.pid`（A2 纸 594×420mm）。
  - 两种坐标系之间的映射关系尚未建立；f64 归一化坐标 × 页面尺寸 = 物理坐标（推测）。

## Sheet record shape classifier
- 已新增 `classify_field_x_record_shapes` 与 `SheetFieldXRecordShapeClass`，按 `(field_delta_from_chunk, coordinate_delta_from_chunk)` 聚合 non-endpoint `field_x` window features，统计 distinct `field_x` support，并保留示例 field / coordinate offset。
- `/Sheet6` 当前 top shape classes 为 `(14, 38)` 和 `(46, 70)`，support 均为 2；这说明存在可复查的重复 record shape，但还不是 source-proven geometry。
- `sheet6_field_x_window_features_report_chunk_shapes` 已接入 classifier，仍保持 `max_score=45`、`promotable=0`，不填充 `SheetObjectGeometryHint`。
- 多 fixture inventory 已接入 classifier 汇总：当前 `record_shape_classes=328`，top aggregate shapes 最高累计 support 为 4，但 identity/text promotion threshold 仍为 0。
