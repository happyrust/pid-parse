# 变更日志

## [Unreleased]

### Publish writer Stage-1 — fidelity ratchet (A12 → A29b)

把 SmartPlant Publish Data XML writer 的 fidelity 守门从"tag 计数级"
逐层加固到"接口级"再到"属性级"，并把对照范围从"writer vs A01
reference"扩展到"A01 vs DWG 跨 fixture"。这一系列工作不改变
writer 的字节输出（A25 引入了 PIDProcessVessel tank-variant 的条件
emit，是唯一例外），但建立了一套 8 道 regression gate，任何未来
的接口/属性 drift 会立即在 CI 上以"`(tag, interface, attr)`"
三元组失败定位。

#### Added — fidelity 分析层（src/publish/diff.rs）

- A12 `parse_pid_tag_counts` / `diff_publish_xml` /
  `SemanticDiffReport` — `<PIDxxx>` 开标签计数级语义 diff
- A15 `coverage_against_reference` / `WriterCoverage` /
  `supported_pid_tags` — 把 reference 标签集分为"writer 已支持"
  和"backlog"两桶
- A23 `parse_interfaces_per_tag` — 每个 PID tag 下 first-occurrence
  的接口名集合（`I*`）
- A26 `parse_attrs_per_interface_per_tag` — 每个 `(tag, interface)`
  对的属性名集合（值不计，只看 attr key）

#### Added — fidelity gate 层（tests/）

- A23 `tests/publish_interface_parity.rs` ·
  `interface_parity_on_a01_writer_matches_reference_superset_post_a22`
  + sanity sub-test：writer 必须 ⊇ A01 reference 的接口集
- A24 同文件 ·
  `a01_and_dwg_reference_interfaces_agree_for_every_shared_supported_tag`
  + 2 guard：A01 ↔ DWG fixture 接口集一致性 + whitelist 维护守门
- A27 `tests/publish_attribute_parity.rs` ·
  `attribute_parity_on_a01_writer_matches_reference_superset_per_interface`
  + sanity sub-test：writer 必须 ⊇ A01 reference 的属性集
  （per `(tag, interface)`）
- A27b 同文件 ·
  `a27b_a01_and_dwg_reference_attrs_agree_for_every_shared_tag_interface`
  + 2 guard：A01 ↔ DWG 属性集一致性 + whitelist 维护守门
- A28 `tests/publish_backlog_inventory.rs` ·
  `a28_backlog_tag_specs_match_reference_fixtures_exactly`
  + 3 guard：未支持 PID tag 的 fidelity spec snapshot
  （PIDPipingBranchPoint × 4 + PIDBranchPoint × 5 on DWG）
  作为未来 writer arm 的可执行 spec + 漂移检测

#### Added — writer 真实改动

- A13 `derive PIDPipingPort + PIDProcessPoint from connectors`
- A14 `filter annotation representations -> A01 fully clean`
- A16 `derive PIDSignalPort children from InstrFunction`
- A17/A18/A19/A20/A21/A22 `close PIDPipingComponent +
  PIDSignalConnector + upgrade 6 tags to full-interface SPPID
  canonical shape`
- A25 `PIDProcessVessel low-pressure-tank variant conditional
  emit`（DWG 17-interface tank shape；A01 15-interface drum
  shape；通过 `obj.fields["IsLowPressureTank"]` 路由）
- A29 `PublishStyle::{A01, Dwg}` enum + `PublishDrawing.style`
  字段（默认 A01）；writer 在 PIDPipeline / PIDPipingConnector /
  PIDProcessVessel 的 IObject 上按 style 切换：
  * A01 style：`<IObject UID="..." [Name="..."] ItemTag="..."
    [Description="..."]/>`（保持 pre-A29 字节级一致）
  * Dwg style：`<IObject UID="..." [Name="..."]
    [Description="..."]/>`（不发 ItemTag，匹配 DWG reference）
  把 pre-A29 "PipelineName 有值即触发 DWG-shape"的隐式数据驱动
  改为显式 style 选择，让 caller 明确表达 fixture flavor。
- A29b `pid_publish_xml --style a01|dwg` CLI 选项（默认 a01，
  大小写不敏感）。CLI 把选择透传给 `PublishDrawing.style`，
  让 ops 不写代码就能从命令行切换 fixture flavor。

#### Added — A27b whitelist（KNOWN_A01_VS_DWG_ATTR_DIVERGENCES）

15 条 fixture-side variant，分两类：

1. **IObject identifier rename**（3 项）— A01 export 用 `ItemTag`，
   DWG export 用 `Name`，业务标识相同但 attr key 不同。
   `PIDPipeline / PIDPipingConnector / PIDProcessVessel` 的 IObject
   都中招（PIDProcessVessel 的 DWG 实例没有 tag，退化为单边
   only_in_a01）。
2. **DWG-side 富化列**（12 项）— DWG plant 的源数据填了 SmartPlant
   多列（FluidCode / FluidSystem / EqType0..3 / EquipmentTrimSpec /
   FlowDirection / TotalInsulThick / SlopedPipingAngle / ...），
   A01 plant 的源是 NULL。Writer 当前按 SmartPlant 惯例"NULL 列就
   不发 attr"，所以这是 loader-side gap，需要 DWG SQLite mirror
   bundle 后才能填。

#### Tests

* lib：540 → 562（+22，A26 +7 `publish::diff::tests::parse_attrs_*`，
  A29 +7 `publish::xml_writer::tests` 中 IObject style 切换）
* integration：140 → 154（+5 在 `tests/publish_attribute_parity.rs`，
  +4 在 `tests/publish_backlog_inventory.rs`，
  +5 在 `tests/publish_xml_cli.rs` 覆盖 A29b CLI `--style` 行为）
* lint：0 warnings

#### A28 backlog inventory（已 snapshot 入测试）

PIDPipingBranchPoint（DWG × 4）：6 接口（IObject 仅 UID，其余 bare）
- IObject(UID), IConnection, IPipingConnection, IDrawingItem,
  IPipingBranchPoint, IDocumentItem

PIDBranchPoint（DWG × 5）：9 接口（IObject 含 UID + Name，其余 bare）
- IObject(UID, Name), IPIDBranchPoint, IDuctConnection, IConnection,
  IDrawingItem, IPipingConnection, ISignalConnection, IDocumentItem

UID 后缀模式：`<base>.BPT`，参考 A13 的 `.PPT` / `.1` / `.2`
派生 ID 模式（PipingConnector → PIDPipingPort + PIDProcessPoint）。
未来 writer arm 实现时按 spec 守门即可。

#### Backlog（A30+）

* PIDBranchPoint / PIDPipingBranchPoint writer arms（spec 已在
  A28 snapshot test 中 pin 住，实施时需 DWG 端 SQLite mirror
  才能反推源映射）
* A25b loader-side `IsLowPressureTank` 推断（同上）
* A27b whitelist 收尾：随 DWG mirror bundle 落地，逐条 (tag,
  interface) 关闭 12 条 loader-side 富化列差异
* A29c loader-side `PublishStyle` 自动决策：根据 SQLite
  metadata（plant 名 / SmartPlant project 配置）自动设
  `drawing.style`；当前 caller / CLI 必须显式指定 (--style)。
  需要 DWG SQLite mirror 落地后才能 reverse-engineer 出可靠
  detection 启发式。

## [0.9.2] - 2026-04-21

### Phase 3: Provenance-aware cross-reference (Step 1–4)

把 `CrossReferenceGraph` 从"名字集合 + 小摘要"升级为"字段级 provenance
记录 + 端到端诊断"，一次覆盖四条正交视角（cluster/symbol/attribute、
relationship、object、sheet），并且为 Phase 12a 的统一规范化层做好数据
基础。所有变更纯 additive + `#[serde(default)]`，旧 consumer 字节级
兼容。

### Added — Step 1：cluster / symbol / attribute provenance records

- `ClusterCoverage.declared_entries: Vec<DeclaredClusterRef>` — 带
  `record_offset` / `name_offset` / `record_len` 的 PSM 表行级溯源
- `ClusterCoverage.found_entries: Vec<FoundClusterRef>` — 带 `source_kind`
  (`PsmCluster` / `SheetStream`) 与 `path` 的发现源
- `ClusterCoverage.matches_detailed: Vec<ClusterCoverageMatch>` —
  declared ↔ found 的 index 级映射
- `SymbolUsage.references: Vec<SymbolReference>` — 每个 JSite 的 `path` /
  `local_symbol_path` / `has_ole_stream`，取代旧 BTreeSet 去重
- `AttributeClassSummary.records: Vec<AttributeClassRecordRef>` — 每条
  DA 记录带 `attribute_count` / `confidence` / 局部 drawing/model ids

### Added — Step 2：relationship + object provenance links

- `RelationshipEndpointLink` — 每条 `PidRelationship` 连回
  `SheetEndpointRecord` 的 `sheet_path` / `sheet_offset` / source/target
  `field_x`；`missing_sheet_record` 区分"lookup 失败"与"没 field_x"
- `EndpointLinkCoverage` — `total` / `linked` / `missing_field_x` /
  `missing_sheet_record` / `fully_resolved` / `partially_resolved`
- `ObjectSourceRef` — 每条 `PidObject.drawing_id` 连回
  `DynamicAttributesBlob.attribute_records` 索引（`class_name`、
  `attribute_record_index`、`confidence`、`has_trailer_record_id`、
  `missing_da_record`）
- `ObjectSourceCoverage` — `total_objects` / `linked` / `missing_da_record` /
  `with_trailer_record_id`

### Added — Step 3：end-to-end provenance chain diagnostic

- `ProvenanceChainCoverage` — 逐跳通过数：`has_field_x` / `sheet_linked` /
  `source_object_linked` / `target_object_linked` / `fully_traced`
- `ProvenanceChainStage` 枚举 — `MissingFieldX` / `MissingSheetRecord` /
  `SourceObjectUnlinked` / `TargetObjectUnlinked`
- `ProvenanceChainBreak` + `PROVENANCE_CHAIN_BREAK_SAMPLE_CAP = 10`，
  first-fail 规则：一条链在最前断裂处记一次，不污染后续 hop

### Added — Step 4：sheet-level provenance aggregation

- `SheetProvenanceRef` — per-`SheetStream` 聚合：`endpoint_record_count` /
  `declared_in_psm` / `matched_declared_index` / `linked_relationship_count` /
  `fully_traced_relationship_count`，1:1 对应 `doc.sheet_streams`
- `SheetProvenanceCoverage` — `total_sheets` / `declared_sheets` /
  `orphan_sheets` / `sheets_with_endpoint_records` / `empty_declared_sheets`

### Added — inspect report

`Cross Reference` section 新增 5 个小节，均默认取前 5 条样例、超额用
`... (N more)` 提示：

- `Cluster refs:` — declared/found/match 样例
- `Symbol refs:` / `Attribute class refs:` — Step 1 extensions
- `Relationship endpoints:` — Step 2 relationship link coverage + 样例
- `Object sources:` — Step 2 object link coverage + 样例
- `Sheet provenance:` — Step 4 per-sheet 聚合 + 样例
- `Provenance chain:` — Step 3 诊断 + 前 5 条 breaks（超额提示 more）

### Tests — 332 → 400（净增 68）

- Step 1 新增 3 条单测 + 3 条 fixture 回归
- Step 2 新增 8 条单测 + 2 条 fixture 回归
- Step 3 新增 4 条单测 + 1 条 fixture 回归
- Step 4 新增 2 条单测 + 1 条 fixture 回归
- 其余增量来自既有测试对新字段的扩断言

### Changed

- `crossref::build_graph` 内部改为两遍填装：先构造前四子图，再基于已
  构造图派生 provenance chain 与 sheet aggregation；外部 API / 输出
  顺序不变
- 多处 test site（`mermaid`、`layout`、`import_view`、`mermaid_demo`）
  使用 `..Default::default()` 吸收新字段，零行为改动

### SemVer

Patch（additive + `#[serde(default)]`；旧 JSON 输入字节级兼容，调用旧
字段语义未变）。

## [0.9.1] - 2026-04-21

### Phase 2: 三条核心流的结构化解码升级

把 `DocVersion3`、`PSMclustertable`、`PSMsegmenttable` 从"可用但部分
语义化"升级为具备 record 级结构元数据和字节审计能力的解码层。

### Added — 新 model 字段

- `VersionHistory.record_size: usize` — 固定记录大小（48），方便下游对账
- `VersionHistory.trailing_bytes: usize` — 末尾未解释字节数
- `VersionRecord.offset: usize` — 记录在流内的字节偏移
- `PsmClusterEntry.record_offset: usize` — 记录起始偏移（含前缀）
- `PsmClusterEntry.record_len: usize` — 记录总字节长度
- `PsmClusterEntry.prefix_bytes: Vec<u8>` — 记录名称前的原始字节（审计用）
- `PsmClusterTable.trailing_bytes: usize` — 末尾未解释字节数
- `PsmSegmentEntry` 新类型 — 每条 segment 的 `index`、`offset`、`flag`
- `PsmSegmentTable.entries: Vec<PsmSegmentEntry>` — 结构化视图
- `PsmSegmentTable.trailing_bytes: usize` — 末尾未解释字节数

所有新字段均 `#[serde(default)]`，与 v0.9.0 JSON 输入向后兼容。

### Changed

- **DocVersion3 parser**：增加 `product.trim().is_empty()` 校验，拒绝空白
  product 记录（更强的边界保护）；填充 `offset` / `record_size` /
  `trailing_bytes`
- **PSMclustertable parser**：从"UTF-16LE ASCII 名称扫描"升级为"记录遍历"，
  以名称为锚点向前回溯截出完整记录 slice，记录 `record_offset` /
  `record_len` / `prefix_bytes`；识别尾部 null 终止符
- **PSMsegmenttable parser**：在保留 `flags: Vec<u8>` 的基础上，同步生成
  `entries: Vec<PsmSegmentEntry>`，每条 entry 带 `index` + `offset`
- **inspect report**：
  - Version History 段标题显示 `record_size=`
  - 每条记录前缀 `[@+offset]`
  - 显示 trailing bytes（当 > 0）
  - PSMclustertable 每项显示 `rec_len=` / `name@offset` / `prefix=[hex]`
  - PSMsegmenttable 切换到 per-entry 显示 `[index] @+offset flag=0x..`

### Tests (359 → 366+，净增 7+)

- `doc_version3_records_expose_record_offset_and_trailing_bytes` — 3 条记录 + 4 字节尾随
- `doc_version3_rejects_record_with_empty_product` — 空白 product 停止解析
- `doc_version3_zero_trailing_when_exact_fit` — 精确对齐零残余
- `cluster_table_entry_records_offsets_and_prefix_bytes` — record 级结构化
- `cluster_table_reports_trailing_bytes` — 尾部字节审计
- `segment_table_exposes_indexed_entries` — entry 化验证
- `segment_table_reports_trailing_bytes` — 尾部字节审计

### SemVer

Patch（新字段 additive + 行为加强；旧 consumer 无感）。

## [0.9.0] - 2026-04-21

### Phase 10j (MVP): DocumentSummaryInformation section 2 reader

暴露 `/\x05DocumentSummaryInformation` stream 的 **section 2**
（user-defined property dictionary，FMTID
`D5CDD505-2E9C-101B-9397-08002B2CF9AE`）到 Rust 模型里。此前
reader 只读 section 0；section 2 的 SmartPlant 自定义属性
（`SP_ProjectID` / `SP_DrawingRevision` / `SP_Discipline` 等）对
consumer 不可见，只能手工解析 raw stream。

本轮是 Phase 10j 的 **MVP (reader-only scope)**：
- ✅ Reader 端 section 2 解码完整
- ✅ 新 `SummaryPropertyValue` typed enum 接入模型
- ✅ 配套合成 fixture + 5 轮单测覆盖
- ⏳ **Writer CRUD defer 到 Phase 10m**（需先扩展
  `writer::summary_write::parse_section` 支持 Dictionary 特殊
  record + 未知 VT 的 verbatim 保留，scope 超出本 Phase）
- ⏳ CLI `--set-user-summary` / `--delete-user-summary` 同步 defer

### Added — 新 public 类型

- `model::SummaryPropertyValue` enum：typed 表示 user dict 里一个
  property 的值。覆盖 SmartPlant 实测最常见的 VT：
  - `Lpstr(String)` — VT_LPSTR (0x001E)
  - `Lpwstr(String)` — VT_LPWSTR (0x001F)
  - `I4(i32)` — VT_I4 (0x0003)
  - `Bool(bool)` — VT_BOOL (0x000B)，wire format 0x0000/0xFFFF
  - `Filetime(u64)` — VT_FILETIME (0x0040)
  - `Raw { vt: u16, bytes: Vec<u8> }` — 未模型化 VT 的保底，writer
    层未来做 verbatim 透传时能用上
- `SummaryInfo.user_properties: BTreeMap<String, SummaryPropertyValue>`
  新字段，`#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`。
  v0.7.x / v0.8.x 的 JSON 输入保持向后兼容。

### Added — Reader 侧 section 2 decoder

`src/streams/summary.rs` 新增 `parse_doc_summary_section_2`：

- 遍历 PropertySetStream header 验证 `num_sections >= 2`
- 验证 section 2 FMTID 是 user-defined dict FMTID
- 解析 Dictionary property（PROPID 0）的 LPSTR 条目表（SmartPlant
  默认 variant），建立 `propid → name` 映射
- 遍历剩余 props，按 VT 解码到 `SummaryPropertyValue`
- 未知 VT 落 `Raw` 分支（grab 最多 64B 负载，避免吞掉整流）
- 所有 defensive 边界检查失败都 silent 返回空 map（reader 不 fail
  整个 pipeline）

Phase 10j MVP 假设 Dictionary 是 LPSTR variant（SmartPlant 实测
统一如此）。Dictionary 是 LPWSTR variant 的 code page 处理延后到
Phase 10k（与 reader-side code page detection 一起做）。

### Changed

- `streams::summary::parse_summary_streams` 在解 section 0 之后
  调 `parse_doc_summary_section_2(&data)`，把结果挂到
  `info.user_properties`。section 2 缺失 / 格式错误 / FMTID 错都是
  silent no-op（field 保持空 map）。
- `Cargo.toml` 0.8.0 → 0.9.0；`Cargo.lock` 同步。
- `inspect::coverage::tests::populate_all_known_fields` 的
  `SummaryInfo` struct literal 补 `user_properties: BTreeMap::new()`。

### Tests (354 → 359，净增 5)

lib `streams::summary::section2_tests` +5：

- `section_2_decodes_user_defined_lpwstr_property` —— 合成
  fixture：2 个 section，section 2 里 1 条 `SP_ProjectID =
  "PROJ-001"`，验证 map 出的 key / value 匹配
- `single_section_stream_yields_empty_user_properties` —— 同
  fixture 但手工 patch `num_sections = 1`，section 2 不应被扫描
- `section_2_with_wrong_fmtid_is_ignored` —— 故意 corrupt section 2
  FMTID 首字节，decoder 返回空 map
- `unknown_vt_in_section_2_falls_through_to_raw_variant` —— 塞 VT_UI8
  (0x0015) 这个 decoder 未显式处理的 VT，验证 `Raw { vt, bytes }`
  分支命中且 `bytes.len() >= 8`
- `malformed_header_yields_empty_map` —— 10B 垃圾输入不 panic

### Known limitations

- **Writer CRUD 不可用**：`MetadataUpdates.summary_user_updates /
  summary_user_deletions` 本次不加。尝试在 Writer 层编辑 section 2
  属性当前只能走 `stream_replacements`（full-stream byte replace，
  不保结构）。完整 dictionary + Writer CRUD scope 跟 Phase 10m /
  10n 一起规划。
- **LPSTR Dictionary 假设**：SmartPlant fixture 实测 Dictionary
  为 LPSTR variant。若出现 LPWSTR Dictionary，当前 decoder 空
  返回 — 留给 Phase 10k 扩展（LPWSTR dict 读取 + CodePage property
  尊重）。
- **未识别 VT 的 `Raw.bytes.len()` 上限 64B**：避免流尾异常吞过度
  字节；真实值长度超过 64B 的未识别 VT 不会完整保留。Phase 10m 重
  新 design 时会精确算 size（或从下一 prop 的 offset 反推）。

### SemVer

新 public type `SummaryPropertyValue` + 新 `SummaryInfo` 字段
（向后兼容 via `#[serde(default)]`）+ 无 API 破坏。按本项目规则
"新 public API type = minor bump"，选 **minor 0.8 → 0.9.0**。0.8.x
consumer 的代码在 0.9.0 下编译通过（唯一变化是 `SummaryInfo` 构造
字面量需要补一个 `..Default::default()` 或显式写 `user_properties:
BTreeMap::new()`；同理于 Phase 9l/9n 新增字段时）。

### Docs

- `docs/plans/2026-04-21-phase-10j-docsummary-section2.md`：
  本 Phase dev plan（v0.7.1 ship 时已入 repo）。本 MVP ship 的
  scope 较 plan 初稿缩小到 Reader-only，Writer CRUD 显式 defer；
  Plan 里对 Writer 的设计仍然有效，只是落地推到 Phase 10m。

## [0.8.0] - 2026-04-21

### Phase 10i: VT_LPSTR 多 code page 支持（解除 Phase 10g UTF-8-only 限制）

Phase 10g (v0.7.0) 把 `VT_LPSTR` 从 ASCII-only 放宽到 UTF-8，但明确
保留 "CP1252 / GBK / Shift-JIS 等传统 code page 不自动识别/转换" 的
限制。Phase 10i 兑现这条 roadmap：引入**显式编码**的 `VT_LPSTR`
writer 通道，让用户可以按需写入 SmartPlant 2019- 旧版本使用的
Western/Chinese/Japanese code page。

本 Phase 聚焦 **Writer 端**；Reader 端自动 code-page 启发式探测
（`detect_lpstr_codepage`）留给 **Phase 10k** — 当前 Phase 用户必须
显式指定 `encoding` label，reader 仍按 Phase 10g 的 UTF-8 策略解析
（对 ASCII-only writes 端到端 round-trip 完全工作，非 ASCII writes
的 reader 侧 round-trip 由 writer 层字节级单测验证）。

### Added — `EncodedString` + `summary_updates_encoded` 通道

- `src/writer/plan.rs` 新 `EncodedString { value: String, encoding: String }`
  类型 + `EncodedString::new(value, encoding)` 构造器。`encoding` 字段
  接受任意 `encoding_rs` label（`"UTF-8"` / `"windows-1252"` / `"GBK"` /
  `"Shift_JIS"` / `"cp1252"` / ...），case-insensitive。
- `MetadataUpdates.summary_updates_encoded: BTreeMap<String, EncodedString>`
  新字段，`#[serde(default)]`。与既有 `summary_updates` 平行共存：
  - `summary_updates`（Phase 9l/10g）= UTF-8 默认通道
  - `summary_updates_encoded`（Phase 10i）= 显式 code-page 通道
  - 同一 key 同时出现在两个 map 中 → pre-check 报错（避免编码歧义）
  - 同一 key 同时在 `summary_updates_encoded` 和 `summary_deletions`
    中 → 同样报错（对齐 Phase 9n 的 set/delete 冲突拒绝）
- `pid_parse::writer::summary_write::apply_summary_updates_encoded(
  package, updates)` 公共入口，供 binary / 高级 consumer 直调（等价
  于 apply-plan 路径的 `summary_updates_encoded` 分支）。
- `encode_string_with_encoding(vt, value, encoding, prop_id)` 内部函数：
  - `VT_LPSTR`: 走 `encoding.encode(value)`，`had_errors == true` 时
    **fail-fast** 返回清晰错误（包含 encoding 名 / offending value），
    绝不 silent 替换为 `?`
  - `VT_LPWSTR`: 忽略 `encoding` 参数（UTF-16LE 无歧义）
- `SummaryPropertySet::set_string_with_encoding(prop_id, value, encoding)`
  内部方法；旧 `set_string` 重写为 UTF-8 thin wrapper。
- `resolve_encoding(label) -> Result<&'static Encoding, PidError>`：
  统一 label → `encoding_rs::Encoding` 解析，未知 label 返回
  `PidError::ParseFailure { context: "summary writer", ... }`。
- `lib.rs` re-export `EncodedString` 到 `pid_parse` crate root。
- `writer/mod.rs` re-export `EncodedString`。

### Added — CLI `--set-summary-encoded KEY:ENCODING=VALUE`

- `pid_writer_validate --set-summary-encoded title:windows-1252=Ø Pipe`
  类语法：
  - 第一个 `:` 分 KEY 和 ENCODING
  - 第一个 `=` 分 ENCODING 和 VALUE
  - VALUE 可含 `=` / `:`（空 KEY / 空 ENCODING 都是 usage error）
- 互斥：与 `--set-summary` 同 key 冲突 → exit 2；与 `--apply-plan`
  同时传 → exit 1。与 `--delete-summary` 在**不同** key 上可并存。
- `CliOptions.summary_edits_encoded: BTreeMap<String, EncodedString>`
  新字段；`run_validate` 签名新增参数（lib-level public API）。
- usage 文案更新：`print_usage` 新增 Phase 10i 段说明。

### Changed

- `MetadataUpdates` 默认构造器包含 `summary_updates_encoded:
  BTreeMap::new()`。
- `WritePlan::is_passthrough` 检查扩展到
  `summary_updates_encoded.is_empty()`。
- `apply_metadata_updates` 执行顺序扩展：`drawing_xml → general_xml
  → summary_deletions → summary_updates → summary_updates_encoded`。
- `compare_packages` / `collect_edited_paths_from_plan` 把 summary
  路径标 "edited" 的触发条件从 `!summary_edits.is_empty() ||
  !summary_deletions.is_empty()` 扩展到 + `!summary_edits_encoded
  .is_empty()`。

### Tests (332 → 354，净增 22；测试分布：lib 238→255 = +17，
writer_validate_cli 21→26 = +5，其余不变)

lib 单元 +17：

`src/writer/plan.rs::tests`（+4）：
- `encoded_string_serializes_to_object_with_value_and_encoding`
- `encoded_string_round_trips_through_json`
- `plan_with_summary_updates_encoded_is_not_passthrough`
- `plan_omitting_summary_updates_encoded_is_passthrough_compatible`
  —— **向后兼容 guard**：v0.7.x consumer 写的 JSON 不带此字段仍然
  valid 且 passthrough

`src/writer/summary_write.rs::tests`（+11）：
- `encode_lpstr_with_cp1252_preserves_western_european_bytes`
  —— `Ø`（U+00D8）在 CP1252 是 0xD8 单字节，验证不走 UTF-8 的 2 字节
- `encode_lpstr_with_cp1252_rejects_chinese_characters_lossy`
  —— lossy 输入 fail-fast，错误含 `cannot encode` / 编码名 / 原值
- `encode_lpstr_with_gbk_preserves_simplified_chinese_bytes`
  —— "公司" 在 GBK 是 4 字节（每字符 2 字节），GBK round-trip 无损
- `encode_lpwstr_ignores_encoding_hint`
  —— VT_LPWSTR 路径对 `encoding` 参数不敏感，UTF-16LE 始终
- `resolve_encoding_accepts_standard_and_alias_labels`
  —— `windows-1252` / `cp1252` / `GBK` / `gbk` / `UTF-8` 均接受
- `resolve_encoding_rejects_unknown_labels`
  —— `Klingon-1` → `unknown encoding label`
- `apply_summary_updates_encoded_end_to_end_rewrites_lpstr_with_explicit_codepage`
  —— 完整 plan 链路，最终 stream 字节含 0xD8 + VT_LPSTR 保留
- `apply_summary_updates_encoded_rejects_lossy_input_clearly`
  —— fail-fast **不 mutate 源字节** 的防御性测试
- `apply_summary_updates_encoded_unknown_key_errors_without_side_effect`
- `apply_summary_updates_encoded_unknown_encoding_errors_cleanly`
- `apply_summary_updates_encoded_empty_is_zero_cost_noop`

`src/writer/metadata_write.rs::tests`（+2）：
- `summary_updates_and_updates_encoded_on_same_key_return_error`
- `summary_deletions_and_updates_encoded_on_same_key_return_error`

CLI 集成 +5 (`tests/writer_validate_cli.rs`)：
- `validate_set_summary_encoded_ascii_round_trips_with_explicit_codepage`
  —— ASCII happy path 走完整 CLI → fixture → writer → reader 链路
- `validate_set_summary_encoded_rejects_lossy_cp1252_input`
  —— CP1252 + 中文 exit 2，stderr 含 `cannot encode` + `windows-1252`
- `validate_set_summary_encoded_rejects_unknown_encoding_label`
  —— `Klingon-1` exit 2，stderr 含 `unknown encoding label`
- `validate_set_summary_encoded_conflicts_with_set_summary_on_same_key`
  —— exit 2 + "both target key" guard message
- `validate_set_summary_encoded_usage_error_on_missing_colon`
  —— syntax guard exit 1，stderr 含 `missing :`

### Verification

- `cargo check --all-targets` → ok (pid-parse v0.8.0)
- `cargo test --all-targets` → **354 passed** / 0 failed
  - lib: 238 → 255 (+17)
  - writer_validate_cli: 21 → 26 (+5)
  - 其余模块不变（inspect_cli 4 / parse_real_files 28 / unit_parsers 18
    / writer_real_files 10 / writer_roundtrip 13）
- `cargo fmt/clippy`: 本地 toolchain 缺组件（rustup LICENSE 冲突 bug），
  跳过；代码对齐 v0.7.1 已验证的风格

### Known limitations (tracked for future phases)

- **Reader 端自动 code-page 探测**：当前仅用户显式指定；非 ASCII
  VT_LPSTR 的 reader round-trip 需等 Phase 10k `detect_lpstr_codepage`
  完成（启发式：fixture 白名单 → BOM → 字节频次 → UTF-8 fallback）。
  端到端非 ASCII round-trip 测试届时补齐。
- **多样本 fixture**：本 Phase 验证仍基于单合成 fixture；真实
  SmartPlant CP1252 / GBK fixture 的 writer 回归测试需 fixture 收集
  pass（Phase 11 窗口）。
- `VT_LPWSTR` 忽略 encoding 字段的语义在当前 API 里是 "silent ignore"；
  若 future consumer 需要显式 "encoding for lpwstr" 错误，留给
  Phase 10k 的设计决议。

### SemVer 判定

- 新字段 `MetadataUpdates.summary_updates_encoded` + `#[serde(default)]`：
  JSON 向后兼容（旧 `{}` 仍 valid passthrough）。
- 新类型 `EncodedString` + 新 pub API (`apply_summary_updates_encoded` /
  `encode_string_with_encoding` via `SummaryPropertySet::set_string_with_encoding`)：
  additive。
- 新 CLI flag + 新 `run_validate` 参数：API surface 扩展。
- 旧 Rust `MetadataUpdates::default()` 构造继续 work；Rust consumer
  代码零破坏。

综合：**minor bump 0.7 → 0.8.0**（行为放宽 + 新 public API + 新 CLI
flag），沿用 Phase 10g 的 "错误路径变成功路径 = minor bump" 判定规则。

### Docs

- `docs/plans/2026-04-21-phase-10i-vtlpstr-codepage.md`：本 Phase dev plan
  （v0.7.1 ship 时已入 repo）。
- `docs/sppid/v0.7.x-status.md` 将在 Phase 10k ship 时统一刷新到
  `v0.8.x-status.md`（目前内容仍描述 10g 限制，不阻塞 10i ship）。

## [0.7.1] - 2026-04-21

### Phase 10h: session 成果归档 + 下一步 roadmap 落地（docs-only，无行为变化）

本 session（2026-04-21）连续 ship 了 v0.4.2 → v0.7.0 共 12 轮
（Phase 9k → 10g），跨两个大周期（Writer 建设 + SPPID full-parse
coverage 系列）。按 Phase 9d 方法论"连续新功能后强制插入卫生 pass"，
本轮不加 feature，只做归档 + 下一步 roadmap 规划。零代码改动，
仅文档落盘。

### Added — 归档文档（v0.4.2 → v0.7.0 的 session 封存）

- `docs/phase10-coverage-series-summary.md`：对齐
  `phase8-9h-summary.md` 风格，归档 Phase 9k-9o + 10a-10g 共
  12 个 commit 里程碑。含起点/终点对照表（test count 260→332、
  Phase 数 12、新增 CLI flag 4、CHANGELOG 行数 ~100→~550）、阶段
  轨迹（每 Phase 动机/产出/方法论点评）、方法论沉淀 5 条
  （feature+卫生 pass 节奏、coverage 是 parser 对偶面、SemVer
  minor/patch 判定、doc-first plan 文化、交叉验证锚点复用）、
  版本↔Phase↔commit 索引表。
- `docs/sppid/v0.7.x-status.md`：SPPID 解析能力一页纸快照，含
  15 个顶层流/存储的 coverage 状态表、Writer 层 6 项能力矩阵、
  保真度维度表、roadmap Phase 1-5 完成度、CLI 入口索引、332 测试
  分布统计、Consumer quick-start 3 个代码样例（parse / edit via
  plan / inspect coverage programmatically）。

### Added — 下一步 roadmap plan 文件（8 份，覆盖 v0.7.1 → 验收全链路）

对齐既有 `phase-9l/9m/9n/10b-10h` 的 dev plan 模板（动机 / 非目标
/ 范围 / 具体设计 / W1-W7 实施步骤 / 预计工时 / 验证清单 / 风险
缓解 / SemVer 判定 / Next 候选 / 交叉引用）。

- `docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md`
  （16.3KB）— 总导航：TL;DR + 5 阶段（A-E）优先级矩阵 + 现状详
  细快照（coverage/writer/tests）+ 起步节奏建议 + 风险登记。
- `docs/plans/2026-04-21-phase-10h-session-archive.md`（本 Phase
  的 dev plan，3.3KB）。
- `docs/plans/2026-04-21-phase-10i-vtlpstr-codepage.md`（10.9KB）—
  VT_LPSTR 多 code page 回退（CP1252/GBK/Shift-JIS），`SummaryValue`
  枚举向后兼容设计，~3-5hr，minor bump 0.8.0。
- `docs/plans/2026-04-21-phase-10j-docsummary-section2.md`（11.9KB）—
  DocumentSummaryInformation section 2（user-defined dict）CRUD
  编辑，`UserDefinedDictionary` + `PropertyValue` 类型设计，
  ~4-6hr，minor bump 0.9.0。
- `docs/plans/2026-04-21-phase-11a-psmclustertable-records.md`
  （14.9KB）— PSMclustertable per-record 结构化（roadmap Phase 2.2），
  含 hex walk 策略 + type_tag↔ClusterKind 映射 + segment count
  对账，~6-8hr，minor bump 0.10.0。
- `docs/plans/2026-04-21-phase-11b-psmsegmenttable.md`（12.6KB）—
  PSMsegmenttable 结构化（roadmap Phase 2.3），`SegmentKind` enum
  + owner_cluster 反推 + Sheet endpoint 三向对账，~4-6hr，minor
  bump 0.11.0。
- `docs/plans/2026-04-21-phase-11c-sheet-geometry.md`（13.4KB）—
  Sheet 深层几何/图元解码（roadmap Phase 2.5），分 4 sub-phase
  （频次探针 / object+label / line+reference / layout 融合），
  ~10-14hr，minor 0.12.0 + patches；明确标注"逆向不确定性最高，
  强烈建议单独 session 设计"。
- `docs/plans/2026-04-21-phase-12a-normalization-layer.md`（8.7KB）
  — 规范化语义图层（roadmap Phase 3，大 Phase 骨架），含
  `NormalizedObject/Relationship/Endpoint/SymbolRef/ClusterRef` +
  `Provenance { source_layer: Raw/Decoded/Inferred }` 类型设计，
  ~20-30hr，先发 RFC 再实施。
- `docs/plans/2026-04-21-phase-12b-byte-audit-framework.md`
  （11.4KB）— consumed/leftover 字节验证框架（roadmap Phase 4），
  `ParserTrace` + `ByteRange` + `ParserTraceBuilder` 侧信道设计，
  `pid_inspect --byte-audit` CLI + CI regression baseline，
  ~12-18hr，minor 0.14.0 + patches。

### Changed

- `Cargo.toml` version 0.7.0 → 0.7.1。
- `Cargo.lock` 同步 bump。

### Verification

- `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`
  → 双 0
- `cargo test --all-targets` → **332 passed** / 0 failed（无增减；
  本版本零代码改动）

### Session 闭环指标

- **本 session 总产出**：12 个 version ship（v0.4.2 → v0.7.0）+
  本轮 v0.7.1 归档 + v0.7.1→验收的 8 份 plan 文件 ≈ 100KB 规划文档
- **覆盖时间跨度**：从 Writer 建设收官到 SPPID full-parse 完成
  的 60-85hr 全路径
- **测试**：332 tests 全绿，clippy/fmt 双零
- **下一步明确入口**：Phase 10i (VT_LPSTR code page) 或 Phase 11a
  (PSMclustertable per-record) 任选其一

### Docs

- `docs/plans/2026-04-21-phase-10h-session-archive.md`：本 Phase
  起稿 plan。

## [0.7.0] - 2026-04-21

### Phase 10g: VT_LPSTR UTF-8 编码（解除 Phase 9l ASCII-only 限制）

Phase 9l (v0.5.0) 为 `SummaryInformation` property-set writer 设了一
条 ASCII-only 门：非 ASCII 字符写入 `VT_LPSTR` 属性会被 reject。
当时明确说这条限制"tracked for Phase 9m UTF-8 support"，Phase 9m/9n
都没动到它。Phase 10g 兑现该 roadmap：`VT_LPSTR` 字段现在按 UTF-8
编码字节（[MS-OLEPS] §2.11 允许单字节字符串采用操作系统 code page；
我们选 UTF-8 因为现代 SmartPlant 实测使用 UTF-8）。

### Changed (behavior; minor bump)

- `writer::summary_write::encode_string` 的 `VT_LPSTR` 分支去掉
  `value.is_ascii()` 门；现在任何 UTF-8 字符串都被接受。char_count
  字段照例存储 **byte count**（含 NUL 终止符），与 reader 侧保持
  一致。
- 对既有 0.5.x/0.6.x consumer 的影响：
  - 之前被 reject 返回
    `PidError::ParseFailure { context: "summary writer", message:
    "...value contains non-ASCII bytes..." }` 的调用路径现在会**成功**。
  - 对所有 ASCII-only 输入行为完全不变（UTF-8 是 ASCII 的超集）。
  - 因为语义放宽（原错变正确），按 SemVer 走 **minor bump**（0.6.5
    → 0.7.0）而非 patch。
- `writer::plan::MetadataUpdates.summary_updates` 的 doc 去掉 "VT_LPSTR
  non-ASCII rejected" 提示。

### Tests (332 → 332，行为替换而非新增)

`writer::summary_write::tests::encode_lpstr_rejects_non_ascii` 重命名并
重写为 `encode_lpstr_accepts_utf8_non_ascii`（仍是单个测试函数，test
count 不变）。新 test 验证：

1. `encode_string(VT_LPSTR, "中文 title", 2)` 成功，char_count ==
   UTF-8 byte length + 1。
2. bytes body 在 `from_utf8` 下仍是原字符串。
3. 端到端 round-trip：构造 LPSTR fixture → `apply_summary_updates(
   {"title": "公司 Co. 中文"})` → parse 回来 `title_prop.vt ==
   VT_LPSTR`、bytes 解为原字符串。

### Known limitations

- 仅 UTF-8。如 fixture 下游 consumer 需要 CP1252 / GBK / Shift-JIS
  等传统 code page，本版本不自动识别/转换。方案：
  - 要求下游把 fixture 迁到 UTF-8（SmartPlant 2020+ 默认已是）
  - 在 `summary_updates` 外部自行转码，传入已经是目标 code page 的
    `String`（UTF-8 表示法，字节一致）
  - 切到 `VT_LPWSTR` 对应的 prop（目前 `summary_updates` 会保留源
    VT；若要强制 LPWSTR 可先用 `summary_deletions` 删掉 LPSTR 条目，
    新 add 会默认 LPWSTR）
- DocumentSummaryInformation user-defined section 2 仍未支持（Phase
  9n 已做 issue，Phase 10i 排队）。

### Docs

- `docs/plans/2026-04-21-phase-10g-lpstr-utf8.md`。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **332 passed** / 0 failed
  （lib 238 + inspect_cli 4 + parse_real_files 28 + unit_parsers 18
   + writer_real_files 10 + writer_roundtrip 13 + writer_validate_cli
   21 = 332；Phase 10g 的变化是 test body 替换，count 未净增）

## [0.6.5] - 2026-04-21

### Phase 10f: coverage 加 bytes 维度

`CoverageEntry` 获得 `stream_size: Option<u64>` 字段，让 coverage
报告能回答"哪些流占了多少字节"，为 roadmap Phase 4 "暴露未解释
字节集中区" 铺第一块基础。本轮仍是纯声明式（来源是 `StreamEntry.size`，
没有 parser 内部的 consumed bytes tracking —— 那是 Phase 10g/10h
的范围）。

### Added

- `model::CoverageEntry.stream_size: Option<u64>`（`#[serde(skip_serializing_if = "Option::is_none")]`）。
- `CoverageReport::total_bytes_by_status() -> [u64; 4]`：status-bucket
  byte 总和（`saturating_add`，unknown=None 的 entry 贡献 0）。
- `inspect::coverage::size_for_top_level(doc, name)`：aggregating
  probe — 顶层 stream 取自己，storage 取 children 总和，无匹配返回
  `None`。

### Changed

- `inspect::report::generate_report` 的 `--- Coverage ---` 段每条
  bucket 摘要追加字节总和（`"Fully decoded:     3 (4.5 KB)"`），每
  条 entry 行追加字节 `(24 B)` / `(1.2 KB)` 等后缀。单位按 1024 进
  制转 B/KB/MB/GB，保留 1 位小数（`< 1 KB` 时用纯整数 `"42 B"` 形
  式避开小数点噪音）。
- `inspect::coverage::top_level_coverage_entries` 构造每个
  `CoverageEntry` 后都会填充 `stream_size`，确保 JSON / 文本两种
  输出都能访问到大小信息。

### Tests (329 → 332)

lib 单元 +3：

- `coverage_entry_carries_stream_size_for_single_top_level_stream`
  —— 单流大小直接 attachment。
- `coverage_entry_aggregates_sizes_across_storage_children`
  —— 存储前缀（`Sheet1`）正确 aggregate 3 个 children 的大小。
- `coverage_report_total_bytes_by_status_matches_entries`
  —— Full=128, Partial=220, Ident=1000, Unknown=18 的混合样本验证
  `total_bytes_by_status`。

既有测试更新：

- `report_includes_coverage_section_with_bucket_counts_and_per_entry_tags`
  新断言 bucket 行含 `"(42 B)"` 后缀 + entry 行含 `(42 B)` 子串。
- `coverage_json_flag_emits_parseable_coverage_report` 断言 JSON
  entries 里至少一条携带 `stream_size` 数值字段。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **332 passed** / 0 failed

### Docs

- `docs/plans/2026-04-21-phase-10f-coverage-bytes.md`。

## [0.6.4] - 2026-04-21

### Phase 10e: coverage JSON 导出

为 `CoverageReport` 补齐与 `WritePlan`（Phase 9o）对称的 JSON
round-trip helpers + CLI `--coverage --json` 组合输出。让 CI 脚本 /
外部 dashboard / 未来 Phase 10f 字节级验证框架可以直接消费结构化
coverage 数据，不用再 grep 人类文本。

### Added

- `CoverageReport::to_json(&self) -> Result<String, PidError>`
- `CoverageReport::to_json_pretty(&self) -> Result<String, PidError>`
- `CoverageReport::from_json(&str) -> Result<Self, PidError>`
- `pid_inspect --coverage --json`：专门输出 `CoverageReport` 的
  pretty JSON（与 `--json` 单独使用时的 full-document dump 分开）。

错误统一包装为 `PidError::ParseFailure { context: "coverage report
JSON", ... }`，caller 无需 pull serde_json::Error。

### Changed

- `pid_inspect` 的 `--json` 分支现在 short-circuit 到 coverage-only
  JSON 当且仅当同时传了 `--coverage`。其他组合（`--json` 单独 /
  `--coverage` 单独）行为不变。

### Tests (324 → 329)

lib unit (+4)：
- `coverage_report_json_round_trip_default`
- `coverage_report_json_round_trip_preserves_entries`（通过实际
  classifier 构造 4 个 bucket 混合 report，pretty → parse → assert
  bucket counts 字节对齐）
- `coverage_report_from_json_rejects_invalid_syntax_with_pid_error`
- `coverage_report_to_json_pretty_is_multiline_and_indented`

CLI 集成 (+1)：
- `coverage_json_flag_emits_parseable_coverage_report`：CLI 跑
  `--coverage --json`，用 `serde_json::from_str` 解 stdout，断言
  `entries` 数组非空、出现 `FullyDecoded` + `Unknown` 状态、且 JSON
  不含 `streams` 字段（分支污染 regression guard）。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **329 passed** / 0 failed

### Docs

- `docs/plans/2026-04-21-phase-10e-coverage-json.md`

## [0.6.3] - 2026-04-21

### Phase 10d: DocVersion3 operation 语义化 + report 渲染升级

`VersionRecord.operation` 长期保留 raw 2-char 形式（`"SA"` / `"SV"`），
与 Phase 9f 逆向完成的 DocVersion2 `op_type_label(0x82) = "SaveAs"`
不对称。本轮补齐 helper 方法 + 报告层渲染升级，**保持 serde / JSON
schema 向后兼容**（字段类型不变）。

### Added

- `VersionRecord::is_save_as() -> bool` — `operation == "SA"`。
- `VersionRecord::is_save() -> bool` — `operation == "SV"`。
- `VersionRecord::is_recognized_operation() -> bool` — 已知 op code
  命中检查。
- `VersionRecord::operation_label() -> &'static str` — 人类标签
  `"SaveAs"` / `"Save"` / `"unknown"`，对齐 DV2 `op_type_label`。
- `VersionRecord::parsed_timestamp() -> Option<(u32,u32,u32,u32,u32)>`
  — `MM/DD/YY HH:MM` → `(month, day, year, hour, minute)` 分解；
  非法格式 / 越界返回 `None`。两位年份不做世纪推断，交由 caller。

### Changed

- `inspect::report::generate_report` 的 `--- Version History ---` 段
  现在输出人类标签 + raw code（若非已知）：
  - `[SaveAs 12/29/25 10:45] SmartPlantPID.a 090000.0144`
  - `[Save 12/30/25 09:12] SmartPlantPID.a 090000.0144`
  - `[unknown (XY) 01/01/26 00:00] SmartPlantPID.a 090000.0144`
- `tests/parse_real_files.rs::doc_version2_decoded_matches_version_history`
  改用 `VersionRecord::operation_label()` 替代内联 match，让 DV2
  `op_type_label` 与 DV3 helper 的任何 silent drift 立即 fail。

### Tests (318 → 324)

lib 单元测试 +5（`version_record_tests` 模块）：
- `version_record_is_save_as_matches_sa_literal`
- `version_record_is_save_matches_sv_literal`
- `version_record_operation_label_echoes_unknown_to_flat_string`
- `version_record_parsed_timestamp_happy_path`
- `version_record_parsed_timestamp_returns_none_for_malformed`

report 单元测试 +1：
- `report_version_history_uses_operation_label_instead_of_raw_code`
  —— 同时 assert "SaveAs"/"Save"/"unknown (XY)" 三种格式 + 保护
  "raw [SA ...]" 旧格式不再出现的 regression。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **324 passed** / 0 failed

### Docs

- `docs/plans/2026-04-21-phase-10d-docversion3-operation-helpers.md`

## [0.6.2] - 2026-04-21

### Phase 10c: cluster & dynamic-attrs 动态 probe（完成 v0.6.1 parking）

v0.6.1 把 4 个 cluster / dynamic-attrs 顶层流从动态分类里显式排除
（`stream_is_populated` 返回 `None`，走静态 pass-through），理由是
它们对应多流合并聚合的 `doc.clusters: Vec<ClusterInfo>` /
`doc.dynamic_attributes: Option<DynamicAttributesBlob>`，探针写法
和单一流字段不同。本轮为这 4 个流都配齐动态 probe，完成 v0.6.1 的
parking list。

### 新 probe 规则

| 流名 | probe |
|---|---|
| `PSMcluster0` | `doc.clusters.any(kind == PsmCluster)` |
| `StyleCluster` | `doc.clusters.any(kind == StyleCluster)` |
| `Dynamic Attributes Metadata` | `doc.clusters.any(kind == DynamicAttributesMetadata)` |
| `Unclustered Dynamic Attributes` | `doc.dynamic_attributes.is_some() \|\| doc.clusters.any(kind == UnclusteredDynamicAttributes)` |

`Unclustered Dynamic Attributes` 有两条 surface：DAB blob 或
cluster 里的 `UnclusteredDynamicAttributes` kind，任一填充都算
parser 已识别，避免误降级。

### Changed

- `inspect::coverage::stream_is_populated` 4 个 cluster/dynamic-attrs
  arm 从 `None` 改为具体 probe 表达式。
- `inspect::coverage::document_field_for_known_stream` 4 个对应 arm
  返回具体字段引用（`"clusters (kind=PsmCluster)"` 等），让降级
  note 可操作。
- `src/inspect/coverage.rs` module doc 追加 "v0.6.2 (Phase 10c)" 段。

### Tests (312 → 318)

lib 单元 +6：
- `coverage_downgrades_psm_cluster0_when_no_cluster_kind_psmcluster`
- `coverage_keeps_psm_cluster0_partial_when_cluster_kind_psmcluster_populated`
- `coverage_downgrades_style_cluster_when_no_cluster_kind_style`
- `coverage_downgrades_dynamic_attrs_metadata_when_no_cluster_kind_dam`
- `coverage_keeps_unclustered_dynamic_attrs_when_blob_populated`
- `coverage_keeps_unclustered_dynamic_attrs_when_cluster_kind_populated`

每个都通过 `cluster_of(ClusterKind, name)` test helper 构造
`ClusterInfo`；`UnclusteredDynamicAttributes` 两个 surface 各有一
专门测试。

### Docs

- `docs/plans/2026-04-21-phase-10c-cluster-dynamic-probes.md`。

### Verification

- `cargo fmt --check` / `cargo clippy -D warnings` → 双 0
- `cargo test --all-targets` → **318 passed** / 0 failed

## [0.6.1] - 2026-04-21

### Phase 10b: coverage 动态分类

兑现 Phase 10a 明确承诺的"Phase 10b+ 会动态化"。v0.6.0 的 coverage
是**纯静态**映射（name → bucket），无法区分"流存在 + parser 解出了
模型"vs"流存在但 parser silent-failure"。v0.6.1 让 `classify` 咨询
`&PidDocument` 本身，在对应模型字段为 `None` / 空集合时**降级**到
`IdentifiedOnly` 并附注原因。

降级表（FullyDecoded / PartiallyDecoded → IdentifiedOnly）：

| 流名 | 检查的字段 |
|---|---|
| `\x05SummaryInformation` / `\x05DocumentSummaryInformation` | `summary` |
| `PSMroots` | `psm_roots` + `!entries.is_empty()` |
| `PSMclustertable` | `psm_cluster_table` |
| `PSMsegmenttable` | `psm_segment_table` |
| `DocVersion2` | `doc_version2_decoded` |
| `DocVersion3` | `version_history` + `!records.is_empty()` |
| `AppObject` | `app_object_registry` |
| `JTaggedTxtStgList` | `tagged_storages` |

`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` /
`Unclustered Dynamic Attributes` 的动态探针暂留静态 —— 它们对应的
model shape 是多流合并聚合（`clusters`, `dynamic_attrs`），结构更
复杂，留给 Phase 10c 系统处理。

### Changed

- `inspect::coverage::classify` 内部签名从 `classify(name)` 改为
  `classify(name, doc)`（仍为 private，公共入口
  `coverage_report(&PidDocument)` 签名不变）。
- 新增内部 helper：
  - `apply_dynamic_downgrade(name, static_status, note, doc) -> (status, note)`
    —— 把静态结果 + doc 状态组合成最终结果
  - `stream_is_populated(name, doc) -> Option<bool>` —— 每个流的
    populate probe（`None` = 本版本无探针，静态结果透传）
  - `document_field_for_known_stream(name) -> Option<&'static str>`
    —— 复用 `known_stream_state` 的字段名，避免在 note 生成时重复
    写 mapping 表
- 降级条目的 `note` 被替换为诊断字符串，例如：
  `stream present but parser did not populate the expected 'version_history' field — downgraded from FullyDecoded`

### Added / Changed tests (307 → 312)

lib 单元测试（新 4 条 Phase 10b，更新 3 条 Phase 10a）：
- 新：`coverage_downgrades_docversion3_when_parser_did_not_populate`
- 新：`coverage_downgrades_psm_cluster_table_when_empty_model`
- 新：`coverage_keeps_fully_decoded_when_model_populated`
- 新：`coverage_unknown_and_identified_unaffected_by_model_state`
- 更新：3 条 Phase 10a `coverage_marks_*` 测试现在通过新 helper
  `populate_all_known_fields(doc)` 填充对应 model，保持静态
  `FullyDecoded` / `PartiallyDecoded` 的 baseline 断言稳定。
- `inspect::report::tests::report_includes_coverage_section_*` 同理：
  fixture 填充 `version_history` + `psm_segment_table`。

CLI 集成测试（+1 + 1 existing fixture 升级，`tests/inspect_cli.rs`）：
- 新：`coverage_flag_downgrades_docversion3_when_record_is_illegal`
  —— 真实 CLI 端到端验证降级路径：fixture 写一段非 printable bytes
  到 `/DocVersion3`，parser 静默失败 → CLI `--coverage` 输出
  `[ID]   DocVersion3` 并带 `stream present` 降级 note。
- 升级：`build_mixed_coverage_fixture` 中 `/DocVersion3` 现在写
  合法 48-byte record（helper `legal_doc_version3_record`）让
  `[FULL]` 断言在 Phase 10b 下继续成立；`/PSMsegmenttable` 换成
  `/PSMcluster0`（PSMcluster0 在 Phase 10b 中没有动态探针，静态
  `PartiallyDecoded` 不受降级影响），避免 fixture 构造难度的
  footgun。

### Docs

- `docs/plans/2026-04-21-phase-10b-dynamic-coverage.md`：本轮 dev plan。

### Verification

- `cargo fmt --all -- --check` → 0
- `cargo clippy --all-targets -- -D warnings` → 0
- `cargo test --all-targets` → **312 passed** / 0 failed（lib 219 +
  inspect_cli 3 + parse_real_files 28 + unit_parsers 18 +
  writer_real_files 10 + writer_roundtrip 13 + writer_validate_cli 21）

## [0.6.0] - 2026-04-21

### Phase 10a: SPPID 解析覆盖清单（SPPID full-parse roadmap 的 Phase 1）

开启 SPPID 完全解析的新大周期。v0.5.x "Writer 全功能可编辑" 系列收
束于 v0.5.3 之后，本版本建立 **结构化覆盖清单（coverage inventory）**
作为后续 parser 升级工作的主线驱动。

本 Phase 的核心论点来自 `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md`：

> 当前项目对 SPPID 的解析虽然"入口识别"基本完成，但"完全解码"仍有
> 明显缺口（`PSMclustertable` per-record、`DocVersion3` header 稳定
> 模型、`Sheet*` 深层结构、字节级验证框架）。下一阶段不应直接做零
> 散 parser 修补，而应建立一个**覆盖清单驱动**的持续推进机制。

v0.6.0 交付该清单的**静态版本**，把"已识别"和"已完全解析"从之前
的布尔过滤升级为四态分类（`FullyDecoded` / `PartiallyDecoded` /
`IdentifiedOnly` / `Unknown`），配套 inspect 报告段和 CLI flag。
后续 Phase 10b+ 会把静态映射逐步升级为基于字节消费率的动态分类。

### Added — 覆盖清单基础设施（public API，minor bump）

- `model::ParseCoverageStatus`：四态枚举
  （`FullyDecoded` / `PartiallyDecoded` / `IdentifiedOnly` / `Unknown`），
  按 "覆盖度从高到低" 排序。
- `model::CoverageNodeKind`：`TopLevelStream` vs `TopLevelStorage` 区分。
- `model::CoverageEntry`：单条覆盖记录（`name` / `kind` / `status` /
  `parser` / `document_field` / `note`）。
- `model::CoverageReport`：完整报告（`entries: Vec<CoverageEntry>` +
  `status_counts() -> [usize; 4]` helper）。
- `inspect::coverage`：新模块，提供：
  - `coverage_report(&PidDocument) -> CoverageReport` 公共入口
  - `top_level_coverage_entries(&PidDocument) -> Vec<CoverageEntry>`
    低层切片
- `pid_inspect --coverage` CLI flag：只打 coverage section，不打
  full report；作为 CI / diagnostic 脚本单独消费的入口。

### Added — 静态映射（v0.6.0 初版，Phase 10b+ 会动态化）

硬编码 11 个 `KNOWN_TOP_LEVEL_STREAM_NAMES` 条目到其覆盖状态：

| 流名 | 状态 | 理由 |
|---|---|---|
| `\x05SummaryInformation` | FullyDecoded | Phase 9l Writer 层全 CRUD |
| `\x05DocumentSummaryInformation` | FullyDecoded | 同上 |
| `PSMroots` | FullyDecoded | parser 稳定 |
| `DocVersion2` | FullyDecoded | Phase 9f 逆向成功 |
| `DocVersion3` | FullyDecoded | version_history 稳定 |
| `AppObject` | FullyDecoded | registry parser 稳定 |
| `JTaggedTxtStgList` | FullyDecoded | 稳定 |
| `PSMclustertable` | PartiallyDecoded | 头已知，per-record audit-only |
| `PSMsegmenttable` | PartiallyDecoded | 记录形状部分映射 |
| `PSMcluster0` / `StyleCluster` | PartiallyDecoded | 记录边界已知 |
| `Dynamic Attributes Metadata` / `Unclustered Dynamic Attributes` | PartiallyDecoded | 类/属性表提取、绑定推断 |

`KNOWN_TOP_LEVEL_STORAGE_PREFIXES`（`Sheet*` / `TaggedTxtData` /
`JSite*`）统一 `IdentifiedOnly`；其他顶层名 `Unknown`。

### Changed

- `inspect::report::generate_report` 在 "Top-level Unidentified
  Streams" 段之前新增 `--- Coverage ---` 段，输出 4 个 bucket 计数 +
  逐项 tag（`[FULL]` / `[PART]` / `[ID]` / `[UNK]`）+ 解析器 /
  `PidDocument` 字段 / 备注。
- 旧 `unidentified_top_level_streams` 函数 + 对应 report 段保留，保
  持 backward compat；新旧视角可并存（Phase 10b+ 再决定是否下线
  旧视角）。

### Tests (296 → 307)

lib 单元（+9）：
- `inspect::coverage::tests::coverage_marks_known_top_level_streams_with_expected_status`
- `inspect::coverage::tests::coverage_marks_known_storage_prefixes_as_identified`
- `inspect::coverage::tests::coverage_marks_unknown_top_level_entries_as_unknown`
- `inspect::coverage::tests::coverage_entries_sorted_by_name_deterministic_across_input_orders`
- `inspect::coverage::tests::coverage_report_empty_for_default_document`
- `inspect::coverage::tests::coverage_status_counts_matches_entries`
- `inspect::report::tests::report_includes_coverage_section_with_bucket_counts_and_per_entry_tags`
- `inspect::report::tests::report_omits_coverage_section_when_document_has_no_streams`
- `inspect::report::tests::report_coverage_section_precedes_top_level_unidentified_when_both_present`

CLI 集成（+2，`tests/inspect_cli.rs` 新增）：
- `coverage_flag_prints_section_and_all_four_buckets`
- `no_flags_still_produces_full_report_including_coverage_section`

`cargo fmt --check` / `cargo clippy -D warnings` 双零。

### Docs

- `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md`：SPPID 完全
  解析战略路线图（4 个阶段 + 具体任务 + 风险应对）。正式入 repo 作为
  后续 Phase 10b/10c/... 的导航文档。
- `docs/plans/2026-04-21-sppid-coverage-inventory-implementation-plan.md`：
  Phase 10a Task 1..6 的战术实施 plan，本 ship 的代码即其完成品。
  入 repo 供未来类似 Phase 的 pattern 参考。

### Version rationale

0.5.3 → 0.6.0 走 **minor bump**，不走 patch：

- 新增 public API 类型（`ParseCoverageStatus` / `CoverageNodeKind` /
  `CoverageEntry` / `CoverageReport` / `coverage_report` / `top_level_coverage_entries`）
- 新增 CLI flag (`--coverage`)
- 新增 `CoverageReport` serde/JsonSchema surface
- 这些是 additive 的新周期起点，与 0.5.x "Writer 建设" 的定位区分
  开；未来 10a/10b/... 共用 0.6.x 系列。

Rust API 本身无破坏性改动；既有 0.5.x consumer 代码直接在 0.6.0 下
编译通过。

## [0.5.3] - 2026-04-21

### Phase 9o: Writer API ergonomics patches

四轮 Writer 内部能力扩展（Phase 9k/9l/9m/9n）完成后，本轮回头收拾
下游 consumer 侧的入门样板。全部改动都是 **additive**（新 public
方法），不破坏任何 v0.5.x 已有 API 签名。

### Added

- `PidPackage::from_path<P: AsRef<Path>>(path)` — 简写
  `PidParser::new().parse_package(path)`，两步变一步。
- `PidPackage::from_bytes(&[u8]) -> Result<Self, PidError>` — 从
  内存字节流解析。v0.5.3 内部实现走 tempfile 兜底（HTTP service /
  压缩包 / 嵌入资源都可直接喂字节）；真正的零磁盘纯内存路径依赖
  parser 内部 reader 泛型化，留给 Phase 10a。
- `PidWriter::write_to_bytes(pkg, plan) -> Result<Vec<u8>, PidError>` —
  镜像 `write_to`，直接返回 CFB 字节数组。内部复用新的
  `cfb_write::write_package_to_writer<F: Read + Write + Seek>` 泛型
  backend，避免磁盘往返。
- `cfb_write::write_package_to_writer` 公共泛型入口（Phase 10a 以后
  可能用作 zero-disk 测试基础设施）。
- `WritePlan::from_json(&str)` / `to_json()` / `to_json_pretty()` —
  JSON round-trip helpers，错误一律包装成
  `PidError::ParseFailure { context: "WritePlan JSON", ... }`，
  consumer 不用自行 handle `serde_json::Error`。

### Changed

- `PidWriter::write_to` 内部提取 `apply_plan_to_package` helper，与
  `write_to_bytes` 共用流水线。未来添加新 plan 字段（比如
  `post_write_clsid_set`）只需要改一次。行为完全等价。

### Tests

lib 单元（+7）：
- `api::tests::from_bytes_parses_a_minimal_synthetic_pid`（构造内存
  CFB fixture → `from_bytes` → verify stream 存在）
- `api::tests::from_bytes_on_invalid_data_returns_error`
- `api::tests::from_path_matches_parse_package_behavior`（两个入口
  行为一致）
- `writer::plan::tests::plan_json_round_trip_default_is_passthrough`
- `writer::plan::tests::plan_from_json_rejects_invalid_syntax_with_pid_error`
- `writer::plan::tests::plan_to_json_pretty_contains_newlines_and_indent`
- `writer::plan::tests::plan_from_json_empty_object_is_valid_passthrough`

集成（+2，`tests/writer_roundtrip.rs`）：
- `write_to_bytes_produces_bytes_parseable_by_from_bytes` — 全在线
  round-trip：`from_path` → plan → `write_to_bytes` → `from_bytes`
  → verify edit 落地。
- `write_plan_json_round_trip_preserves_metadata_and_payload_bytes` —
  包含 `summary_updates` + `summary_deletions` + `stream_replacements`
  + `sheet_patches` 完整 plan 的 JSON 往返无损（特别断言 base64
  payload 字节级等价）。

全套 287 → **296 tests pass**（lib 199 → 206 +7；writer_roundtrip
11 → 13 +2）。`cargo fmt --check` / `cargo clippy -D warnings` 双零。

### Docs

- `docs/plans/2026-04-21-phase-9o-api-ergonomics.md`：本轮 dev plan
  含 "不做 parser 泛型化 / tempfile 兜底" 的 trade-off 说明 + Phase
  10a roadmap 衔接。

### Consumer quick-start

```rust
use pid_parse::{PidPackage, PidWriter, WritePlan};

// 从字节流或路径二选一
let pkg = PidPackage::from_path("input.pid")?;
// let pkg = PidPackage::from_bytes(&http_response_body)?;

// 用 JSON 声明式 plan 或直接构造
let plan = WritePlan::from_json(r#"{"metadata_updates":{"summary_updates":{"title":"Q4"}}}"#)?;

// 输出到内存或磁盘
let bytes = PidWriter::write_to_bytes(&pkg, &plan)?;
// PidWriter::write_to(&pkg, &plan, Path::new("output.pid"))?;
```

## [0.5.2] - 2026-04-21

### Phase 9n: `summary_deletions` — SummaryInformation CRUD 收尾

Phase 9l/9m 铺的 Writer 层字符串 property 编辑只有 CREATE（新增）/
UPDATE（覆写）两条语义。本轮补上 **DELETE**，让 SummaryInformation
与 DocumentSummaryInformation 的 property-set 写路径真正支持 CRUD。

### Added

- `WritePlan.MetadataUpdates.summary_deletions: Vec<String>`
  （`#[serde(default)]`，JSON plan 向后兼容）。语义等价于
  `summary_updates` 的逆操作：按符号 key（title / author / subject /
  keywords / comments / template / last_author / rev_number / app_name /
  category / manager / company）定位到 PROPID 并从 section 移除。
  - 空 vec = free no-op
  - key 在 section 里不存在 = 静默 no-op（遵循 stream_replacements
    "删不存在的不 fail" 传统）
  - key 不在符号表 = `UnknownKey` 错误
- `pid_parse::writer::summary_write::apply_summary_deletions` 公共入口。
- `SummarySection::remove(prop_id)` 内部方法（Phase 9l 的
  `SummaryPropertySet` 只多一个方法，其他基础设施复用）。
- `pid_writer_validate --delete-summary KEY` 便利 CLI flag，对称于
  `--set-summary KEY=VALUE`。可多次传入累加；与 `--set-summary` 在
  *不同* key 上可共存，同 key 则报冲突错。与 `--apply-plan` 互斥。
- `ValidateError::Edit("summary_updates and summary_deletions both target
  key '{k}'")`：同 key 同时出现在 set/delete 两个字段时的明确拒绝，
  在 lib 层 (`apply_metadata_updates`) 和 CLI 层 (`run_validate`) 都有
  pre-check。

### Changed

- `apply_metadata_updates` 执行顺序正式定义：`drawing_xml` →
  `general_xml` → `summary_deletions` → `summary_updates`。先删再增
  保证 edge case "删 A、加 A" 的最终态一致（与冲突拒绝共存只是防御性
  guard；即便 guard 放过也不会产生 inconsistent state）。
- `pid_writer_validate`：
  - `print_usage` 更新列出 `--delete-summary` 段与 "cannot be combined
    with … --delete-summary" 的精确错误消息。
  - `run_validate` / `compare_packages` 签名新增 `summary_deletions`
    参数，`edited_paths` 自动扩展到 summary 两流（保证删除后字节级
    变化归入 `edited` 而非 `mismatched`）。
  - `collect_edited_paths_from_plan` 亦同步识别 `summary_deletions`。
- `WritePlan::is_passthrough` 扩展：`summary_deletions.is_empty()` 加入
  判空链。
- `WritePlan::metadata_only` 构造函数新增 `summary_deletions:
  Vec::new()` 初始化。

### Tests

lib 单元测试（`src/writer/summary_write.rs::tests`，+5 条）：
- `apply_summary_deletions_removes_existing_prop`
- `apply_summary_deletions_nonexistent_key_is_silent_noop`（断言静默
  no-op 且**流字节保持不变**，避免 `modified: true` 误染 diff）
- `apply_summary_deletions_unknown_key_returns_error`
- `apply_summary_deletions_empty_is_zero_cost_noop`
- `apply_summary_deletions_preserves_filetime_byte_for_byte`（非目标
  prop 字节级保留的 Phase 9l 契约，在 delete 路径上再验证一次）

lib 单元测试（`src/writer/metadata_write.rs::tests`，+1 条）：
- `summary_updates_and_deletions_on_same_key_return_error`

CLI 集成测试（`tests/writer_validate_cli.rs`，+4 条）：
- `validate_delete_summary_removes_target_prop`
- `validate_delete_and_set_summary_combine_legally`
- `validate_delete_summary_conflicts_with_set_summary_on_same_key`
- `validate_delete_summary_unknown_key_exits_two`

Real-file 集成测试（`tests/writer_real_files.rs`，+1 条件性条）：
- `real_file_delete_summary_prop_when_present`：fixture 存在且有
  summary 流时删除 `keywords`（或 fallback 到 `title`），verify 非
  summary 流全字节等价 + 目标 prop 在 reader 视角消失。

全套 276 → **287 tests pass**（lib 193 → 199 +6；writer_real_files
9 → 10 +1；writer_validate_cli 17 → 21 +4）。

### Docs

- `docs/plans/2026-04-21-phase-9n-summary-deletions.md`：本轮 dev plan。
- `docs/writer-quickstart.md` 5.6 节已在 Phase 9m 的 "CLI 快捷方式"
  块里把 `--delete-summary` 作为第 9n 版延伸注解。

## [0.5.1] - 2026-04-21

### Phase 9m: `--set-summary` CLI flag + real-file integration

Corner-case convenience pass on top of Phase 9l (v0.5.0) — turns the
SummaryInformation writer from "编辑需要手写 plan.json" into "可用单个
命令行 flag 直接改"。

### Added

- `pid_writer_validate --set-summary KEY=VALUE`：特化便捷 flag，对称于
  `--edit` (drawing XML) / `--general-edit` (general XML)。多次 `--set-summary`
  会累加到同一个 summary map；后传覆盖先传。支持所有 Phase 9l
  KEY_TO_*_PROPID 表里的 11 个 key。
- `run_validate` 函数签名新增 `summary_edits: &BTreeMap<String, String>`
  参数（binary 专用 public API；CLI 集成测试可直接传；lib consumer 不受
  影响）。
- `tests/writer_validate_cli.rs` 4 条新集成测试：
  - `validate_set_summary_single_key_rewrites_title`
  - `validate_set_summary_multiple_keys_accumulate`（title / author /
    subject 一次调用累加）
  - `validate_set_summary_conflicts_with_apply_plan_exits_one`
  - `validate_set_summary_unknown_key_exits_two_with_clear_error`
- `tests/writer_real_files.rs` 1 条 conditional 测试
  `real_file_set_summary_title_preserves_other_streams`：当
  `test-file/DWG-0201GP06-01.pid` 存在并含 `/\x05SummaryInformation` 时
  验证真实 `.pid` 端到端 → 写 title → parse → 断言只有 summary 流变，
  其他所有流 byte-identical；fixture 缺失或无 summary stream 时 skip。

### Changed

- `--apply-plan` 与 `--set-summary` 互斥：同时传返回 usage error（exit 1），
  stderr 含 `--set-summary` 字样。延续 Phase 9b 的"declarative plan 与
  特化 flag 互斥"设计。
- `compare_packages` / `collect_edited_paths_from_plan` 都扩展到把
  `/\x05SummaryInformation` 和 `/\x05DocumentSummaryInformation` 纳入
  "可能被编辑"的 stream 集合，保证 `edited` vs `matched` 计数在
  summary 改动时不误报 `mismatched`。
- `docs/writer-quickstart.md` 5.6 节追加 CLI 用法示例。

### API surface

- `pid_parse::writer::summary_write::SUMMARY_INFO_PATH` /
  `DOC_SUMMARY_PATH` 从 `pub(crate)` 提升为 `pub`，让 binary 和外部
  consumer 能引用同一组常量而不用硬编码字节。

### Tests

全套 271 → **276 tests pass**（writer_real_files 8 → 9 +1；
writer_validate_cli 13 → 17 +4）。`cargo fmt --check` / `cargo clippy
--all-targets -D warnings` 双零。

### Docs

- `docs/plans/2026-04-21-phase-9m-summary-cli-integration.md`：本轮 dev plan。
- `docs/writer-quickstart.md` 5.6 节："编辑 SummaryInformation"新增
  `--set-summary` CLI 用例块。

## [0.5.0] - 2026-04-21

### Phase 9l: SummaryInformation / DocumentSummaryInformation property-set writer

`MetadataUpdates.summary_updates` 从 **parked placeholder** 变为**真正可
编辑的 OLE 属性流接口**，补全 Writer 层最后一个"骗用户"的 API。

从 v0.4.2 起，apply-plan JSON 里填 `summary_updates` 字段会被静默吞掉；
从 v0.5.0 起，会真实写入 `/\x05SummaryInformation` /
`/\x05DocumentSummaryInformation` 的 OLE property-set。这是**对外语义
变化**（向后兼容的 Rust API，但 JSON plan consumer 的行为改变），因此
走 **minor bump**（0.4.x → 0.5.x 开启新周期："Writer 全功能可编辑"）。

### Added

- `src/writer/summary_write.rs` **新模块**：
  - `SummaryPropertySet` 内部类型，parse + serialize OLE property-set
    stream（[MS-OLEPS] 规范），支持 byte-level round-trip。
  - `apply_summary_updates(pkg, updates)` 公共入口，按符号名 key 定位
    PROPID + 目标 stream，仅编辑 `VT_LPSTR` / `VT_LPWSTR` 字符串型
    property；未触及的 property（含 `VT_FILETIME` / `VT_I4`）字节级
    保留。
  - 支持 key 列表（11 条）：
    - SummaryInformation 段：`title`, `subject`, `author`,
      `keywords`, `comments`, `template`, `last_author`,
      `rev_number`, `app_name`
    - DocumentSummaryInformation 段：`category`, `manager`, `company`
  - 清晰的错误分类（全部包装为 `PidError::ParseFailure { context:
    "summary writer", ... }` 不破坏 public error surface）：
    - unknown key（列出已知 key 表）
    - 目标 stream 不存在（提示用 `stream_replacements` 塞）
    - 未支持的 source VT 类型（避开 FILETIME / I4 写入）
    - 非 ASCII 写入 `VT_LPSTR` 字段（提示 Phase 9m 会支持 UTF-8）
  - 常量 `SUMMARY_INFO_PATH` / `DOC_SUMMARY_PATH` 和标准 FMTID
    (`F29F85E0-4FF9-1068-AB91-08002B27B3D9` /
    `D5CDD502-2E9C-101B-9397-08002B2CF9AE`) 内联定义。

### Changed

- `writer::plan::MetadataUpdates.summary_updates` 文档从 "Placeholder —
  silently ignored" 更新为"实际生效"，列出 11 个可用 key 和 encoding
  规则。字段签名（`BTreeMap<String, String>`）不变，现有 Rust consumer
  零破坏。
- `writer::metadata_write::apply_metadata_updates` 在 drawing / general
  XML 写入之后调用 `apply_summary_updates`。空 map = 0 开销。
- `writer/mod.rs` 模块 doc 去掉"no SummaryInformation property-set
  writer"的 caveat。

### Tests

lib (unit)：
- `writer::summary_write::tests::parse_then_serialize_is_byte_identical_for_untouched_stream`
- `writer::summary_write::tests::apply_summary_updates_passthrough_empty_map_touches_nothing`
- `writer::summary_write::tests::apply_summary_updates_edits_title_and_preserves_filetime`
  （关键断言：FILETIME prop 改写其他 prop 后 byte-for-byte 不动）
- `writer::summary_write::tests::apply_summary_updates_rejects_unknown_key`
- `writer::summary_write::tests::apply_summary_updates_adds_new_string_prop_when_absent`
  （新 prop 默认 `VT_LPWSTR`）
- `writer::summary_write::tests::apply_summary_updates_returns_stream_not_found_when_missing`
- `writer::summary_write::tests::encode_lpstr_rejects_non_ascii`
- `writer::summary_write::tests::encode_lpwstr_accepts_unicode`

集成（`tests/writer_roundtrip.rs`）：
- `summary_updates_rewrite_title_end_to_end_through_pid_writer`：完整
  链路 `PidWriter::write_to → CFB → parse → SummaryInfo.title`。
- `summary_updates_unknown_key_fails_writer_with_clear_error`：错误传
  播到 Writer top-level。

全套 261 → **271 tests pass**（lib 185 → 193 +8；writer_roundtrip 9 → 11 +2）。

### Docs

- `docs/plans/2026-04-21-phase-9l-summary-info-writer.md`：本轮 dev plan
  （scope / 关键设计决策 / 5-7 hr W1-W6 步骤 / 风险缓解表 / 回滚策略 /
  Next 候选）。

### Known limitations (tracked for future phases)

- `VT_LPSTR` 字段不接受非 ASCII 值（Phase 9m 计划支持 UTF-8 / CP1252）。
- DocumentSummaryInformation 第二个 section（user-defined dictionary）
  不编辑；section 2..N 的原字节 verbatim 透传（Phase 9n）。
- 不支持删除 property（`summary_deletions` 字段挂位，future minor bump）。
- 不支持从零新建 `/\x05SummaryInformation` stream；源 package 必须
  已有此 stream，否则返回 `stream does not exist` 错误（由用户先走
  `stream_replacements` seed）。

## [0.4.2] - 2026-04-21

### Phase 9k: Ship `--apply-plan` + P3 cleanups + lint/fmt restore

从 [0.4.1] 合并：layout 语义关键词数据驱动重构（`48135a8`）+ Writer 层
`--apply-plan` CLI（`3a2ecde`）。同时扫清 Phase 9i 之后悄悄堆积的 **7 条
clippy warning**（lib-test 代码里 6 条 `field_reassign_with_default` +
1 条 `map_clone` / `iter_cloned_collect`）与 10 文件的 `cargo fmt`
漂移，执行 `phase8-9h-summary.md` 里列的 P3 cleanups 中 3 条低风险项
（#1 `file_stem` 跨平台、#3 `diff.rs` `writeln!().unwrap()`、#4 tests
use 散落；#2 `representative_symbol_hints` 缓存留给下一轮性能 Phase）。

全套从 260 增至 **261 tests pass**（lib 185 + parse_real_files 28 +
unit_parsers 18 + writer_real_files 8 + writer_roundtrip 9 +
writer_validate_cli 13 = 261）。`cargo clippy --all-targets -- -D warnings`
和 `cargo fmt --all -- --check` 均返回 0 退出码。

### Added

- `pid_writer_validate --apply-plan <plan.json>`：一次性施加完整 `WritePlan`
  （metadata XML / stream replacements / sheet patches）并走 round-trip +
  byte-diff verify。与 `--edit` / `--general-edit` 互斥；`--json` 输出
  扩展 `plan_applied` 字段。
- `Cargo.toml`：`base64 = "0.22"` 依赖（WASM / no_std 友好，为批量 CLI
  及未来跨语言 binding 共用）。
- `src/layout.rs::file_stem_cross_platform`：内部 helper，先把 Windows
  反斜杠 UNC 路径归一化为正斜杠再喂 `Path::file_stem`，消除 Linux CI 上
  `\\srv\sym\piping\valve.sym` 被当成单一文件名返回整串的怪行为。

### Changed

- `WritePlan`、`MetadataUpdates` 字段追加 `#[serde(default)]`，让 `{}`
  成为合法 JSON passthrough，`{"metadata_updates":{"drawing_xml":"..."}}`
  也无需显式写 `general_xml: null` / `summary_updates: {}`。保留 Rust
  侧 `WritePlan::default()` 行为不变。
- `StreamReplacement.new_data` / `SheetChunkPatch.replacement` 的 JSON
  序列化由 `Vec<u8> = [int array]` 改为**标准 base64 字符串**（`A-Z a-z
  0-9 + / =`）。Rust consumer 透明，JSON 大小约缩减 6x。内部 `#[serde(with
  = "bytes_base64")]` 自定义 adaptor，反序列化失败走 serde error。
- `src/layout.rs`: 语义关键词推断改为数据驱动。新增 `SEMANTIC_KEYWORDS` 常量表
  （`OffPageConnector` / `Nozzle` / `Instrument` / `Vessel` / `Note` /
  `PipingComponent`），以及每个 tag 的英文 + 中文同义词列表，取代原先 if/else
  链。行为对既有 fixture 保持等价，新增中文 symbol 路径（例如 `\\srv\sym\管件\球阀.sym`）
  的语义命中。顺序依赖显式保留（`OPC` 先于 `valve`）。
- `src/layout.rs`: `representative_symbol_hints` 的 tiebreaker 抽成
  `should_replace_representative(existing_count, existing_path, candidate_count,
  candidate_path)` helper，带 doc comment 说明 "higher usage_count wins; ties
  break on lexicographically smaller path" 规则，替代原 inline 表达式。
- `src/inspect/diff.rs::render`：11 处 `writeln!(&mut out, ...).unwrap()` 改为
  `String::push_str` / `push_str(&format!(..))`。`writeln!` 对 `String`
  的 `fmt::Write` impl 技术上不会 fail，但 `.unwrap()` 让读者每次都要
  自己重新确认这一点；改成 `push_str` 直接消除这个认知负担，也顺带去掉
  `use std::fmt::Write` import。

### Fixed (lint / fmt restore)

- `src/import_view.rs` / `src/inspect/mod.rs` / `src/layout.rs` 6 处
  `field_reassign_with_default` clippy lint（均在 lib-test 代码里），
  改成 struct-literal + `..Default::default()` 形式。
- `src/inspect/mod.rs::tests::unidentified_filters_all_known_top_level_names`
  里 `.iter().map(|s| *s).collect::<Vec<&str>>()` → `.to_vec()`
  （clippy `iter_cloned_collect`）。
- `cargo fmt --all` 应用 10 文件的 whitespace / line-break 漂移：
  `src/bin/pid_inspect.rs`, `src/bin/pid_writer_validate.rs`,
  `src/import_view.rs`, `src/inspect/mod.rs`, `src/layout.rs`,
  `src/model.rs`, `src/parsers/sheet_probe.rs`,
  `src/writer/metadata_helpers.rs`, `src/writer/plan.rs`,
  `tests/writer_validate_cli.rs`。

### Tests

- `layout::tests::infer_semantic_maps_chinese_symbol_path_to_piping_component`
- `layout::tests::infer_semantic_keyword_ordering_keeps_opc_before_piping`
- `layout::tests::should_replace_representative_covers_all_three_rules`
- `layout::tests::infer_semantic_normalizes_backslash_path_across_platforms`
  （Phase 9k 新增；回归守 P3-1 `file_stem_cross_platform` helper 对
  `\\srv\...` 和 `//srv/...` 两种风格返回相同 stem，且关键词匹配等价）
- `writer::plan::tests::stream_replacement_round_trips_through_json_with_base64_payload`
- `writer::plan::tests::sheet_chunk_patch_round_trips_through_json_with_base64_payload`
- `writer::plan::tests::deserialize_rejects_invalid_base64`
- `tests/writer_validate_cli.rs`：5 条新集成测试覆盖 `--apply-plan`
  （passthrough `{}` / drawing 元数据整体替换 / base64 stream 替换 /
  非法 JSON exit 2 / 与 `--edit` 冲突 exit 1）。

### Docs

- `docs/plans/2026-04-19-layout-symbol-hint-p2-fixes.md`：layout P2 dev plan，包含
  "审核自纠" 一节说明 P2-1 撤回的理由（恢复 `file_stem()` 回退反而会让
  `bounds_for_item` fall through 到默认尺寸，丢失 `PipingComponent` 的 18×18
  命中；4c1cb80 的"坍塌到语义 tag"是正向设计）。
- `docs/plans/2026-04-19-apply-plan-cli.md`：`--apply-plan` 的 dev plan。
- `docs/plans/2026-04-21-phase-9k-ship-and-p3.md`：本轮 Phase 9k 的 dev plan
  （ship v0.4.2 + P3 cleanups + lint/fmt restore）。
- `docs/writer-quickstart.md` 新 5.5 节"批处理 via `--apply-plan <plan.json>`"：
  JSON schema 说明 + CLI 调用样例 + Rust 侧构造 plan 并 serialize 示例。

## [0.4.1] - 2026-04-19

### Phase 9k: cfb 0.10 → 0.14 升级 + 时间戳 + state_bits 保真

`cfb` 在 2026-02-13 发布 0.14.0，新增 `set_created_time` / `set_modified_time` / `set_state_bits` 三个关键 API —— **直接解锁 Phase 9e 文档里标记"cfb upstream 依赖" 的全部遗留限制**。升级零 breaking changes 直接编译通过。

真实样本 `DWG-0201GP06-01.pid` 承载 **25 个非 epoch storage timestamps**（root / 19 × JSite / PSMspacemap / TaggedTxtData），从 Phase 9a (v0.3.3) 起的 passthrough round-trip 一直在**悄无声息地丢失**这些时间戳（旧 cfb 0.10 只能 `touch(path) = now`）。Phase 9k 一并修复。

### 升级依赖

- `cfb = "0.10"` → `cfb = "0.14"`（零 breaking，无代码改动即可编译）
- 新增 cfb 的传递依赖 `web-time = "1"`

### 模型扩展

- `PidPackage.storage_timestamps: BTreeMap<String, StorageTimestamps>` 新字段
  - `StorageTimestamps { created: Option<SystemTime>, modified: Option<SystemTime> }`
  - CFB-spec epoch (1601-01-01) 被 parser 归一化为 `None`（避免把"未设置" 误报为"1601年")
- `PidPackage.state_bits: BTreeMap<String, u32>` 新字段
  - 仅记录非零值（零是 CFB 默认，sparse map）
- `PidPackage::with_storage_timestamps(...)` / `with_state_bits(...)` builder 方法

### Parser / Writer 改动

- `parse_pid_package`：单次 `cfb.walk()` 一并采集 CLSID / timestamps / state_bits（避免多次 walk）
- `writer::cfb_write::write_package`：新增 step 5（`set_created_time` / `set_modified_time`）+ step 6（`set_state_bits`）

### Diff 扩展

- `PackageDiff.storage_timestamp_diffs: Vec<StorageTimestampDiff>` 新字段
- `PackageDiff.state_bits_diffs: Vec<StateBitsDiff>` 新字段
- `is_empty()` / `diff_count()` 纳入新维度
- `inspect::diff::render` 新增 `--- Storage Timestamp Diffs ---` / `--- State Bits Diffs ---` 段，`render_time` 用 `unix+Ns` 稳定格式（无需 chrono 依赖）

### Report 扩展

- `generate_package_report` 新增 `--- Storage Timestamps (N) ---` / `--- State Bits (N) ---` 段展示
- 真实样本运行 `pid_inspect drawing.pid` 默认可以看到全部 25 个 storage 的 created/modified 时间戳

### re-export

- `pid_parse::{StorageTimestamps, StorageTimestampDiff, StateBitsDiff}` 新导出

### 测试

- 模块内单元测试 +2：`diff_flags_storage_timestamp_mismatch` / `diff_flags_state_bits_mismatch`
- `tests/writer_roundtrip.rs` +1：`storage_timestamps_and_state_bits_round_trip`（内存 fixture 烧任意 created/modified/state_bits → round-trip 完整保留）
- `tests/writer_real_files.rs` +2：
  - `real_file_passthrough_preserves_storage_timestamps`（真实样本 25+ timestamps 全部保真）
  - `real_file_passthrough_produces_empty_diff_full`（6 个维度全部 0 diff）
- **总计 177 个测试通过**（从 172 → 177，新增 5 个）

### 修复：Phase 9a-9g 的 "passthrough 0 diffs" 实际上一直在丢数据

Phase 9a 起 `--round-trip --verify` 报告 "verified: 0 diffs"，但旧 `diff_packages` 只看 stream 字节 + root CLSID，不看非 root CLSID（Phase 9e 补）、不看 timestamps / state_bits（Phase 9k 补）。v0.3.13 起 **"0 diffs"** 真正意味着 "容器级字节几乎无损"。

### 文档

- `docs/writer-clsid-and-timestamps.md` 全线改写：能力矩阵从"3 层保真"升级到"6 层保真"；新增 v0.3.13 验证清单；历史升级表
- `ARCHITECTURE.md`：能力边界 v0.3.13

## [0.3.12] - 2026-04-19

### Phase 8c: Layout-first 可读整图模型（供 H7CAD PID 工作台消费）

在既有 `PidDocument + ObjectGraph + CrossReferenceGraph` 之上新增面向显示的 `layout` 真值层，让下游不必再把 `.pid` 对象简单摆成网格圆点，而能生成一份**可读整图**所需的布局摘要。此层仍是 visualization model，不追求 SmartPlant 原始几何逐字节/逐像素复刻。

### 公共 API

- `PidDocument` 新增可选字段 `layout: Option<PidLayoutModel>`
- 新增类型：
  - `PidLayoutModel { items, segments, texts, unplaced, warnings }`
  - `PidLayoutItem { layout_id, drawing_id, graphic_oid, kind, anchor, bounds, symbol_name, symbol_path, label, model_id }`
  - `PidLayoutSegment { layout_id, owner_drawing_id, graphic_oid, start, end, role }`
  - `PidLayoutText { layout_id, drawing_id, text, anchor, bounds }`
  - `PidLayoutUnplaced { drawing_id, kind, label }`
- 新增导出函数：
  - `derive_layout(doc: &mut PidDocument)`
  - `build_layout_model(doc: &PidDocument) -> Option<PidLayoutModel>`

### 布局推导规则

- 统一支持两类输入证据：
  - `bundle mode`：消费 sidecar `_Data.xml` 带入的 `PIDRepresentation / DwgRepresentationComposition / DefUID` 关系
  - `pid-only mode`：消费 `.pid` 内已解出的对象图与关系图
- `PIDRepresentation` 的 `GraphicOID` 会通过 `DwgRepresentationComposition` 转移到被表示对象上，供图形层选择/联动使用
- layout 的连线只消费已证实的物理关系角色：`PipingEnd1Conn` / `PipingEnd2Conn` / `PipingTapOrFitting` / `ProcessPointCollection`；无证据时不伪造线
- 未能放进主图的对象进入 `unplaced`，由下游单独做 fallback rail，不再混排到主图

### 符号语义增强

- `infer_symbol_identity` 不再只覆盖 `Pipeline / Branch / Connector / Instrument / Equipment`
- 新增 bundle/真实样例驱动的语义类别：
  - `ProcessPoint`
  - `Note`
  - `Nozzle`
  - `OffPageConnector`
  - `PipingComponent`
  - `Vessel`
  - `PipingPort / SignalPort`
- 若对象 extra 中可见 `.sym` 路径，现会保留到 `symbol_path`，并尽量从 basename 回填 `symbol_name`

### 测试

- 新增 `layout::tests::build_layout_model_classifies_bundle_specific_symbol_kinds`
- 真实样例护栏保持：
  - `tests/parse_real_files.rs::second_file_builds_readable_layout_model`
- 验证：
  - `cargo test --manifest-path D:/work/plant-code/cad/pid-parse/Cargo.toml`
  - 结果：`137` lib tests + `28` parse_real_files + `18` unit_parsers + `1` writer_real_files + `7` writer_roundtrip + `8` writer_validate_cli 全绿

### 设计边界

- `layout` 是**可读整图布局模型**，不是 `.sym` 原始几何解码
- 不做 SmartPlant 原始线型、标注、字高、版式的像素级复刻
- `symbol_name/symbol_path` 目前仍是 best-effort 语义证据；后续若把 `JSite` 真正挂接到对象级，可继续细化
- v0.4.1 同段后续补丁：`layout` 现会从 `cross_reference.symbol_usage` / `jsites` 提取代表性 `.sym` 路径作为 pid-only 的对象级 symbol hint。典型受益对象包括 `OPC`、`PipingComp`、`Nozzle` 等粗类型；即便没有 sidecar XML，也能把 `Off-Drawing.sym` / `Cap2.sym` / `Flanged Nozzle.sym` 一类证据下沉到 `PidLayoutItem.symbol_path`

### Phase 8b: Metadata 编辑 helper（为 H7CAD UI 编辑桥铺路）

在 v0.4.0 Writer 层之上新增 `src/writer/metadata_helpers.rs` 纯函数模块，让上层不再需要自己拼/改 XML 字节即可对 `/TaggedTxtData/Drawing` 与 `/TaggedTxtData/General` 做"改一点点"式编辑。所有 helper 都是 byte-level splice — 除被替换的属性值/元素文本外，其它字节（注释、空白、引号风格、兄弟属性顺序）逐字节保留，最大化 SmartPlant 兼容性。

### 公共 API

- `set_drawing_attribute(xml, attr, value) -> Result<String, MetadataEditError>`：替换 `<Tag attr="value"/>` 风格的属性值；要求左侧是空白或开头、右侧是 `=` 或空白后 `=`，从而 `MY_ATTR` 不会误匹配 `EXTRA_MY_ATTR`
- `set_drawing_number(xml, value)`：`set_drawing_attribute(xml, "SP_DRAWINGNUMBER", value)` 的便利别名
- `set_element_text(xml, element, value)`：替换 `<E>text</E>` 形式的元素文本内容；自闭合标签 `<E/>` 直接报 `MalformedElement`
- `set_general_file_path(xml, value)`：先试 `<FilePath>` 后退回 `<Path>`，与 `parsers/general_xml.rs` 的接受面对齐

### `MetadataEditError`（`thiserror::Error + PartialEq`）

- `AttributeNotFound { attr }` / `ElementNotFound { element }`：找不到目标
- `DuplicateAttribute { attr, count }` / `DuplicateElement { element, count }`：拒绝二义编辑（调用方需先把范围缩到唯一的位置）
- `UnterminatedAttribute { attr }`：属性值起始 `"` 后找不到结束 `"`
- `MalformedElement { element }`：自闭合或缺闭合标签

### XML 转义

- 属性值：`& < > " '` 全部转 entity；调用方传裸字符串
- 元素文本：`& < >` 转 entity（属性专用的 `" '` 不转，避免误伤合法文本）

### 测试

- `writer::metadata_helpers::tests` **18 个全绿**：
  - 简单替换 / 保留兄弟属性 + 空白 / 不匹配长名后缀 / 未找到 / 重复 / XML 转义 / 空字符串 / Unicode（中文 + №）
  - 元素文本：基本替换 / 带属性 / 自闭合拒绝 / 未找到 / 转义 / 不匹配长元素名后缀
  - `set_general_file_path` FilePath 优先 + Path 回退
  - 空 XML 双类返回各自的 NotFound
- `cargo test --lib`：**100/100 通过**（93 → 100，新增 18 - 11 个之前没数到的，无 regression）
- `cargo test --test writer_roundtrip` 7/7、`--test writer_real_files` 1/1 全绿
- 已知失败：`tests/parse_real_files.rs` 26 个 + `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 都依赖未提交的 `test-file/DWG-0201GP06-01.pid`，与本次无关

### 设计取舍

- **byte-level vs XML re-emit**：选 byte 级是为了兼容性。`quick_xml::Reader` → `Writer` 的 round-trip 会归一化引号风格 / 属性顺序 / 空白；对 SmartPlant 这种 picky reader 容易引入"我没改这里它怎么也变了"的视觉 diff
- **不处理 BOM / UTF-16**：跟 v0.4.0 risk note 一致；helper 假设输入是 UTF-8 文本。下一迭代如发现真实 `/TaggedTxtData/*` 是 UTF-16，可在 `metadata_write::apply_metadata_updates` 一处统一做编码 round-trip
- **重复匹配显式拒绝**：与其 silently 替换"第一个"或"最后一个"，不如让调用方知道范围太宽并先 narrow（例如 SmartPlant 模板里有多张 sheet 的标题块共享 SP_DRAWINGNUMBER 时，应分别编辑每个 sheet 流的 Drawing XML）

### 公共 API 增量

- 新类型：`MetadataEditError`
- 新函数：`set_drawing_attribute` / `set_drawing_number` / `set_element_text` / `set_general_file_path`
- 配套读取器（v0.4.1 同段补丁）：`get_drawing_attribute(xml, attr) -> Option<String>` 与 `get_general_element_text(xml, element) -> Option<String>` —— "exactly once" 语义与 set 端的"重复拒绝"对偶；duplicates / not-found / 自闭合（element 版本）都返回 `None`
- `writer` 模块直接 re-export 全部新 API；`PidWriter` / `WritePlan` / `MetadataUpdates` 行为不变
- bulk 读取器（v0.4.1 同段补丁）：`list_drawing_attributes(xml) -> Vec<(String, String)>` 列出所有 `<TagName attr="value"…/>` 属性对、`list_general_elements(xml) -> Vec<(String, String)>` 列出所有 `<element>text</element>` leaf 对（自闭合 / 含子元素跳过）；都按源序返回，跳过 PI/comment/CDATA 前缀
- **新 binary `pid_writer_validate`**（v0.4.1 同段补丁）：CLI 工具，对真实 `.pid` 文件做 `parse_package → PidWriter::write_to (passthrough) → re-parse_package` 完整 round-trip，按 path 列出 stream byte 级 diff，支持 `--out` / `--keep` / `--json` / `--quiet` / `--max-diff-bytes`；exit 0=PASS / 1=mismatch / 2=parse-IO/edit 错；公共 helper `run_validate(input, output, max_diff_bytes, edits) -> Result<ValidateReport, ValidateError>` 与 `ValidateReport`/`StreamMismatch`/`EditOp`/`EditKind`（带 `serde::Serialize`）暴露给测试与下游集成。`tests/writer_validate_cli.rs` 8 个端到端测试通过 `env!("CARGO_BIN_EXE_pid_writer_validate")` 驱动 CLI（不引新依赖，无 assert_cmd / escargot）
- **`--edit ATTR=VALUE` / `--general-edit ELEMENT=VALUE`**（v0.4.1 同段补丁）：让 CLI 支持"编辑后再验证"模式。任意条数 edit 在 round-trip 之前应用到 source 包；roundtrip 用 **edited** 包作对照基准，被 edit 触碰的流标 `EDITED`、未触碰的流仍要字节级匹配。报告新增 `edited` 计数与 `edits_applied` 数组；新错误变体 `ValidateError::Edit` 透传 `MetadataEditError`；CLI 用 `splitn(2, '=')` 切 ATTR/VALUE 让 value 含 `=` 也安全
- **`inspect::unidentified_top_level_streams` 可发现性 API**（v0.4.1 同段补丁）：新 pub 函数 `pid_parse::inspect::unidentified_top_level_streams(&PidDocument) -> Vec<&StreamEntry>` 返回 pid-parse 尚未识别的顶层 CFB 流（解码工作的待办清单）；配套 pub 常量 `KNOWN_TOP_LEVEL_STREAM_NAMES` + `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 作为识别白名单；`inspect/report.rs` 内部的 "Top-level Unidentified Streams" 段改用此 API，人类输出一致
- **条件测试降级**（v0.4.1 同段补丁）：`tests/parse_real_files.rs`（26 测试）+ `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 改为"fixture 缺失 eprintln! + return"，与 `tests/writer_real_files.rs` 同风格；消除 `cargo test` 在缺 `test-file/DWG-0201GP06-01.pid` 时的噪音失败。新增条件测试 `top_level_unidentified_streams_are_empty_on_sample_file` 把"样本文件顶层流全部已识别"锁为不变量，回归警报
- **`ObjectGraph` 图遍历便利方法**（v0.4.1 同段补丁）：`impl ObjectGraph` 加 4 个 ergonomic 查询方法 + 1 个新结构体：
  - `pub fn object_by_drawing_id(&self, drawing_id: &str) -> Option<&PidObject>`：O(log N) 索引化查找
  - `pub fn relationships_touching(&self, drawing_id: &str) -> Vec<&PidRelationship>`：返回 source/target 任一为该 id 的关系
  - `pub fn neighbors_of(&self, drawing_id: &str) -> Vec<&PidObject>`：通过关系边解析的对端对象，去重 + 跳过自环
  - `pub fn endpoint_resolution_stats(&self) -> EndpointResolutionStats`：fully/partially/unresolved 三态汇总
  - `pub struct EndpointResolutionStats { total, fully_resolved, partially_resolved, unresolved }` (Serialize/Deserialize/JsonSchema/Default)
  - 6 个新单元测试 (`model::object_graph_impl_tests`) 覆盖空图/已知/未知/自环/三态计数
  - 配套增量：`pub fn find_drawing_ids_by_prefix(&self, prefix: &str) -> Vec<&str>`，`BTreeMap::range`-backed O(log N + K)；空 prefix 返回所有 id；4 个新单测覆盖排序/未匹配/多匹配/长 prefix 等价精确
  - 搜索增量：`pub fn find_objects_by_item_type(&self, &str) -> Vec<&PidObject>` 与 `pub fn find_objects_by_extra(&self, key, value) -> Vec<&PidObject>`，O(N) 线性扫；4 个新单测覆盖匹配/未匹配/extra key 缺失/value 不匹配。`object_graph_impl_tests` 共 14/14 全绿
  - BFS 多跳遍历增量：`pub fn neighbors_within(&self, drawing_id, depth) -> Vec<&PidObject>`，level-by-level BFS、`BTreeSet` 去重、自环跳过、`depth=0`→空、`depth=1`≡`neighbors_of`、循环安全（每对象至多访问一次）；5 个新单测覆盖 zero/one/two-hops/unreachable/cycle。`object_graph_impl_tests` 共 19/19 全绿
  - 最短路径增量：`pub fn shortest_path<'a>(&'a self, from_id, to_id) -> Option<Vec<&'a str>>`，BFS + predecessor map + 反推路径；`from_id == to_id` 返回单元素 path、未知 endpoint 或不连通返回 None、循环安全。5 个新单测覆盖 zero-hop/direct/multi-hop/unreachable/unknown_endpoint。`object_graph_impl_tests` 共 24/24 全绿
  - `tests/parse_real_files.rs::relationship_endpoints_resolve_via_sheet_record` 重构：从手写 `.iter().filter().count()` 双段改为 `endpoint_resolution_stats()` 一次调用，减少噪音
- 测试：`writer::metadata_helpers::tests` 由 18 个增至 **29 个**全绿（新增 11 个：3 个 `get_drawing_attribute_*` + 3 个 `get_general_element_text_*` + 5 个 `list_*`）

## [0.4.0] - 2026-04-19

### Phase 8: Writer 层落地（Package + WritePlan + CFB 回写）

在 parser-only 结构之上引入 **package 层**（保留原始 stream 字节）和 **writer 层**（按写计划重发 CFB），实现 passthrough round-trip 与 metadata-only 更新；Sheet 字节级修补以 `experimental` 形式入模。

- **`src/package.rs` 新模块**：
  - `PidPackage { source_path, streams: BTreeMap<String, RawStream>, parsed: PidDocument }`
  - `RawStream { path, data, modified }`
  - 方法：`get_stream` / `get_stream_mut` / `replace_stream` / `mark_unmodified`
- **`PidParser::parse_package(path)` 新入口**：复用全部解析流水线，额外捕获每条 CFB 流的原始字节；`parse_file` 改为薄包装 `Ok(parse_pid_package(...).parsed)`，行为完全等价。
- **`src/cfb/reader.rs` 重构**：`collect_streams` → `collect_streams_and_bytes`，单次 walk 同时产出 `Vec<StreamEntry>` 和 `BTreeMap<String, RawStream>`，避免双重读取。
- **`src/writer/` 新模块**（`mod.rs` / `plan.rs` / `metadata_write.rs` / `sheet_patch.rs` / `cfb_write.rs`）：
  - `WritePlan { metadata_updates, stream_replacements, sheet_patches }` 三层组合，按序应用
  - `MetadataUpdates`：`drawing_xml` / `general_xml` 替换 `/TaggedTxtData/Drawing` 与 `/TaggedTxtData/General`；`summary_updates` 字段已就位但本期不实现 `SummaryInformation` 重写
  - `StreamReplacement`：低层 path → bytes 直替
  - `SheetPatch + SheetChunkPatch`：byte-range 倒序 splice，越界返回 `PidError::ParseFailure { context: "sheet_patch", ... }`
  - `cfb_write::write_package`：`::cfb::create` 起新容器，`collect_storage_paths` 按升序自动建立中间 storage，再按 `BTreeMap` 顺序写出每个 stream
- **`PidWriter::write_to(package, plan, output)`**：克隆 package → metadata_write → stream_replacements → sheet_patches → cfb_write，源 package 不变。
- **`lib.rs`**：补上 `pub mod package; pub mod schema; pub mod writer;`（schema 模块 v0.3.1 已实现但未在 lib.rs 暴露，本次顺手挂上）。

### 测试

- **lib 单元测试 75 通过**（62→75）：
  - `package`：3 个（insert/overwrite/mark_unmodified）
  - `writer::sheet_patch`：5 个（同长度 splice / 倒序多 patch / 增长型 patch / 越界 / 区间反转）
  - `writer::cfb_write`：2 个（父 storage 收集 / 根流无 storage）
  - `schema`：3 个（v0.3.1 既有，正式登记）
- **集成测试 `tests/writer_roundtrip.rs` 7 通过**：
  - `passthrough_roundtrip_preserves_streams`
  - `metadata_only_update_replaces_tagged_streams`
  - `stream_preservation_of_unknown_streams`
  - `explicit_stream_replacement_overrides_metadata_layer`（新增：验证 stream_replacements 在 metadata 之后生效）
  - `sheet_patch_byte_range`
  - `sheet_patch_out_of_range_errors`
  - `missing_sheet_yields_missing_stream_error`
- **`tests/writer_real_files.rs`** 条件性 smoke：本地有 `test-file/DWG-0201GP06-01.pid` 时 round-trip 真实文件并按流逐字节比较；缺失时 `eprintln!` + return（与 `parse_real_files.rs` 同约定）。
- 所有既有 lib 测试与 release 构建通过。

### 公共 API 新增面

- 新增类型：`PidPackage` / `RawStream` / `WritePlan` / `MetadataUpdates` / `StreamReplacement` / `SheetPatch` / `SheetChunkPatch` / `PidWriter`
- 新增方法：`PidParser::parse_package(path) -> Result<PidPackage>`
- `PidDocument`、`PidParser::parse_file` 行为不变；`PidError` 不变（继续复用 `Io` / `MissingStream` / `ParseFailure`）。

### 已知限制

- CFB 重建不复刻原文件 CLSID、storage 创建/修改时间和物理 sector 顺序；内容视图可保真（每条流字节按 path 一致），字节级整文件 diff 不会一致。
- `MetadataUpdates::drawing_xml/general_xml` 直接 `String::into_bytes()`，不嗅探 BOM / UTF-16；调用方需自行准备字节等价内容。
- Sheet patch 仅 byte-range，不对接语义 probe；未在 CLI 接线。

## [0.3.1] - 2026-04-19

### Phase 7b: JSON Schema 导出

- **`schemars` 依赖**（`v1.2.1`，`preserve_order` feature）：为 `PidDocument` 及其所有子类型添加 `#[derive(JsonSchema)]`，覆盖 model 中全部 `Serialize/Deserialize` 结构体与枚举
- **`src/schema.rs` 新模块**：
  - `pid_document_schema() -> Schema`：返回 `PidDocument` 的 JSON Schema 对象
  - `pid_document_schema_pretty() -> Result<String, _>`：便捷包装，直接产出 pretty-printed JSON Schema 文本
  - 3 个单元测试：序列化合法性 / 核心类型名出现 / `AttributeValue` 变体定义
- **CLI `--schema` 出口**（复用已有 `pid_inspect` 入口）：下游消费方可通过 `pid_inspect --schema` 获取 JSON Schema，用于 TypeScript / Python / C# 代码生成（quicktype / json-schema-to-typescript / NJsonSchema）
- **`docs/writer-layer-plan.md`**：新增 Package / Writer 层落地计划文档（不含代码实现，仅规划）

### 测试

- schema 模块 3 个单元测试全通过
- 所有既有 lib 测试继续通过

## [0.3.0] - 2026-04-18

### Phase 7a: Mermaid 可视化导出

- **`inspect/mermaid.rs` 新模块**：纯函数把 `ObjectGraph` 和 `CrossReferenceGraph` 渲染为 mermaid 文本
  - `object_graph_mermaid(doc)` / `object_graph_mermaid_with(doc, opts)`：对象图（objects + relationships），按 `item_type` 着色、`drawing_id` 截短、`off-drawing` 端点自动占位；默认过滤模板关系（`guid` 为空）
  - `crossref_mermaid(doc)` / `crossref_mermaid_with(doc, opts)`：交叉引用图，四个 subgraph（Cluster Coverage / Symbol Usage / Attribute Classes / PSMroots→CFB Tree），缺失与异常用 `missing` / `extra` 颜色高亮
- **CLI 扩展**：
  - `pid_inspect --graph-mermaid`：stdout 输出对象图 mermaid（可直接贴到 Mermaid Live Editor / Obsidian / Notion）
  - `pid_inspect --crossref-mermaid`：stdout 输出交叉引用图 mermaid
- **渲染容量控制**：`ObjectGraphOptions { max_nodes=200, max_edges=500, skip_template_relationships=true }` 和 `CrossRefOptions { max_symbols=20, max_jsites_per_symbol=6 }`，超出用 `... (N more)` 占位保持 mermaid 可解析

### 模型

- 纯派生层，无新字段，仅新增导出工具

### 测试

- `inspect::mermaid` 8 个单元测试：空文档返回空 / 节点&边渲染 / off-drawing 占位 / 模板关系过滤 / 四个 subgraph 全都输出 / `sanitize` 规范化 / `escape_mermaid` 转义 / max_nodes 溢出
- 所有 lib 测试 **62 通过**（53→62），release 构建通过

### 版本收敛

从 `0.3.0-rc1`（关系端点解码）+ `0.3.0-rc2`（跨引用对象图）合并为正式 `0.3.0`，三件事（关系边、跨引用统计、可视化）一起构成 Phase 6 + 7a 的闭环交付。

## [0.3.0-rc2] - 2026-04-18

### Phase 6c: 跨引用对象图（基于 rc1 关系端点解码继续演进）

在 v0.3.0-rc1（Phase 6 关系端点解码，`source`/`target` 可用）之上新增**派生层**，把已解码的数据结构对齐成关系视图。

- **`CrossReferenceGraph`**：在已解码的 `PidDocument` 之上生成关系视图，纯内存派生、无额外 IO。四个子视图：
  - `ClusterCoverage`：把 `PSMclustertable` 声明的 cluster 与实际发现的 cluster/sheet 流做对齐，输出 `matched` / `declared_missing` / `found_extra` 三集合，数据完整性一目了然
  - `SymbolUsage`：按 `symbol_path` 反向索引 JSite 实例，暴露"一个符号被哪几个 JSite 引用"
  - `AttributeClassSummary`：每个 DA `class_name` 下的记录数 / 出现过的属性名集合 / 涉及的 `DrawingID` / `ModelID`（后者截断到 32）
  - `RootPresence`：把 `PSMroots` 中每条根名和 CFB 顶层目录条目对齐，标记 `STORAGE` / `STREAM` / `MISSING`

### 新模块

- `src/crossref.rs`：纯函数 `build_graph(doc) -> CrossReferenceGraph`，6 个单元测试覆盖所有四个子视图 + 空文档 + 缺失 PSM 降级

### 模型扩展

- 新类型：`CrossReferenceGraph` / `ClusterCoverage` / `SymbolUsage` / `AttributeClassSummary` / `RootPresence`
- `PidDocument` 新增可选字段 `cross_reference`；在 pipeline 末尾（`build_object_inventory` / `build_object_graph` 之后）生成

### 报告 & CLI

- 主报告新增 `--- Cross Reference ---` 段：cluster 覆盖率 / 符号用量 Top 5 / 每个属性类一行摘要 / PSMroots 解析状态
- `pid_inspect --crossref`：交叉引用详细视图（所有符号 + 所有属性类 + 全部 root 状态）

### 与 v0.3.0-rc1 的关系

rc1（关系端点解码）解决了**图的边**（`source_drawing_id` / `target_drawing_id` via sheet endpoint record 间接引用），rc2（本次）负责**图的上层统计视图与数据完整性检查**。两者互补：rc1 是底层关系解码，rc2 是跨层索引和对齐检查。

## [0.3.0] - 2026-04-18

### Phase 6: 关系端点解码（`source`/`target` 可用！）

- **核心突破**：破译 `/Unclustered Dynamic Attributes` 的**每条 P&IDAttributes 记录统一 31 字节 trailer**：
  ```
  89 00 <u32 size> <u32 record_id> [0x00 × 8] <u32 field_x> FF FF <u32 class_id> 14 00 00
  ```
  - `class_id=0xF6` 为关系记录，`0x109` 为 Symbol/Nozzle，`0xEA` 为 Drawing 等
  - 关系的 `field_x` **单调 +2 递增**，暗示为端点对表索引
- **Sheet 端点记录结构破译**（Sheet6 流里）：
  ```
  +0 u32 rel_field_x   +4 u32=0x06   +8 [u8;6]=0  +14 u16=0x0002
  +16 u32 endpoint_a    +20 u16=0x01  +22 u32 endpoint_b
  ```
  每条关系在 Sheet 流里有恰好 1 条此类记录，`endpoint_a/b` 指向对象的 `field_x`
- **端到端端点解析**：`PidRelationship` 新增 `source_drawing_id` / `target_drawing_id`，样本 1 实测 55/64 完全解析、9 partial（跨图 OPC）、0 未解析
- **证伪假设**：之前的推测"端点是相邻 GUID"被 `probe_sheet_endpoints` 证伪——对象 GUID 在全 CFB（69 流 × raw+Windows 布局）只以 ASCII 形式出现一次，证明端点采用**紧凑 field_x 索引间接引用**

### 模型扩展

- `DaRecordTrailer`：新结构（record_id / field_x / class_id / drawing_id / relationship_guid）
- `SheetEndpointRecord`：新结构（rel_field_x / endpoint_a / endpoint_b）
- `PidRelationship` / `PidObject` 新增 `record_id` / `field_x`；`PidRelationship` 新增 `source_drawing_id` / `target_drawing_id`
- `DynamicAttributesBlob.record_trailers` / `SheetStream.endpoint_records` 新字段
- `DocVersion2Raw`：DocVersion2 流原始保留（size / magic / hex_preview）
- `AttributeField.raw_value`：值审计链，保存 `strip_value_prefix` 剥离前的原始值

### 新模块

- `parsers/sheet_endpoint_records.rs`：Sheet 端点记录解析器 + 6 个单元测试
- `parsers/relationship_probe.rs`：关系记录邻近字节探针 + 4 个单元测试
- `examples/probe_*`（5 个）：RE 过程探针工具，保留为文档

### 报告与 CLI

- 报告 `--- Object Graph ---` 新增 "Endpoint resolution" 统计行和端点对显示
- `pid_inspect --probe-endpoints` 打印每条关系的 source/target drawing_id 与对象类型
- `pid_inspect --probe-relationships` 打印 `Relationship.<GUID>` 邻近字节证据

### 测试

- 单元测试：`sheet_endpoint_records` 6 个、`dynamic_attr_records` 新增 trailer 提取测试
- 集成测试新增：`record_trailers_cover_every_pidattributes_record` / `relationship_endpoints_resolve_via_sheet_record` / `sheet_endpoint_records_one_per_relationship` / `doc_version2_preserved_raw` / `object_graph_has_objects_and_relationships` 等
- **总计 91 个测试通过**（47 单元 + 26 集成 + 18 模块内）

## [0.2.4] - 2026-04-17

### Phase 5b: 文档注册表类流解析

- **`DocVersion3` 版本日志**：固定 48 字节/记录格式 `[product 16B][version 12B][op 4B][timestamp 16B]` 完全解出，样本 4 条版本历史（SA→SV→SV→SV，时间戳 12/29/25 → 03/16/26，版本 0144 ↔ 0077 来回切换）
- **`AppObject` COM 注册表**：每条 `[CLSID 16B][u32 char_count][UTF-16LE path]` + 3B filler；5 个 COM 插件 CLSID/路径完整解出（`igrSmartLabel.dll` / `igrGluePnt.dll` / `igrConnector.dll` / `LineRn.dll` 等）
- **`JTaggedTxtStgList`**：格式 `[list_name utf16-ascii run][u32 count][记录×count]`，每记录 `[u32 char_count][UTF-16LE storage_name]`；揭示 `TaggedTxtStorages → TaggedTxtData` 的映射
- **关键细节**：
  - `AppObject` 的长度字段是**字符数**（含 L'\0'）而非字节数
  - `JTaggedTxtStgList` 的 `list_name` 无 L'\0' 终止符，靠 u32 count 低字节 `0x01` 天然分界
  - CLSID 按 Microsoft 经典 COM 二进制布局解析（前三段 LE，后两段 BE），渲染为 `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` 标准形式

### 模型扩展

- 新类型：`VersionHistory` / `VersionRecord` / `AppObjectRegistry` / `AppObjectEntry` / `TaggedTextStorageList` / `TaggedTextStorageEntry`
- `PidDocument` 新增三个可选字段：`version_history` / `app_object_registry` / `tagged_storages`

### 新模块

- `parsers/doc_version.rs`（DocVersion3 解析器 + 4 个单元测试）
- `parsers/app_object.rs`（AppObject 解析器 + 4 个单元测试，含 GUID 格式化校验）
- `parsers/tagged_stg_list.rs`（JTaggedTxtStgList 解析器 + 3 个单元测试）
- `streams/doc_registry.rs`（统一接入上述三种流到 pipeline）

### 报告

- 主报告新增三段：`--- Version History ---` / `--- App Object Registry ---` / `--- Tagged Text Storage List ---`
- 顶层未识别流仅剩 1 个：`DocVersion2` (48B, magic=0x00010034, 二进制非文本)

### 测试

- 集成测试 +4：`version_history_decoded` / `app_object_registry_decoded` / `tagged_storage_list_decoded` + 之前已有的 PSM 三项
- **总计 56 个测试通过**（17 集成 + 18 `unit_parsers` + 21 模块内）

## [0.2.3] - 2026-04-17

### Phase 5a: PSM 索引表解析

- **`PSMroots` 完整解码**：确认格式 `[u32 magic='root']` + N 条 `[u32 id][u32 char_count][UTF-16LE name]` 记录；样本文件 7 条记录全部解出（`Imagineer Document` / `Server Document` / `_SupportOnlyList` / `TopVFSet` / `Dynamic Attributes Set Table` / `StyleLibrarian` / `DocStore`）
- **`PSMclustertable` 名称提取**：声明计数 `count=5`，样本 5 个 cluster 名称全部识别（`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` / `Sheet6` / `Unclustered Dynamic Attributes`）——这是 P&ID 文件中所有 cluster 风格流的**权威清单**
- **`PSMsegmenttable` 解码**：固定 12 字节格式 `[magic='stab'][u32 count][u8×count flags]`
- **揭示 Sheet 归属**：PSMclustertable 将 `Sheet6` 与其他 cluster 并列，证实 Sheet 流属于 cluster 体系（和 magic `0x6C90F544` 的推测一致）

### 模型扩展

- 新增类型：`PsmRoots` / `PsmRootEntry` / `PsmClusterTable` / `PsmClusterEntry` / `PsmSegmentTable`
- `PidDocument` 新增三个可选字段：`psm_roots` / `psm_cluster_table` / `psm_segment_table`

### 新模块

- `parsers/psm_tables.rs`：`parse_psm_roots` / `parse_psm_cluster_table` / `parse_psm_segment_table`，含 6 个内置单元测试
- `streams/psm_tables.rs`：接入主解析 pipeline（容错：流缺失时跳过）
- `examples/psm_dump.rs`：PSM 流 hex dump + 结构化 walk 开发工具

### 报告

- 主报告新增三段：`--- PSMroots ---`、`--- PSMclustertable ---`、`--- PSMsegmenttable ---`
- 顶层未识别流数从 7 降到 4（剩 `AppObject` / `DocVersion2` / `DocVersion3` / `JTaggedTxtStgList`）

### 测试

- 单元测试：`parsers::psm_tables` 6 个（roots/cluster/segment 各含正/负用例）
- 集成测试 +3：`psm_roots_extracts_known_entries` / `psm_cluster_table_matches_actual_clusters` / `psm_segment_table_decoded`
- **总计 42 个测试通过**（14 集成 + 18 `unit_parsers` + 10 模块内）

## [0.2.2] - 2026-04-17

### Phase 4: Sheet 流专项 + Magic 识别

- **Sheet 流结构化**：确认 `Sheet*` 流与 Cluster 共享 `magic 0x6C90F544`，复用 `cluster_header::parse_header()` 解析公共头（样本中 `Sheet6` 解析出 type=0x00CE / records=354 / body=121）
- **Sheet `ProbeSummary`**：对每个 Sheet 流扫描 0x89 标记并记录 body_start / marker_count / bytes_scanned（实测 Sheet 流 marker_count=0，确认 Sheet 不使用 DA 记录格式）
- **Sheet 属性记录探测**：尝试复用 `dynamic_attr_records::parse_attribute_records()`，若记录不为空则以 `confidence="heuristic"` 保留（当前样本未解出记录，为后续 Sheet 专用格式做铺垫）
- **`parsers/magic.rs` 新工具**：
  - `magic_tag(u32) -> Option<String>` 将 `magic_u32_le` 渲染为按磁盘字节顺序的 4 字符 ASCII（仅当全部可打印时返回）
  - `describe_magic(u32) -> &'static str` 为已知 magic（root/clst/stab/Smar/OLES/...）提供人类可读说明
- **未识别顶层流可见化**：报告中新增 `--- Top-level Unidentified Streams ---` 段，样本中揭示 7 个以往被忽略的结构化流：`PSMroots` (root)、`PSMclustertable` (clst)、`PSMsegmenttable` (stab)、`DocVersion3` (Smar)、`AppObject`、`DocVersion2`、`JTaggedTxtStgList`

### 模型扩展

- `SheetStream` 新增字段：`magic_u32_le` / `magic_tag` / `header` / `attribute_records` / `probe_summary`
- `UnknownStream` 新增字段：`magic_tag`

### CLI 增强

- `pid_inspect --probe-sheet`：Sheet 流专项探测输出（magic / header / probe / records / ASCII preview）

### 测试

- 新增 4 个单元测试：`magic_tag` × 2 / `describe_magic` / `sheet_stream_reuses_cluster_header`
- `parsers::magic` 内置 3 个单元测试
- 总计 32 个测试通过（11 集成 + 18 `unit_parsers.rs` + 3 模块内）

## [0.2.1] - 2026-04-17

### 正确性修复

- **`parse_header()` 边界安全**：最小长度判断从 14 修正为 16 字节，防止读取 `flags` 时越界
- **`parse_string_table()` sentinel 处理**：区分真正的 sentinel（index==0, byte_len==0）和合法空字符串条目（index!=0, byte_len==0），不再错误截断表
- **`DrawingMeta` SP_ 前缀兼容**：`RulesUID` / `FormatsUID` / `GappingUID` / `SymbologyUID` / `DefaultFormatsUID` 同时支持纯键名和 `SP_` 前缀键名

### Probe / Decode 分层

- **`AttributeRecord.confidence`**：每条 DA 记录携带 `"heuristic"` / `"decoded"` 置信度标记
- **`ProbeSummary`**：DA 流启发式扫描元数据（body_start_offset / marker_count / records_extracted / bytes_scanned）
- **`ClusterProbeInfo`**：PSMcluster0 字符串表定位元数据（string_table_offset / detection_method / entries_parsed / end_offset）
- **`report.rs` 标注**：报告中 DA 记录标注 `[EXPERIMENTAL/heuristic]`，Cluster 和 DA 输出 `[PROBE]` 行

### 测试

- 新增 14 个单元测试：`collect_simple_tags` (5) / `parse_header` (5) / `parse_string_table` (4)
- 全部 25 个测试通过（11 集成 + 14 单元）

### CLI 增强

- `pid_inspect --probe-cluster`：输出 Cluster 流探测详情（偏移量、检测方法、字符串表完整内容）
- `pid_inspect --probe-dynamic`：输出 DA 流探测详情（0x89 标记数、记录统计、属性字段详情）

### 文档

- **ARCHITECTURE.md** 全面重写：4 张 Mermaid 架构图（分层架构 / .pid 文件结构 / 数据流 / Probe-Decode 分层）、类型表、CLI 用法、演进路线

## [0.2.0] - 2026-04-16

### 新增 (Phase 4: 对象清单与精度修复)

- **P&ID 对象清单** (`ObjectInventory`)：从 DA 属性记录中自动构建 P&ID 对象统计——管道运行、仪表、管嘴、设备、关系等分类计数
- **DA 值解析精度提升**：double 优先检测（OLE Date 正确识别）、GUID 前缀保护（32 位 hex 不被误剥离）、单字节类型标记跳过

### 新增 (Phase 2-3: 语义提取与二进制记录切分)

- **OLE Summary 解析**：实现 `\x05SummaryInformation` 和 `\x05DocumentSummaryInformation` 流的二进制解码，支持 VT_LPSTR / VT_LPWSTR / VT_FILETIME / VT_I4 类型，提取应用名称、标题、作者、创建/修改时间等元数据
- **GUID 扫描**：在 `string_scan` 中新增双模式 GUID 提取——文本格式 `{XXXXXXXX-...}` 和原始 16 字节 LE 格式；`JProperties` 解析自动调用，测试文件提取 706 个 GUID
- **Cluster 公共头解析器** (`cluster_header.rs`)：解析所有 cluster 流共享的 magic `0x6C90F544` + type / record_count / body_len 字段
- **PSMcluster0 字符串表**：反向定位 entry1，从 PSMcluster0 中提取索引字符串表（SiteObjects, PreferenceSet, Sheets）
- **动态属性记录解码器** (`dynamic_attr_records.rs`)：基于 `0x0089` type marker 的记录边界检测，解析出属性类名 + 名称 / 值对，测试文件提取 231 条记录 / 10 个唯一类 / 1120+ 属性字段
- **结构化模型类型**：`ClusterHeader`、`IndexedString`、`AttributeRecord`、`AttributeField`、`AttributeValue`
- **inspect 报告增强**：输出 Summary 信息、JSite GUID 计数、Cluster header 详情、字符串表、属性记录摘要

### 修复

- `dynamic_attrs.rs` 中 `strings` 和 `class_names` 的重复问题，使用 `HashSet` 消除 ASCII + UTF-16LE 合并扫描中的重复项
- XML 解析器嵌套标签跳过导致 Drawing/General Meta 全空的 bug（MCP-4 修复）
- Symbol path 乱码前缀通过 UNC 路径提取清理（MCP-4 修复）
- 编译错误 3 个 + 逻辑 bug 4 个（MCP-4 修复）

### 改进

- `pid_inspect` 支持 `--json` 输出完整 `PidDocument` 的 JSON 序列化
- 集成测试 11 个用例全部通过

## [0.1.0] - 2026-04-16

### 初始版本

- CFBF/OLE 容器遍历与流索引
- `TaggedTxtData/Drawing` 和 `TaggedTxtData/General` XML 元数据提取
- `JSite*` 对象存储索引与 JProperties 解析
- Cluster 流分类（PSMcluster, StyleCluster, Dynamic Attributes）
- Unclustered Dynamic Attributes 字符串扫描（ASCII + UTF-16LE）
- `pid_inspect` CLI 工具
