# 变更日志

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
