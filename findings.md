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
