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

## Phase 14-17 关键结论（2026-05-14 ~ 2026-05-17）
- Phase 14 落地 8 个 PSM 类型 typed decoder（GLine2d=3、GArc2d=48、igLine2d=284、igLineString2d=119、igPoint2d=146、igTextBox=142、igSymbol2d=27，共 769 decoded entities）；reusable seven-layer decoder template 在该 phase 验证 6×。
- Phase 14 §6.3 把 PSM `0x0010`（638 probe scan hits）定性为 "embedded sub-records / attribute fragments inside other record types"，留给 Phase 18。
- Phase 15 落地 PSM `0x00FA` GraphicGroup audit-only decoder（352 records，header + raw_variable_tail），不引入 PidGraphicKind variant；audit-only 模板被 Phase 18 复用。
- Phase 16 跨 5 IDA instance（radsrvitem.dll → J2DSrv.dll → JUTIL.dll → style.dll）反向 PSM `0x0030`，最终钉到 RAD `JStyleOverride` 类（CLSID `{47FCC338-2D0F-11D0-A1FF-080036A1CF02}`），V3 disk schema 13 个 IOContext::DoIO（64 字节 payload），跨 fixture 98 records；找回 Phase 14 GArc2d `axis_a.y ≈ 0` 约束误拒的 50 条真 record；新增 `decode_jstyle_overrides` + `PidGraphicKind::Annotation`。
- Phase 16 §11：probe v5 把磁盘 `+0..15` 解读为 2 个 f64 anchor（跨 fixture 100% 落归一化范围）；IDA V3 schema 解读为 4 个独立 u32。`JStyleOverride::Clone` (sub_10010640) 的 `qmemcpy(v5+22, this+22, 0x58)` 证明 RAD 层是 untyped byte IO；SmartPlant 把 RAD 的 byte slots 当 `union { u32; f64 }` 用，两种解读**同时正确**。
- Phase 17 移除 legacy `decode_primitive_arcs` 系列（parser API + DTO + model field + schema entry + geometry emission），杜绝下游消费者继续误读 0x0030 为 IGDS GArc2d。Default schema 新增 `jstyle_override` 入口。

## Phase 18 关键结论：PSM 0x0010 sub-record audit-only landing（2026-05-17, commit 81daa20）
- `SheetSubRecord0x0010Decoded` 沿用 Phase 15 GraphicGroup 6-byte header 模板（`type_word + bytes_to_follow`，NOT Phase 14 IGDS 18-byte header），无 `oid` 字段。
- Advancing-scan decoder（accept-then-skip）跨 4 fixture 输出 **582 records**：DWG-0201=161 / DWG-0202=104 / 工艺管道-1=306 / A01=11。probe non-advancing scan 报 638（含 overlap）。
- Validation 极保守：`type_code == 0x0010` + `bytes_to_follow ∈ [8, 100_000]` + 边界检查；不在 payload 上做额外 validation（不知 sub-kind discriminator 在哪个字节）。
- `raw_payload: Vec<u8>` 复制 payload bytes（不借用），JSON 序列化为 number array，与 Phase 15 GraphicGroup `raw_variable_tail` 保持一致。
- 关键设计原则：未 IDA-confirmed 前不命名 sub-kind 字段；不引入 PidGraphicKind variant；不实现 reference resolver（这些都是 Phase 19/20+ 工作）。
- 12 个 parser unit test + 1 cross-fixture ratchet test + adversarial panic-safety matrix；5 道 pre-commit gate 全绿；Phase 14-17 baseline 全部保持。

## Phase 19 关键结论：PSM 0x0010 leading_word audit field（2026-05-17, commit 6beb6f1）
- **RAD sibling sweep 假设被证伪**：`examples/probe_rad_siblings_0x0029_0x0035.rs` 扫描 `/Sheet6` 上 PSM type code `0x0029..=0x0035`，跨 4 fixture 只有 `0x0030`（JStyleOverride）有 hits（115 total），其余 12 个 type code 全 0。"RAD 47FCC330..47FCC33E CLSID 段 1:1 映射 PSM 0x29..0x35" 假设不成立。Evidence：`docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md`。
- **leading_word @ payload[0..2] LE u16 是部分 sub-kind discriminator**：`examples/probe_psm_0x0010_sub_kind.rs` 跨 4 fixture 578 records 直方图：
  - `0x0002` = 164 records (28.2%)，跨 ~40 个 size bucket 普遍出现
  - `0x0003` = 21 records (3.6%)
  - `0x0001` = 18 records (3.1%)
  - `0x4C1C` / `0x4E1C` = 各 8 records（size=16 bucket 双峰）
  - `0x8EA5` = 7 records（size=86 bucket 85% 单峰）
- **~30 个 size bucket 是 single-word-dominant**：size=12/15/19/22/25/26/27/29/36/37/41/42/45/47/69/76/86/92/94/97/102/115/119/120/123/147 在 `+0` 处 100% 单一 word；这类 record ~280 条。
- **size 31 / 70 / 13 / 16 / 43 在 `+0` 异质**：size=31 是最大 bucket（182 records）但 top word 只占 1%；size=70 (53 records) top word 5%；size=13 (21 records) top word 14%。这些 bucket 的 leading bytes 几乎肯定不是 sub-kind discriminator，可能是坐标或 OID。
- **结论**：单一固定偏移 discriminator 不能干净划分整个 0x0010 family。`leading_word` 字段名描述字节位置（`payload[0..2]` LE u16），**不**描述语义；杜绝重蹈 Phase 14 GArc2d 错误命名（axis_a / axis_ratio / sweep_direction / sweep_angle 全错）。
- **落地**：`leading_word: Option<u16>` 在 parser DTO 与 model DTO mirror；`Option<>` 类型保留给 < 2 byte payload，虽然 decoder min payload = 8 让 `None` 不可达，但保留契约诚实性。cross-fixture ratchet 锁定 0x0002=164 / 0x0003=21 / 0x0001=18 / None=0 / total=582；Phase 18 ratchet 582 不退化。

## Phase 20 IDA-RAD-class roadmap（2026-05-17, package drafted, awaiting /goal）
- 12 个 IDA instance 全 reachable：`radsrvitem.dll` port 13346 / `J2DSrv.dll` 13347 / `style.dll` 13348（Phase 16 navigated）/ `sppid.dll` 13341 / `smartplantpid.exe` 13342 / `sppidautomation.dll` 13340 / `sppiddwgprocess.dll` 13343 / `sppidautomation.exe` 13344 / `llama.dll` 13345 / `ipidobjectmanagerinf.dll` 13339 / `sppidautomationwrap.dll` 13338 / `core.dll` 13337（AVEVA E3D，可能 unrelated）。
- `radsrvitem.dll` 规模：32-bit，base 0x56440000，5374 functions（4867 unnamed、~90%），1739 strings；exports `GetServerItemTransceiver` (0x56448040) / `GetServerItemVersion` (0x564480d0)。PSM dispatch table 反向必须从 unnamed function 入手，预期需要 `search_text` literal + xref + analyze_function 多次跳转。
- Phase 20 工作量预估：Phase 16 单 type code 反向用了多 session，Phase 20 polymorphic family 预期 **2-5 session**，必须按 Slice A-G 逐个 checkpoint；详细路线图见 `docs/plans/2026-05-17-phase20-ida-rad-class-roadmap-cn.md`。
- Phase 20 scope **严格 reverse engineering + 文档**，不改 src/ 代码、不改 test；Phase 21 才会基于 Phase 20 IDA 证据落地 typed sub-kind DTO + reference resolver。
- 备选方案：20-B `JStyleOverride/GraphicGroup → 0x0010 reference resolver`（不需 IDA、0.5-1 session）；20-C size=31 bucket 专项反向；20-D 多 Sheet* 流未知 type code inventory。详细决策矩阵见 roadmap 文档 §4。

## Phase 21 关键结论：D06 fixture baseline + relationship graph fix + Sheet audit（2026-05-18）
- D06（`test-file/D06.pid`）是一个紧凑 SmartPlant PID 样本，当前解析输出 97 total / 25 decoded geometry entities，无 decoded lines（polyline/point/annotation 为主）。
- **relationship gap 根因**：D06 的 10 条关系身份存放在 `P&IDAttributes` 的 `ModelItemType=Relationship` + `ModelID=Relationship.<GUID>` 中，而非 DWG fixture 使用的 `class_id == 0xF6` DA trailer。修复方案：当 trailer path 产生 0 条 relationship 时，扫描 `P&IDAttributes` 提取已被 probe 确认的 GUID，保留为 unresolved `PidRelationship`（endpoint = None）。
- **D06 relationship 全部 unresolved**：无 Sheet-level `field_x` link，endpoint resolution 需等待后续 phase。
- **Sheet6 audit inventory**：21 GraphicGroup + 20 `0x0010` audit-only records；`leading_word == 0x0002` 在 D06 也出现；GraphicGroup `raw_reference_payload` 不 promote 为 child OIDs。8 个 probe-only unknown 是未定位的 text runs。
- **Phase 14-20 边界完好**：不命名 `0x0010` sub-kind，不新增 typed DTO，不解释 GraphicGroup tail。

## Phase 23 方案结论：Coordinate/Page Context 优先（2026-05-18）
- Phase 20 partial AC 后，typed `0x0010` DTO 仍缺 human class name、Read/DoIO sequence 与 sub-kind discriminator；继续推进会违反 Phase 18/19 audit-only 原则。
- Phase 21/22 已把 D06 作为 compact fixture 纳入 baseline，但 D06 text probes 仍不能 promotion 为 inferred `Text`。
- 当前代码已有 `NormalizedPidGeometry.page_dimensions_mm`、`PidCoordinateContext`、`PidPageTransform` 与 `coordinate_page_metadata_investigation_report`，但 transform 仍应保持 unavailable。
- 下一阶段推荐 Phase 23A：先收敛 coordinate/page metadata 报告和 transform promotion gate，明确 page dimensions 不等于 page transform。
- 方案文件：`docs/plans/2026-05-18-phase23-coordinate-page-context-plan-cn.md`。
- 后续只有在找到完整 source record / scalar source / decoded semantics 时，才允许 `PidPageTransform::Available`；否则继续保留 source coordinates 与 explicit unavailable diagnostics。
- Slice A 已落地 guardrail：`template_page_dimensions_do_not_make_page_transform_available` 锁定 DWG-0201 A2 page dimensions `Some((594.0, 420.0))` 仍不能让 entity page transform available；`src/geometry.rs` doc comment 也明确 page size evidence 不等于 source-to-page transform。
- Slice B 已落地 compact top evidence：`SheetCoordinatePageMetadataInvestigationReport.top_evidence` 输出最多 8 个强候选摘要；cross-fixture 当前 `coordinate_metadata_candidates=97`、`coordinate_top_evidence=36`、`normalized_f64_pair_count=1397`、`page_dimension_scalar_matches=0`，仍保持 no-promotion。
- Slice C 已落地 transform promotion gate 合同：`PidPageTransform::Available` doc comment 明确需要 source coordinate space、units、transform direction 与 bounded byte provenance；新增默认 context unavailable 单测，并在 normalized geometry schema test 中锁定 `available/origin/scale/page_bounds/matrix` 字段。
- Slice D 已同步下游文档：`docs/prd-pid-parse-current-state.md` 与 `docs/architecture-guide.md` 现在明确 page_dimensions 是 page-size evidence，H7CAD / JSON consumer 在 transform unavailable 时不应猜测 source/page/viewport 映射；`CHANGELOG.md` 已记录 Phase 23 A-D。
- Slice E 全量门禁通过：build / test --workspace --all-targets / clippy -D warnings / fmt --check / rustdoc missing-docs 均绿；Phase 23 可按当前证据声明 complete，但不声明 page transform decoded。

## Phase 24 方案结论：CoordinatePageMetadata decoder 候选筛选（2026-05-18）
- Phase 24 不直接实现 `PidPageTransform::Available`；第一步是从 Phase 23 `top_evidence` 生成 candidate marker group evidence table。
- 当前关键事实：`coordinate_metadata_candidates=97`、`coordinate_top_evidence=36`、`normalized_f64_pair_count=1397`，但 `page_dimension_scalar_matches=0`。
- 方案文件：`docs/plans/2026-05-18-phase24-coordinate-page-metadata-decoder-plan-cn.md`。
- 核心 gate：如果候选无法解释完整 width/height/origin/scale/bounds 字段组，必须以 negative analysis 收口；不能把 coordinate-like f64 evidence 误升为 page transform。

## Phase 22 micro 关键结论：D06 进入 6 个 Phase 14 cross-fixture decoder tests（2026-05-18, commit bf4f972）
- D06 已被 Phase 21 (`678af70`) + `5255f25` 加入仓库并由 baseline test
  `d06_pid_parses_with_expected_structure_and_geometry_summary` 与
  `d06_text_placement_regression_keeps_text_probes_unpromoted` 锁定；
  但 D06 在 Phase 14 cross-fixture decoder tests 中未列为 fixture。
- Phase 22 micro 把 D06 列入 6 个 Phase 14 cross-fixture decoder fixture
  数组，并按 D06 baseline 锁定的逐 decoder 计数 ratchet 阈值：
  K +6 (igLineString2d) / L +10 (igPoint2d) / M +4 (igTextBox) /
  N +2 (igSymbol2d)；E (GLine2d) / J (igLine2d) 阈值不变（D06
  贡献 0，作为 parse-package / panic-safety guard）。
- 与其它 cross-fixture decoder 测试一致：每个 fixture 旁加 inline 注释
  解释 D06 贡献，便于未来 ratchet drift 自我说明。
- 此 commit 与远端 Phase 23 实现 (`6c554b9`)、Phase 24 plan
  (`a1f0843`)、Phase 20-22 文档同步 (`0b56818`/`53f04fa`) 互补，
  pull/push 均 fast-forward 无冲突。

## Phase 24 Task 24-01 关键结论：candidate evidence + negative evidence 收口（2026-05-18, commit 8f3739c）
- **Cross-fixture stable marker = 0**：probe
  `examples/probe_phase24_top_evidence.rs` 跨 5 fixture × 7 sheet dump
  出 **29 top_evidence 行 / 25 distinct marker**；几乎全部
  `support = 1` 且单 fixture / 单 sheet。唯一跨 2 fixture 的 marker
  `0x0000 (0)` 在 DWG-0202 是 `NormalizedF64CoordinateLike`、在 D06
  是 `InsufficientEvidence`，kind 不一致 → 不算 stable cross-fixture
  evidence。
- **page_dimension_scalar_matches = 0 cross-fixture**：29 行
  top_evidence 无一命中 `page_dimension_scalar_matches > 0`；与
  Phase 23 cross-fixture aggregate
  (`sheet_geometry_investigation_aggregates_cross_fixture_evidence_without_promotion`)
  输出 `page_dimension_scalar_matches=0` 完全一致 → page dimension
  scalar source 在当前 5 fixture 中不存在。
- **Phase 24 plan known_unknown marker `0xC03F (49215)` 单 fixture**：
  只在 DWG-0201 `/Sheet6` 出现 2 次，A01 / DWG-0202 / 工艺管道-1 / D06
  完全无。Phase 24 plan §known_unknowns 第 1 条
  "marker 49215 是否是真实 page metadata record 仍未证明" → 本 Task
  确认为 **否定**。
- **NormalizedF64CoordinateLike 占主导 (25/29 = 86%)**：coordinate
  evidence 丰富 (`normalized_f64_pair_count=1397`)，但所有 f64 pair
  缺 page-dimension 锚点；几乎肯定都是 geometry coordinate 而非
  transform metadata。
- **Stop-And-Challenge 4 条满足 3 条** → 按 Phase 24 plan Task 24-02
  `<done>` 选择 **路径 A negative evidence 收口**：跳过 Task 24-03
  typed candidate DTO 实现，保留 Phase 23 `probe_only_no_coordinate_
  page_metadata_promotion` guardrail 不变。
- **下次重启条件**：若新增 PID fixture 在 **同一 marker** 上出现
  **kind 一致** 的 top_evidence，且至少 1 行
  `page_dimension_scalar_matches > 0`，则可重启 Task 24-03。
- **closure_claim_limit 遵守**：本阶段只声明 negative evidence，不声明
  page transform decoded、不让 `PidPageTransform::Available` 出现、
  不修改 `0x0010` audit-only surface。

## Phase 26 方案结论：PID 文件全格式分析说明（2026-06-03）
- 本阶段目标是产出 `.pid` 文件全格式分析说明，而不是新增 parser。
- 说明文档必须采用“当前实现说明 + 证据等级”口径，不写成最终格式规范：
  - `Decoded`：字段布局和语义已能进入 typed model。
  - `Probed`：位置或形状有证据，但语义未完全命名。
  - `IdentifiedOnly`：storage/stream 已识别，但结构化解析不足。
  - `Leftover`：byte-audit 未声明消费的字节。
- 新计划文件：
  `docs/plans/2026-06-03-phase26-pid-file-format-analysis-plan-cn.md`。
- 主交付物：
  `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`。
- Phase 26-A/B 已完成：主文档已按当前实现口径落地，剩余 Phase 26-C/D 负责 fixture 快照与入口交叉链接。
- Phase 26-C 已检查但未生成快照：当前工作树未发现 `.pid` fixture，
  `test-file/**/*.pid` 与 repo-wide `**/*.pid` 均为 0，`git status --ignored -- test-file`
  也无 ignored fixture 输出；主文档 §12 已记录阻塞原因与后续补跑命令。
- Phase 26-D 已完成：README 已新增新版格式说明入口；`docs/format-notes.md`
  已标记为早期轻量 notes，并指向新版 evidence-graded 格式地图。
- 主文档应覆盖：
  - OLE/CFBF 外层容器与 raw byte preservation。
  - known top-level streams 与 storage prefixes。
  - magic / signature 对照。
  - 每类 stream 的字节布局、parser、模型字段、coverage 状态与限制。
  - `Sheet*` decoded / audit-only / probe-only record families 分层。
  - Dynamic Attributes → relationship → Sheet endpoint 证据链。
  - `PidPageTransform` guardrail 与 Phase 24 negative evidence。
  - coverage / byte-audit 的实际验证方法。
- Grill-me 关键决策：
  - publish XML / MDF 是独立管线，只应作为附录说明边界。
  - `Sheet*` 不能整体写成完整 decoded；只能按 record family 分层声明。
  - fixture coverage / byte-audit 快照有价值，但作为附录，不阻塞主文档草案。
  - 文档粒度到 stream / record-family 级，不逐 byte 穷举未命名 payload。
- Stop-And-Challenge 边界：
  - 不把 probe/audit-only 结果包装为 decoded contract。
  - 不声明 `PidPageTransform::Available` 当前已支持。
  - 不命名 `0x0010` sub-kind、GraphicGroup child list 或其它未 IDA-confirmed 字段。
  - 如果 fixture 快照与 registry/coverage 冲突，先修正事实源再继续写说明。
- `docs/format-notes.md` 当前属于早期简版：只覆盖 container / tagged metadata / JSite / cluster / dynamic attribute string scan，未表达 Phase 14+ Sheet typed decoder、audit-only record family、Phase 23/24 transform guardrail；Phase 26-D 应将其改为新版说明入口或明确标记为 legacy notes。

## Phase 27 关键结论：IDA type-code mapper 与数据类型矩阵（2026-06-03）
- Phase 27 目标从“格式说明”升级为“IDA 证据驱动的全 PID 数据类型提取”：先建立完整数据类型矩阵，再逐类补齐 reader、字段布局、parser DTO 与测试门禁。
- 当前可用 IDA 起点是 `radsrvitem.dll` (`127.0.0.1:13338`)；survey 确认 32-bit、5374 functions、1739 strings、exports `GetServerItemTransceiver` / `GetServerItemVersion`。
- `sub_56448F70(_WORD *a1)` 是当前最直接的 `u16 type_code -> SmartPlant/IGDS type name` mapper：
  - switch case 覆盖 27 个 type code。
  - if/else 额外覆盖 `0x0115 igDimension`、`0x0117 igBalloon`、`0x0118 igLeader`。
  - 默认返回空/默认字符串，不代表未命名 type 已解析。
- 已确认当前 parser 名称证据：
  - `0x0018 = igLine2d`
  - `0x004D = igTextBox`
  - `0x005E = igPoint2d`
  - `0x0084 = igLineString2d`
  - `0x00CE = igSymbol2d`
- `sub_564468B0` 是 `igTextBox` reader / extraction 样板候选：入口检查 `*a2 == 77`，按 `a2[12] == 1/2/3` 选择不同 payload reader，读取 UTF-16LE 文本并写入 `"TEXT"` 属性。
- 当前 parser 已覆盖但不在 `sub_56448F70` IGDS mapper 表内的类型必须分开处理：
  - `0x0010`：PSM sub-record / attribute fragment family，仍禁止把 `leading_word` 命名为 `sub_kind`。
  - `0x0030`：Phase 16 已确认是 `JStyleOverride`，不能回退解释为 arc。
  - `0x00FA`：`GraphicGroup` audit-only，不能命名 child OID list。
  - `0x3FE6`：`GLine2d` SmartPlant 扩展 wrapper，需另找 dispatch/reader 证据。
- 新矩阵文件：`docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`。
- 新计划文件：`docs/plans/2026-06-03-phase27-ida-driven-pid-data-type-extraction-plan-cn.md`。
- Phase 27 的下一步优先级：
  1. 校准已 decoded 类型：`igTextBox`、`igLine2d`、`igLineString2d`、`igPoint2d`、`igSymbol2d`。
  2. 追高价值几何图元：`igCircle2d`、`igArc2d`、`igEllipse2d`、`igEllipticalArc2d`、`igBSplineCurve2d`、`igRectangle2d`。
  3. 再处理关系约束与标注类型。
- `igTextBox` 样板对照的首个结论：
  - `sub_56445F40` 是当前 per-record dispatch 入口之一：`0x004D` 进入 `sub_564468B0`，`0x00FA` 进入 `sub_56446020`，`0x003D` 进入 `sub_564464D0`，其它类型走默认 `sub_564462F0`。
  - `sub_564468B0` 确认 `igTextBox` 的 type identity、UTF-16LE 文本语义和 `"TEXT"` 属性写入。
  - `sub_564468B0` 处理的是 runtime record pointer，不足以直接证明 Rust raw Sheet decoder 的 payload offset 30/32；当前 offset 仍主要是 fixture/probe 证据。
  - IDA 暴露了 `a2[12] == 1/2/3` 三种文本 layout mode；Rust 当前固定读取一个 raw layout，后续必须确认 runtime `a2` 与 raw `Sheet*` bytes 的 offset 映射。
  - IDA 会写 `"RELEATIONS"` 属性，可能是当前 `SheetIgTextBoxDecoded` 未显式暴露的语义候选；但需确认是否已由其它 relationship parser 覆盖，不能直接加字段。
- 默认 IGDS 路径 `sub_564462F0` 的 negative evidence：
  - 该函数只调用 `sub_56448F70` 获取 type name，创建/写入 RAD object properties，并写 `"RELEATIONS"`。
  - 未读取普通几何字段：line endpoint、point coordinate、polyline point list、symbol transform、circle/arc/ellipse 参数都未在该路径出现。
  - `igPoint2d`、`igLineString2d`、`igSymbol2d`、`igCircle2d` 在当前 `radsrvitem.dll` 只命中 mapper 和字符串本体。
  - 结论：`radsrvitem.dll` 当前足以提供 type identity / naming evidence，不足以恢复普通几何字段 layout；后续需要追 `vtable offset 164` 的 record manager，或切到 `J2DSrv.dll` / `style.dll` / `sppid.dll` / `XCeedRAD.dll` 等相关 IDA 数据库。
- `sub_56445F40` 的 `v10` record pointer 来源追踪：
  - `v10` 来自 `this+0x3c` runtime record manager 的 vtable `+0xA4` lookup：`(manager, record_id, 0x40, &record_ptr)`。
  - `sub_5644B640` 是当前发现的 vtable 入口之一；`a3 == 0` 时遍历 type=1 section payload 中的 6-byte stride record-id list 并筛 `*(u16*)record == 0x0089`，`a3 == 1` 时直接把外部 `a2` 当 runtime record id 交给 `sub_56445F40`。
  - runtime section 编码已恢复：低 7 bit 是 section type，高 bit 是 list-end marker；长度普通走 `section[1]`，`0xFF` 时走 `section[2..4]` 扩展长度；payload 从 `section+2` 或 `section+4` 开始。
  - `this+0x3c` 来自 `sub_56445C90` 对 `PersistManager` 的 QueryInterface，IID 为 `{1FC155A0-6BE3-101B-97A9-08003601CDC9}`，构造函数 `sub_56467810` 显示对应 `ImpIJPersistManager::vftable`。
  - `ImpIJPersistManager::vtable+0xA4 = sub_56468DB0`，核心 `sub_56468DF0` 将 `record_id` 拆为 `record_id >> 13` segment/bucket 与 `record_id & 0x1FFF` descriptor index。
  - `SerialCluster::vtable+0x70 = sub_56493F50` 是当前命中的 materializer：最终 `out_record_ptr = serial_cluster_base + record_descriptor[0]`；未加载页时通过 `sub_56495BD0` 按 4KB page 经 stream Seek/Read 映射 bytes。
  - `sub_56493BC0` 提供反向公式 `offset = ptr - serial_cluster_base`；`sub_56494C40` / `sub_56495BD0` 做 stream IO 时同样用 `page_or_record_ptr - serial_cluster_base` 作为 Seek offset。因此 `record_descriptor[0]` 可视为该 `SerialCluster` stream 内 offset。
  - `ImpIPersistStorage::vtable+0x18 = sub_56469BF0` 对应 COM `IPersistStorage::Load(IStorage*)`，它从传入的 CFB `IStorage` 打开 `PSMclustertable`、`PSMroots`、`PSMspacemap`、`PSMcluster0`。
  - `sub_56491090` 是 `IStorage::OpenStream` wrapper，`sub_56491150` 是 `IStorage::OpenStorage` wrapper；保存侧 `sub_56469950` / `sub_5646AE30` 通过 `sub_56490B30` / `sub_56490BF0` 创建同名 PSM streams/storages。
  - 现有 `core.dll` IDA instance 只提供 `ASHEET` / `DSHEET` 数据库属性初始化与 `CMPTSZ` sheet token 坐标调试/命令输出证据，未发现 `igTextBox` / IGDS type reader 或 PID `Sheet*` raw record reader。
  - 结论：`sub_564468B0` 处理 loaded SerialCluster 中的 runtime record layout；IDA 已把 runtime pointer 链到 CFB 根 storage 下的 PSM 持久化 stream offset，但没有把该 offset 直接绑定为 Rust raw `Sheet*` stream byte offset，也未排除 PSM→Sheet 投影层或 envelope/header offset。Rust `igTextBox` 的 `payload+30 text_length` / `payload+32 text` / trailing f64 仍属于 fixture/probe-backed offset，不能因本轮 IDA 证据升级为 IDA-confirmed。

## Phase 28 关键结论：Spec Kit 风格 PID 文件全格式规格包（2026-06-08）
- 新增规格包目录：
  `docs/specs/2026-06-08-pid-file-format-spec-kit/`。
- 规格包采用 Spec Kit 风格拆分为：
  - `spec.md`：目标、用户故事、证据等级、功能需求、guardrails、验收标准。
  - `plan.md`：Phase 28-A..E 执行切片、IDA 续查入口、风险矩阵。
  - `research.md`：parser 事实、Phase 27 IDA 事实、fixture 限制、`0x0010` / JStyle 阻塞。
  - `data-model.md`：container / metadata / registry / PSM / Sheet type-code / derived geometry / writer-publish 边界的 evidence-graded inventory。
  - `tasks.md`：后续 IDA availability、evidence refresh、fixture snapshot 与 backlog 分类任务。
  - `quickstart.md`：parser inspection、coverage / byte-audit、test gates 与 IDA 复查流程。
- 本规格包不替代 Phase 26 / Phase 27 原始证据文档；它作为“规格入口 + evidence matrix + 任务索引”存在，详细证据继续链接到：
  - `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`
  - `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`
  - `docs/plans/2026-06-03-phase27-ida-driven-pid-data-type-extraction-plan-cn.md`
- 当前 IDA 结合方式是“使用已落盘 Phase 27 / MCP progress 中的 IDA 证据组织规格包”：
  - `radsrvitem.dll` 已证明 type-code mapper、`igTextBox` runtime reader 样板、默认 IGDS path negative evidence、SerialCluster runtime pointer 链。
  - 最新 MCP 进度显示 `radsrvitem.dll` 内 JStyleBase / IJPersist 路径基本收敛；继续 `0x0010` 字段语义需要打开 `style.dll`、`J2DSrv.dll`、`sppid.dll`、`XCeedRAD.dll` 或其它 JStyle/RAD host IDB。
  - 当前 `user-ida-pro-mcp` 的 MCP 文件系统只暴露 `SERVER_METADATA.json`，没有可读取的 tool descriptor；按 MCP 规则，未直接发起新的 live IDA tool call。
- 规格包的核心 guardrail：
  - `0x0010.leading_word` 仍是 byte-position audit 字段，不能命名为 `sub_kind`。
  - `0x0030` 仍是 `JStyleOverride`，不能回退成 arc。
  - `0x00FA GraphicGroup` 仍 audit-only，不能命名 child OID list。
  - `PidPageTransform::Available` 仍禁止出现，直到 source coordinate space / units / transform direction / bounded provenance 同时成立。
  - Text probes 仍 no-promotion。
- 按推荐方案继续后，当前工作树实际发现 6 个 `.pid` fixture：
  - `test-file/工艺管道及仪表流程-1.pid`
  - `test-file/D06.pid`
  - `test-file/DWG-0201GP06-01.pid`
  - `test-file/DWG-0202GP06-01.pid`
  - `test-file/export-test/publish-data/A01/A01.pid`
  - `test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid`
- 已为全部 6 个本地 `.pid` fixture 生成 Spec Kit coverage / byte-audit 快照，
  文件命名为 `<fixture-id>-coverage.json` 与 `<fixture-id>-byte-audit.json`。
- 6-fixture snapshot matrix 已写入 `data-model.md`：
  - coverage entries 范围：22–37。
  - `FullyDecoded` entries：除非 ASCII fixture 为 6，其余为 7。
  - `PartiallyDecoded` entries：全部为 6。
  - `IdentifiedOnly` entries：9–24。
  - byte-audit overall coverage ratio 范围：0.042824525–0.10965584。
  - `dwg0201` 当前 ratio 最高（0.10965584），非 ASCII fixture 当前最低（0.042824525）。
  - 这些数字是当前 6 个本地 fixture 的 snapshot matrix，不是私有/未提供客户文件的全局覆盖承诺。
- `data-model.md` 已新增 completion classification：
  - `Complete`：CFBF/container、metadata、registry、`PSMroots`、A01 publish 当前交付合同。
  - `NeedsIDA` / `NeedsParser`：PSM table deep semantics、PSMspacemap、P0/P1 Sheet geometry、Dynamic Attributes body、JSite。
  - `Blocked`：`0x0010`、`GraphicGroup` child/reference payload、page transform、text placement、writer semantic geometry edits。
- 已新增 `snapshot-priority-backlog.md`，从 6-fixture byte-audit 快照聚合出后续优先级：
  - highest-leftover family #1 是 `JSite*`：leftover 326,403 bytes，但是否优先取决于下游是否需要 symbol-instance 深字段。
  - `PSMcluster0`：leftover 193,173 bytes，ratio 0.00417562，是 parser backlog 候选。
  - `Sheet*`：leftover 121,176 bytes，ratio 0.06886536，仍是最高价值 geometry / IDA target。
  - `Unclustered Dynamic Attributes`：leftover 111,120 bytes，ratio 0.22326840，适合继续收敛 object/relationship deep body。
  - `StyleCluster`：leftover 83,468 bytes，ratio 0.00114882。
  - `PSMspacemap`：leftover 62,802 bytes，ratio 0，需 IDA 或 controlled fixture evidence 后再写 parser。
  - common unregistered paths 包括 `/JSitesList` 与 `/PSMspacemap/0x00000000` / `0x00002000` / `0x00004000` / `0x00006000`。
  - 推荐 backlog 顺序：`Sheet*` 保持 top parser/IDA target；`JSite*` 先等下游需求；`PSMspacemap` 只在 IDA/controlled evidence 出现后推进；PSM table semantics 服务 canonical graph。
- 已新增 `phase29-candidate-slices.md`，将 priority backlog 转为候选 implementation slices：
  - 29-A：Sheet stream delta and unknown record prioritization。推荐为 IDA 仍阻塞时的第一非 IDA slice；只做未知 record family 排序，不改变 parser 语义。
  - 29-B：PSMcluster0 body triage。目标是解释 `/PSMcluster0` 高 leftover，判断 parser-only 还是 IDA-backed。
  - 29-C：Dynamic Attributes deep body backlog。只实现能改善 object / relationship 语义的字段，不因 byte position 命名。
  - 29-D：PSMspacemap `tseg` evidence gate。当前 blocked，需 IDA / controlled diff / multi-fixture semantic link。
  - 29-E：JSite symbol-instance demand gate。剩余量最大，但需 H7CAD / publish / consumer 指名字段后再做。
  - 29-F：IDA module enablement。打开 `style.dll` / `J2DSrv.dll` / `sppid.dll` / `XCeedRAD.dll` / `smartplantpid.exe` 并恢复 tool descriptor 后才能继续 live IDA。
  - 推荐下一步：若 IDA 仍阻塞，执行 29-A；若 IDA 可用，先执行 29-F。

## Phase 29-A 关键结论：Sheet leftover priority report（2026-06-08）
- 新增分析文档：
  `docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md`。
- 该报告只排序 Sheet leftover，不改变 parser 语义，不把 `Probed` /
  `AuditOnly` promotion 为 `Decoded`。
- Aggregate Sheet path 结果：
  - `/Sheet6`：6 fixtures，total 129,506，consumed 8,818，leftover 120,688，ratio 0.06808951，registered hits 6。
  - `/JSite204/Sheet6`：1 fixture，total 6,870，consumed 0，leftover 6,870，ratio 0，unregistered。
  - `/Sheet6615`：2 fixtures，total 632，consumed 144，leftover 488，ratio 0.22784810。
  - `/JSite204/Sheet12` / `Sheet22` / `Sheet32` / `Sheet41` / `Sheet51`：publish A01 嵌套小流，unregistered，需先决定 ownership / byte-audit registration 语义。
- Top leftover items 前 4 名均为 registered `/Sheet6`：
  1. `nonascii-process-1 /Sheet6` leftover 44,053。
  2. `dwg0201 /Sheet6` leftover 27,399。
  3. `publish-dwg0202 /Sheet6` leftover 22,035。
  4. `dwg0202 /Sheet6` leftover 22,027。
- 29-A follow-up 应构建 bounded Sheet leftover window extractor：
  - 输入 byte-audit leftover ranges + source stream bytes。
  - 输出 top unknown record group table。
  - 分组维度：candidate PSM type code、bytes-to-follow、size bucket、marker bytes。
  - 每组映射到 typed / audit-only / probe-only / unknown parser status。
  - 不得命名字段或 promotion。
- 已新增 read-only probe：
  `examples/probe_phase29_sheet_leftover_windows.rs`。
- 已生成 bounded window 报告：
  `docs/analysis/2026-06-08-phase29-sheet-leftover-windows.md`。
- bounded window 报告当前 top local byte-shape groups：
  - #1 `top-level Sheet / 0x0001 unknown / btf 1024+ / prefix 01 00 FB FF`，leftover 16,760，4 fixtures。
  - #2 `top-level Sheet / 0x0002 unknown / btf 1024+ / prefix 01 00 01 00`，leftover 10,787，3 fixtures。
  - #3 `top-level Sheet / 0x0001 unknown / btf 0512-1023 / prefix 01 00 01 00`，leftover 10,537，3 fixtures。
  - #4 `top-level Sheet / 0x0005 unknown / btf 1024+ / prefix 01 00 01 00`，leftover 9,687，3 fixtures。
  - #5 nested `JSite204/Sheet6` 以 `44 F5 90 6C` cluster magic 开头，说明它更像嵌套 cluster-family Sheet stream，需要先决定 ownership / registration，而不是按 top-level `/Sheet6` 同等处理。
- 重要保守解释：报告里的 “candidate PSM-like header / type code” 是 leftover
  window 内局部字节形状，不等于已确认 record type；下一步需要人工对照 typed /
  audit-only decoder，再决定 `NeedsParser` / `NeedsIDA` / `Blocked`。
- 已新增 manual review：
  `docs/analysis/2026-06-08-phase29-sheet-leftover-review.md`。
- 关键发现：当前 byte-audit 对 top-level Sheet streams 只运行
  `probe_sheet_stream`，并额外扫描 26-byte endpoint records；它没有调用
  Phase 14+ typed / audit-only Sheet decoders。因此 Sheet leftover 不能直接等同于未知格式。
- 当前 review 分类：
  - Top groups #1-#4（`0x0001` / `0x0002` / `0x0005` local shapes）= `NeedsShapeReview + NeedsIDA`；不能直接写新 decoder。
  - Nested `/JSite204/Sheet6` = `NeedsRegistration + OwnershipDecision`；cluster magic `44 F5 90 6C` 表明它不是 top-level `/Sheet6` 同类。
  - `0x00CE igSymbol2d` group = `NeedsByteAuditIntegration`，因为 typed decoder 已存在但 byte-audit 未 claim。
- 推荐下一步改为 Phase 29-B：把现有 Sheet typed / audit-only decoder ranges 接入 byte-audit trace，先降低“已 decoded 但未 claim”的 leftover 噪声，再重跑 bounded window probe。

## Phase 29-B 关键结论：Sheet byte-audit trace integration（2026-06-08）
- 新增结果说明：
  `docs/analysis/2026-06-08-phase29-sheet-byte-audit-trace-integration.md`。
- `src/byte_audit/aggregate.rs` 现在会在 top-level Sheet trace 中声明已有 Sheet decoder byte ranges：
  - common Sheet cluster-family header。
  - `Decoded` confidence：`GLine2d`、`igLine2d`、`igLineString2d`、`igPoint2d`、`igTextBox`、`igSymbol2d`、`JStyleOverride`。
  - `Probed` confidence：audit-only `GraphicGroup` 与 `0x0010`。
- 接入后暴露出旧的聚合问题：`ParserTrace::consumed_bytes()` 原先按
  `consumed_ranges` 求和，会对不同 confidence 的重叠 ranges 双计数，导致
  `/Sheet6` coverage ratio > 1。
- 已修复为 `total_bytes - leftover_bytes`，即按 union 统计 consumed bytes。
- 新增 tests：
  - `consumed_bytes_counts_union_across_confidence_overlap`
  - `sheet_typed_decoders_claim_known_record_ranges`
- `cargo test --lib byte_audit -- --nocapture` 通过：41 passed。
- 6-fixture snapshot matrix 已重跑；`/Sheet6` coverage 明显提升：
  - `d06`: 0.8568656
  - `nonascii-process-1`: 0.8194456
  - `dwg0201`: 0.9259647
  - `dwg0202`: 0.89802366
  - `publish-a01`: 0.855618
  - `publish-dwg0202`: 0.89772105
- 这证明早先大块 Sheet leftover 主要是“已有 decoder 未进入 byte-audit trace”，而不是全新未知格式。
- after-trace 剩余 Sheet groups 已写入：
  `docs/analysis/2026-06-08-phase29-sheet-leftover-windows-after-trace.md`。
- 后续若继续 Sheet unknown work，应使用 after-trace report，而不是 pre-integration
  `phase29-sheet-leftover-windows.md`。

## Phase 29-C 关键结论：Sheet after-trace remaining groups review（2026-06-08）
- 新增 review：
  `docs/analysis/2026-06-08-phase29-sheet-after-trace-review.md`。
- after-trace 剩余 groups 已不再是 broad `0x0001` / `0x0002` local shapes；
  主要集中在：
  - nested `/JSite204/Sheet6` cluster-family stream。
  - `0x00CE` variants 当前未被 conservative `decode_igsymbols` claim。
  - 少量 `0x0084 igLineString2d` reject。
  - header/residual numeric payload 小组。
- 分类：
  - nested `/JSite204/Sheet6` = `NeedsRegistration + OwnershipDecision`。
  - `0x00CE CE 00 71/79` = `NeedsSymbolRejectProbe`。
  - `0x00CE CE 80 71/79` = `NeedsIDAOrVariantEvidence`，因为 type flags 非零。
  - `0x0084 igLineString2d` = `NeedsLineStringRejectProbe`。
- 推荐下一步：Phase 29-C1 Symbol Reject Probe。
  - 目标是解释 `0x00CE` rejects 的具体原因：type flags、btf range、payload truncation、f64 invalid/out-of-domain、或其它。
  - 不在同一 slice 放宽 `decode_igsymbols` validation。
  - 如果非零 type flags 占主导，需 IDA 或 controlled fixture evidence。
- Phase 29-C1 已完成：
  - 新增 `examples/probe_phase29_igsymbol_rejects.rs`。
  - 生成 `docs/analysis/2026-06-08-phase29-igsymbol-reject-probe.md`。
  - 所有剩余 `0x00CE` candidates 的拒绝原因均为 `out_of_domain_double`。
  - 未出现 `bytes_to_follow_out_of_range`、`payload_truncated`、`missing_double_payload`、`non_finite_double` 或 `accepted_but_still_leftover`。
  - type flags 分布包含 0 与 2；非零 flags 样本仍需 IDA / controlled fixture evidence，不应直接放宽。
  - 结论：当前不修改 `decode_igsymbols` validation；这些 symbol-like leftovers 可能是 `0x00CE` variants、wrapper/tail 命中，或当前 transform offset 假设不适用的记录，均不能在无 IDA 证据时 promotion。
- Phase 29-C2 已完成：
  - 新增 `docs/analysis/2026-06-08-phase29-nested-sheet-ownership-review.md`。
  - `publish-a01` 的 `/JSite204` 包含成套 nested PSM-style streams：
    Summary / AppObject / DocVersion / Dynamic Attributes Metadata /
    PSMcluster0 / PSMclustertable / PSMroots / PSMsegmenttable /
    PSMspacemap / Sheet* / StyleCluster / Unclustered Dynamic Attributes。
  - 这说明 `/JSite204/Sheet6` 不是孤立 top-level Sheet-like stream，而是
    nested symbol-local / embedded-fragment package 的一部分。
  - Cross-fixture 上 `JSite793`、`JSite204`、`JSite145`、`JSite151` 等都存在
    unregistered child stream groups。
  - 结论：不要把 nested `JSite*/Sheet*` 直接接入 `top_level_sheet_name()`；
    先做 Phase 29-D nested JSite package inventory。
- Phase 29-D 已完成：
  - 新增 `examples/probe_phase29_nested_jsite_inventory.rs`。
  - 生成 `docs/analysis/2026-06-08-phase29-nested-jsite-package-inventory.md`。
  - 分类输出 `NeedsOwnership` / `CanTraceHeaderOnly` /
    `IgnoreUntilConsumerNeeds`。
  - `publish-a01 /JSite204` 是唯一当前带 nested `Sheet*` children 的
    `NeedsOwnership` package：25 child streams，19,760 bytes。
  - `JSite793`、`JSite329`、`JSite7559`、`JSite145` 等为
    `CanTraceHeaderOnly`：有 PSM / Style / registry child streams，但无 nested
    Sheet children。
  - 大量 JProperties-only / OLE-only symbol instances 归类为
    `IgnoreUntilConsumerNeeds`。
  - 下一步决策：是否对 `CanTraceHeaderOnly` nested JSite cluster-family child
    streams 做 header-only byte-audit trace；这仍不应影响 top-level geometry。
- Phase 29-D follow-up 已完成：
  - nested JSite cluster-family child streams 已添加 header-only byte-audit trace。
  - 范围只包括一层 nested JSite child：`PSMcluster0`、`StyleCluster`、
    `Dynamic Attributes Metadata`、`Sheet*`。
  - nested `JProperties` 仍走 `parse_jproperties`，避免被 header-only branch 抢占。
  - 新增 tests：
    `nested_jsite_cluster_header_gets_header_only_trace` 与
    `nested_jsite_jproperties_still_uses_jproperties_parser`。
  - `cargo test --lib byte_audit -- --nocapture` 通过：44 passed。
  - 6-fixture snapshots 已重跑，unregistered path 数下降：
    - `d06`: 33 → 27
    - `nonascii-process-1`: 32 → 26
    - `dwg0201`: 33 → 27
    - `dwg0202`: 21 → 18
    - `publish-a01`: 55 → 40
    - `publish-dwg0202`: 21 → 18
  - 该变更是 accounting-only，不递归解析 nested package，不影响 top-level geometry。
- Phase 29-E 验证收口：
  - `cargo test --lib byte_audit -- --nocapture` 通过，44 passed。
  - `cargo fmt --all -- --check` 通过。
  - ReadLints 无错误。
  - Phase 29-A..E 当前可收口为 byte-audit accounting / evidence backlog 改进完成；剩余工作需要 IDA、controlled fixture 或下游明确语义需求。
- Phase 28-C IDA 可达性核查（2026-06-10）：
  - 可达实例仅 `core.dll`（AVEVA E3D，无关）与 `radsrvitem.dll`。
  - `style.dll` / `J2DSrv.dll` / `sppid.dll` / `XCeedRAD.dll` /
    `smartplantpid.exe` 均未打开；deep `0x0010` 与普通 geometry reader
    确认维持 blocked，Phase 28-D 继续 pending。
- 文档漂移修正（2026-06-10）：
  - `AGENTS.md` 0x0030 旧 GArc2d 叙述更正为 JStyleOverride（含测试表、
    家族表、key insight、caveat、SheetGeometry 字段列表 5 处）。
  - `tasks.md` Phase 28-C / 28-F / Validation Tasks 选框回写。
- Phase 29-F PSMcluster0 body 三角化（candidate Slice 29-B，2026-06-10）：
  - 新增 `examples/probe_phase29_psmcluster0_body_triage.rs`。
  - [结构] `/PSMcluster0` post-string-table body 是**单条连续 PSM envelope
    record 链**：6/6 fixture 从 offset 145 到流尾零 resync，覆盖
    leftover 99.81%–99.99%；唯一未解释字节是 6 fixture 完全一致的
    10 字节 prologue（`135..145`，`00 00 01 00 00 00 00 00 00 00`）。
  - [不变量] `chain_records == header.record_count - 2` 在 6/6 fixture
    成立（60/62、442/444、194/196、229/231、43/45、231/233）——
    `record_count` 字段语义首次获得 PSMcluster0 family 交叉佐证。
  - [分布] type code 分布跨 fixture 稳定：`0x0089` 主导，`0x0003` /
    `0x0081` / `0x00EC` 复现，链首恒为 `0x0002`；
    **namespace 警告**：不得未经 IDA 证据把这些 code 映射到 Sheet ig*
    表。
  - [纹理] payload 含 ASCII 属性名（`_SmartText`、`FormatString`、
    `ModelItemType`、`ModelID`、`PropertyFormat` 等）与 UTF-16LE run
    （`BsplineExtensionMode`、`Section1`、`NotClaimed` 等），属性/格式
    catalog 特征而非几何。
  - [caveat] `ClusterHeader.body_len` 在 6 fixture 恒为 113 而流长
    6K–79K，字段名是历史猜测，未经 IDA 不改名。
  - 产出：`docs/analysis/2026-06-08-phase29-psmcluster0-leftover-triage.md`
    （含 audit-only walker parser backlog + 测试靶点 + IDA target
    request）。
- Phase 29-F walker 实现（同日落地）：
  - `src/parsers/cluster_header.rs` 新增
    `decode_psm_cluster0_record_at` / `decode_psm_cluster0_records` /
    `decode_psm_cluster0_body_records`（full-coverage gate：prologue +
    链必须精确覆盖到流尾，否则不做任何 partial claim）。
  - `parse_psm_cluster0_with_trace` 扩展：prologue `Probed`、record
    envelope `Decoded`、payload `Probed`。
  - 测试：8 个新单测 + 3 个 panic-safety 入口 +
    `psmcluster0_body_chain_matches_record_count_invariant`
    cross-fixture ratchet（6/6 fixture，`record_count - 2` 成立，
    `/PSMcluster0` consumed ratio = 1.0）。
  - 6-fixture 快照重新生成：`/PSMcluster0` family leftover 193,173 → 0；
    全文件 ratio 提升至 0.168–0.585（nonascii-process-1 最高 0.585）。
  - 五项提交级门禁全部通过（build / test 885 lib + 全 integration /
    clippy -D warnings / fmt / missing-docs ratchet 0=0）。
  - 注意：`bash` 在本机不可用，missing-docs ratchet 以 PowerShell 等价
    命令执行（`cargo rustdoc --lib --locked -- -W missing-docs` + 计数
    对比 baseline）。
- Phase 29-G StyleCluster body 三角化 + walker（同日落地）：
  - 泛化探针：`probe_phase29_psmcluster0_body_triage` 接受 stream 名参数。
  - [结构] `/StyleCluster` = 16B header（`stream_type 0x005A` /
    `flags 0x2000`）+ 变长未解析 prefix（10 零字节 + `u16` 计数 0x000D +
    连续 16B CLSID 形条目 + 其余未特征化，1,529–2,360 字节）+
    **单条 end-anchored PSM record 链**（6/6 fixture 零 resync 恰好到
    流尾，链起点 2376/1545/1899/2267/2042/2267）。
  - [警告] `record_count == chain_records` 仅 2/6 fixture 成立
    （d06 47、publish-a01 42；其余 84/135/180 vs 77/76/78），
    PSMcluster0 式 envelope 佐证**不可迁移** → StyleCluster record
    整条 `Probed`（含 envelope）。
  - [警告] StyleCluster 链 type codes（`0x002C/0x002D/0x002E` 主导）与
    Sheet ig* 表数字碰撞（0x0018/0x0020/0x0030/0x007B），在 style 流里
    显然不是几何 —— namespace 判定必须等 IDA。
  - [纹理] payload 含 `Normal` / `Viewport` / `Office Automation` /
    `As Drawn` / `Nozzle - New` / `Arial Narrow` / GB2312 字形 ——
    style catalog 特征。
  - 实现：共享核心改名 `ClusterBodyRecordDecoded` /
    `decode_cluster_body_record_at` / `decode_cluster_body_records`；
    新增 `decode_style_cluster_body_records`（earliest end-anchored
    chain locator，≥3 records gate）+ `parse_style_cluster_with_trace`
    （records 全 `Probed`，prefix 留 leftover）；aggregate.rs
    `/StyleCluster` 分支切换到新 walker（parser_name
    `parse_style_cluster`）。
  - 测试：cluster_header 22 passed（新增 6）；panic-safety 新增入口；
    `stylecluster_body_chain_is_end_anchored_across_fixtures`
    6/6 通过（consumed 0.631–0.898，链起点与探针完全一致）。
  - 快照：`/StyleCluster` family leftover 83,468 → 12,300（仅剩 prefix）；
    全文件 ratio 升至 0.226–0.626。
  - 五项门禁全过（test exit 0 / clippy / fmt / missing-docs 0=0）。
  - 产出：`docs/analysis/2026-06-08-phase29-stylecluster-leftover-triage.md`。
- Phase 29-H StyleCluster prefix 特征化（documentation-only closeout）：
  - 新增 `examples/probe_phase29_stylecluster_prefix.rs`。
  - [结构] prefix = 12 字节 opener（10 零字节 + u16 计数 13，跨 fixture
    恒定且与 prefix 长度无关）+ **532 字节跨 fixture 逐字节相同的
    boilerplate**（`[16..548)`，含真实 COM GUID）+ fixture 专属
    42 字节 slot 样式区（`Normal`×5 @562/604/646/688/730 等距，
    与 body 三角化的 14/28/42 stride 自相关吻合）。
  - [否定] GUID 区不是均匀 stride 数组：stride 16/20/24/28/32 各只有
    5/7/5/2/6 个条目过 GUID 合理性检查 → "count × 固定条目" parser
    不成立。
  - [决策] 文档性 closeout：模板匹配 532 常量字节 = byte-comparison
    而非 parsing，不上 parser；prefix 保持 leftover；IDA target
    扩展为 stream_type 0x005A writer/reader + count 13 + GUID 条目
    布局 + 42 字节 slot record。
  - 产出：
    `docs/analysis/2026-06-08-phase29-stylecluster-prefix-characterization.md`。
- Phase 29-I Unclustered DA body 三角化（candidate Slice 29-C，2026-06-11）：
  - 前置核查：`ida-pro-mcp list_instances` 仍仅 `core.dll`（无关）+
    `radsrvitem.dll`，目标 IDB 未打开 → 29-F 维持 blocked，继续非 IDA 路线。
  - 新增 `examples/probe_phase29_da_body_triage.rs`（DA 专用：全流 envelope
    链测试 + landmark 对齐 + 属性 census）。
  - [结构] DA stream = 8 字节 prologue（cluster-family magic `0x6C90_F544`
    + u32 record counter）+ **单条 end-anchored `0x0089` envelope 链**
    （6/6 fixture，覆盖 0.9978–0.9998，零 resync、零 tail gap）——
    与 `/PSMcluster0` body 同构，DA 流并入 cluster-family。
  - [对齐] 417/417 个 signature-valid "31 字节 trailer" offset 与链 record
    head 重合：Phase 11/12 的 trailer 解读与 envelope head 解读是同一段
    字节（`89 00` + btf + record_id + 8 零 + field_x + FFFF + class_id +
    `14 00 00`，head+31 即 ASCII class name）；首条 record 无前驱故
    trailer 计数恒等于 class-name 命中数减一。
  - [计数] prologue counter == 字面 `0x89 0x00` record 数 6/6；==
    严格链 record 数仅 5/6（`nonascii-process-1` 有一条 head 带高位
    flag：`word & 0x3FFF == 0x0089` 但字面非 `89 00`）→ walker 不能把
    counter 当硬 invariant（StyleCluster 先例：envelope 走 `Probed`）。
  - [census] 高频属性 `DrawingID/DrawingItemType/DrawingNo/Flag/
    ProjectNumber`（439）、`ModelID/ModelItemType`（433）已被 object
    graph / D06 relationship fallback 消费；census 同时暴露 heuristic
    噪声（`.sym` 路径渗入属性名、24-hex 片段名）—— 链边界是 per-record
    scoping 的前置条件。
  - [决策] parser-only 合理：backlog = audit-only
    `decode_unclustered_da_body_records` + `parse_unclustered_da_with_trace`
    （全 `Probed`，end-anchored full-coverage gate，与
    `scan_da_landmarks_with_trace` 并存），ratchet 锁 records
    {47/69/231/169/22/169} + consumed ratio 1.0；预期 leftover
    111,120 → 0。
  - 产出：
    `docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`。
- Phase 29-I follow-up DA walker 落地（同日）：
  - `decode_unclustered_da_body_records`：magic gate（`0x6C90_F544`）+
    固定链起点 8 + end-anchored full-coverage gate；counter 只报告不
    gate（5/6 匹配先例）。
  - `parse_unclustered_da_with_trace`：prologue + 全部 record 走
    `Probed`；gate 失败零 claim。aggregate DA branch 合并 walker 与
    `scan_da_landmarks_with_trace`（landmark `Decoded` 不变），parser
    name 改为 `parse_unclustered_da`。
  - 测试：cluster_header 29 passed（新增 7）；byte_audit 45 passed
    （含新 aggregate 测试 + 87 字节 landmark 测试改名）；panic-safety
    新增入口；ratchet
    `da_body_chain_is_end_anchored_across_fixtures` 6/6
    （records 47/69/231/169/22/169，type 全 `0x0089`，leftover=0）。
  - 快照：`/Unclustered Dynamic Attributes` family leftover
    111,120 → 0；全文件 ratio：d06 0.3793 / nonascii 0.6700 /
    dwg0201 0.5912 / dwg0202 0.5923 / publish-a01 0.2733 /
    publish-dwg0202 0.5925。
  - 五项门禁全过；[边界] per-record attribute scoping 与 head 字段
    surfacing 是 follow-up，不在本 walker 范围。
- Phase 29-J nested JSite cluster body 分派（同日，"按推荐方案继续"）：
  - [结构] 23/23 个一层 nested `JSite*` cluster 流用**未改动**的
    top-level walker 直接 end-anchored 走通：nested `PSMcluster0`
    11/11 保持 `record_count - 2` invariant（链起点统一 145）、
    nested `StyleCluster` 11/11 earliest end-anchored 链、nested
    `Unclustered Dynamic Attributes` 1/1（counter=4=records）——
    nested 包与 top-level 是同一 writer 输出。
  - 实现：aggregate nested branch 由 header-only 升级为按 child
    分派（`parse_psm_cluster0` / `parse_style_cluster` /
    `parse_unclustered_da`）；`Sheet*` / `Dynamic Attributes
    Metadata` 维持 header-only（ownership / semantic review 边界
    不破）；nested DA 加入 nested registry（publish-a01
    unregistered 40 → 39）。
  - 测试：aggregate dispatch 单测 + 23-stream ratchet
    `nested_jsite_cluster_bodies_are_end_anchored_across_fixtures`
    （nested PSMcluster0 / DA leftover=0；nested StyleCluster
    leftover == prefix bytes）。
  - 快照：`JSite*` family leftover 325,843 → 74,559（ratio
    0.0460 → 0.7817）；全文件 ratio：d06 0.7699 / nonascii 0.8201 /
    dwg0201 0.8804 / dwg0202 0.8474 / publish-a01 0.6297 /
    publish-dwg0202 0.8474。
  - [边界] 剩余 JSite leftover = nested PSMspacemap 页（IDA-gated）+
    nested `/JSite204/Sheet6`（ownership-gated）+ StyleCluster
    prefix（IDA-gated）+ PSMroots / OLE 小 payload（demand-gated）。
  - 五项门禁全过。
- Phase 29-K per-record DA attribute scoping（同日，Slice C named
  benefit #1）：
  - 重构：`try_parse_record` 的 section-body 解析抽为
    `parse_section_body`（提取逻辑 byte-for-byte 不变，legacy 行为
    零漂移）。
  - 新增 `parse_attribute_records_chain_scoped`：chain gate 通过时
    按 record 精确边界（head+6 .. byte_range.end）解析 attribute
    section；找回 flagged-head record（字面非 `89 00` 但 masked
    `0x0089`）；payload 内伪 `89 00` 永不开 phantom section；
    gate 失败 byte-for-byte 回退 legacy。
  - 管线：`streams/dynamic_attrs.rs`（DA 文档管线）切换 chain-scoped；
    `streams/cluster.rs`（泛用 cluster 流）保持 legacy —— 避免 DA
    walker 的 magic+chain gate 在非 DA cluster 流上意外触发。
  - [结果] ratchet 6/6：nonascii 68 → 69（找回 `Symbol` class 记录），
    d06/dwg0201/dwg0202/publish-a01/publish-dwg0202 不变 —— 全部
    既有 baseline（D06=47 等）零回归；快照不受影响。
  - [边界] head-field surfacing（benefit #2，非 signature record 的
    record_id/field_x/class_id 提升）仍 IDA-gated。
  - 五项门禁全过。
- Phase 29-L nested JSite registry 分派（2026-06-12 收尾）：
  - [结构] 68 个 nested registry 流复用 top-level 格式（探针总
    consumed 98.4%）：DocVersion2/3 / PSMclustertable /
    PSMsegmenttable 全量解析；nested PSMroots 与 top-level 同款
    4 字节尾；4 字节 AppObject stub 被 parser gate 干净拒绝
    （registered + 0 claim）；JSite204 summary pair 部分解析
    （0.875 / 0.921，与 top-level 形态一致）。
  - 实现：`nested_jsite_registry_parser` helper + nested branch
    分派 8 类 registry child 到既有 top-level trace parser；
    `JSitesList`（top-level + nested）无 parser，维持 unregistered
    （demand-gated）。
  - 测试：dispatch 单测 + ratchet
    `nested_jsite_registry_streams_reuse_top_level_parsers`
    （DocVersion2/3 / clustertable / segmenttable leftover=0，
    PSMroots leftover=4，summary consumed>0）。
  - 快照：JSite family leftover 74,559 → 66,778（ratio 0.8045）；
    unregistered paths 27/26/27/18/39/18 → 15/14/15/12/19/12；
    全文件 ratio：d06 0.7849 / nonascii 0.8257 / dwg0201 0.8877 /
    dwg0202 0.8516 / publish-a01 0.6644 / publish-dwg0202 0.8516。
  - [边界] common unregistered 收敛为：JSitesList（×6，无 parser）、
    PSMspacemap 4 页（×6，IDA-gated）、TaggedTxtData/Revision（×5）、
    JSite OLE payload（×3）+ nested 同族 count-1 尾部。
  - 五项门禁全过（test/clippy/fmt 严格退出码 0，missing-docs 0=0）。
- Phase 29-M JSitesList / Revision 清尾（2026-06-12）：
  - [结构] `/JSitesList` = `"OLEM"`(4F 4C 45 4D) + u32 count +
    4 字节对齐 u32 slot 表；**逻辑 entries（前 count 个 slot）与
    `JSite<id>` storage id 在 6/6 fixture 上全量一一对应**
    （9/9、10/10、20/20、13/13、5/5、13/13）—— 该流就是 JSite
    symbol-instance storage 目录。
  - [陷阱] dwg0202 / publish-dwg0202 的 slot 表有 16 个 slot 但
    count=13：3 个 stale 尾 slot 重复逻辑值（793, 4458, 4458）——
    首版 exact-size gate 被 ratchet 当场证伪；修正为
    `len >= 8+4*count` + 4 对齐，stale 尾不 claim（留 leftover，
    PSMroots 4 字节尾先例）。
  - [结构] `/TaggedTxtData/Revision` = 0 字节占位（5/5）；注册为
    `revision_empty_stream` 零 claim，未来内容会以 registered
    leftover 形式显形。
  - 实现：`src/parsers/jsites_list.rs`（DTO 字段名 `entries` /
    `trailing_slots`，不命名 `jsite_ids`，IDA 确认前只记录关联
    证据）；顶层 + nested（Slice L registry 表）+ Revision 注册；
    8 单测 + panic-safety + ratchet。
  - 快照：unregistered 51 → 38 distinct（9–14/fixture）；剩余
    multi-fixture unregistered 全部 IDA（PSMspacemap 页）/ demand
    （\x01Ole）gated；全文件 ratio 0.665–0.888。
  - [修复] clippy `manual_is_multiple_of` 告警（jsites_list 对齐
    检查改用 `is_multiple_of`）。
  - 五项门禁严格全绿。
- Phase 30-A radsrvitem.dll JSite IDA refresh（2026-06-12）：
  - [IDA] 当前可达 `radsrvitem.dll` 未命中 `"OLEM"` /
    `JSitesList`，所以 `/JSitesList` writer/reader 与 stale tail
    语义仍未解；`style.dll` / `J2DSrv.dll` / `sppid.dll` /
    `XCeedRAD.dll` / `smartplantpid.exe` 仍是后续解锁目标。
  - [证据] `sub_56448A10` 与 `sub_56448A70` 均格式化
    `JSite<id>`；`sub_5646FF60` 接收整数 id，构造 `JSite<id>` 并
    调用 storage open path。`sub_5645FF00` / `sub_56460330` 的
    调用链从 record/runtime context 取 id 后打开对应 JSite storage。
  - [决策] Phase 29-M 的 `/JSitesList.entries` ↔ `JSite<id>`
    storage id 关联获得 IDA 旁证加强，但 DTO 仍保持
    `entries` / `trailing_slots`，不改名为 `jsite_ids`；需要 writer
    侧 `"OLEM"` / `JSitesList` 证据后再升级。
  - [边界] 既有 `sub_5644B640` 的 `*record == 137` 过滤只证明
    `0x0089` 是真实 runtime/persisted record type filter，尚不能
    命名 record family 或提升 DA/PSMcluster0 head 字段语义。
  - 产出：
    `docs/analysis/2026-06-12-phase30-radsrvitem-jsite-ida-refresh.md`。
- Phase 30-B/C 0x0089 export + PSMspacemap handle IDA refresh（同日）：
  - [IDA] `sub_5644B640(a3=0)` 从 record-id list 逐项取 runtime
    record pointer 并过滤 `*record == 137`；`a3=1` 调
    `sub_56445F40` 导出单条 record。
  - [边界] `sub_56445F40` 对 `0x0089` 走 default
    `sub_564462F0`；`sub_56448F70` type-code → RAD type-name 表
    包含 `igLine2d` / `igTextBox` / `igPoint2d` /
    `igLineString2d` / `igSymbol2d` 等，但不包含 `0x0089`；
    因此当前 RAD export path 只写 `RAD_OBJECT_TYPE = "137"` 和
    `"RELEATIONS"` id 列表，不解 DA/PSMcluster0 head fields 或
    ASCII class payload。
  - [IDA] `PSMspacemap` 字符串命中 storage load/save 与
    `ClusterTable::GetSpaceMapSegment()`；`tseg` 字符串 0 hits。
  - [结构] `sub_5648C370` 证明 cluster table entry 维护
    segment-id array；复用 segment 时跳过 flags `0x08`，用
    `sub_56479EE0` 判断可用性；无可用 segment 时 `sub_56479210`
    分配并 append 新 segment id。
  - [结构] segment in-memory layout：`+12` = `m_iNext`，`+22` =
    flags，`+10/+16/+20` 参与 reusable/free-list；`m_iNext <
    0x2000` 或 free-list 非空表示可继续使用，否则 `flags |= 0x08`。
  - [结构] `sub_56479040` / `sub_56479C20` 证明 handle =
    `(segment_id << 13) | entry_index`，`entry_index < 0x2000`；
    低 13 bit 是 segment 内 entry index，高位是 segment id。
  - [边界] 这只证明 handle/segment 选择模型，尚未直接证明
    `/PSMspacemap` raw page byte layout；byte-audit 不应升级 page
    body claim。
  - 产出：
    `docs/analysis/2026-06-12-phase30-radsrvitem-record-spacemap-ida.md`。
- Phase 30-D Style / JStyle low-cost negative pass（同日）：
  - [IDA] 可用 IDB 仍仅 `core.dll` / `radsrvitem.dll`；`StyleCluster`
    0 hits，`JStyleOverride` 0 hits。
  - [IDA] `JStyle` 命中集中在 `JStyleBase` /
    `JStyleBase::IJPersistImp` / `IJManageStyle2Imp` /
    `IJStyleCopyImp` / `IJStyleUserImp` 的 RTTI、vtable 和 thunk。
  - [边界] `sub_5655D200` 只构造 `JStyleBase` base object；
    `sub_5655DB60` / `sub_5655DBE0` 只把
    `"JStyleBase::IJPersistImp"` 转发给 base vtable slots（`+52` /
    `+48`），不是直接 Load/Save persistence body。
  - [决策] 当前 `radsrvitem.dll` 不能解 `0x0030` persistence fields
    或 StyleCluster prefix layout；Phase 16/17 的 `0x0030 =
    JStyleOverride` 结论保持，深层字段仍 gated on `style.dll` /
    `J2DSrv.dll` 等 writer/reader 模块。
  - 产出：
    `docs/analysis/2026-06-12-phase30-radsrvitem-style-jstyle-negative.md`。
- Phase 30-F IDA-gated next actions handoff（同日）：
  - [收口] 当前可达 `radsrvitem.dll` 的低成本路线已全部收口：
    JSite naming/open、`0x0089` export boundary、PSMspacemap handle
    model、Style/JStyle negative pass。
  - [决策] 在未打开新 IDB 前，不建议继续 broad search；剩余问题需要
    `style.dll` / `J2DSrv.dll` / `sppid.dll` / `XCeedRAD.dll` /
    `smartplantpid.exe` 或 controlled fixture。
  - [交接] 下一次 IDA 优先搜索：`JSitesList` / `OLEM`；
    `P&IDAttributes` / `Dynamic Attributes` / `137`；`PSMspacemap` /
    `GetSpaceMapSegment` / `0x2000` / `0x1FFF`；`StyleCluster` /
    `JStyleOverride` / `IJPersistImp`；`0x0010` / `GraphicGroup`。
  - 产出：
    `docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md`。
- Phase 30-G worktree readiness check（同日）：
  - [状态] 当前工作树是 Phase 29/30 累积变更：tracked modified
    16 files（diff stat 4552 insertions / 109 deletions），另有
    Phase 26/27/29/30 docs、spec-kit、probe examples、
    `src/parsers/jsites_list.rs` 等 untracked files。
  - [验证] `ReadLints` scoped to关键 Rust parser/test files 无错误。
  - [风险] `git diff --stat` 对若干 Rust 文件提示 LF→CRLF warning；
    提交前需确认是否只是仓库行尾策略噪声。
  - [决策] 无新 IDB 时，下一步不再 broad search；应运行最终
    pre-commit gate 后提交/评审，或按 parser/byte-audit、spec/docs、
    Phase 30 IDA docs 拆分。
  - 产出：
    `docs/analysis/2026-06-12-phase29-30-worktree-readiness.md`。
- Phase 30-H final pre-commit gate run（同日）：
  - [验证] `cargo fmt --all -- --check`、`cargo build --locked
    --workspace --all-targets`、`cargo test --locked --workspace
    --all-targets`、`cargo clippy --locked --workspace --all-targets
    -- -D warnings` 全部通过。
  - [验证] `bash .github/scripts/check-missing-docs.sh` 在本地
    Windows bash 环境仍失败（乱码 / `REGDB_E_CLASSNOTREG`），按项目
    既有记录不采信为代码失败；fallback `cargo rustdoc --lib
    --locked -- -W missing-docs` 通过。
  - [决策] 当前可执行本地门禁已通过；下一步需用户授权提交/拆分，或
    打开新 IDB 继续 IDA-gated 问题。
- Phase 30-I commit/review plan（同日）：
  - [决策] 因用户未明确授权 commit，本轮不提交；改为新增
    commit/review plan。
  - [方案] 当前 worktree 可走 single milestone commit；若要拆小 review，
    推荐三段：Phase 29 parser/byte-audit、Phase 29 probes/spec/docs、
    Phase 30 IDA evidence/handoff。
  - [交接] 文档中已给出建议 commit message、关键 review 文件、后续
    command sequence 与 residual risks。
  - 产出：
    `docs/analysis/2026-06-12-phase29-30-commit-review-plan.md`。
- Phase 30-J focused self-review（同日）：
  - [验证] `git diff --check` 无 whitespace errors（仅 LF→CRLF
    warning）；ReadLints scoped to关键 Rust parser/test files 无诊断。
  - [审查] cluster body walker 使用 checked arithmetic / length cap /
    payload fit gate，且 zero-payload record 仍前进 6-byte envelope；
    DA chain-scoped extraction 有 full-coverage gate 与 legacy fallback；
    JSitesList parser checked count math、4-byte alignment、stale tail 不
    claim；nested JSite 未知 child 保持 unregistered。
  - [结论] 未发现 blocking code issue；剩余风险仍是大 diff、
    LF→CRLF warning、以及 IDA-gated semantic questions。
  - 产出：
    `docs/analysis/2026-06-12-phase29-30-self-review.md`。

## 关键文件（Phase 13-21 补丁）
- `goals/phase14-sppid-sheet-geometry/`
- `goals/phase15-graphic-group-records/`
- `goals/phase16-j2dsrv-record-decode/`
- `goals/phase17-primitive-arc-deprecation/`
- `goals/phase18-psm-0x0010-sub-record/`
- `goals/phase19-psm-0x0010-leading-word-audit/`
- `goals/phase20-psm-0x0010-ida-class-identity/`
- `docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md`
- `docs/plans/2026-05-14-phase15-graphic-group-final-summary.md`
- `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md`
- `docs/plans/2026-05-17-phase20-ida-rad-class-roadmap-cn.md`
- `docs/plans/2026-05-18-phase21-d06-parse-coverage-plan-cn.md`
- `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`
- `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md`
- `docs/analysis/2026-05-18-d06-relationship-gap.md`
- `docs/analysis/2026-05-18-d06-sheet6-audit-inventory.md`
- `examples/probe_psm_0x0010_shape.rs`
- `examples/probe_psm_0x0010_sub_kind.rs`
- `examples/probe_rad_siblings_0x0029_0x0035.rs`
