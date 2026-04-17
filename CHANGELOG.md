# 变更日志

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
