# PID 文件格式分析说明（当前实现口径）

> 日期：2026-06-03  
> 范围：`pid-parse` 对 SmartPlant / Smart P&ID `.pid` 文件的当前解析认知。  
> 口径：这是“当前实现说明 + 证据等级”，不是 SmartPlant `.pid` 官方规范。  

## 1. 总体结构

`.pid` 文件在当前实现中被视为 **OLE / CFBF（Compound File Binary Format）复合文档**。外层是一个 CFB 容器，内部由 storage 与 stream 组成。

读取入口：

- `PidParser::parse_file(path)`：返回结构化 `PidDocument`。
- `PidParser::parse_package(path)`：返回 `PidPackage`，同时保留所有 raw stream bytes，用于 writer、diff、round-trip 与 byte-audit。
- `PidPackage::from_bytes(bytes)`：从内存字节解析 `.pid`。

主读取路径：

1. `cfb::open` 打开容器。
2. `cfb::tree::build_tree` 构建 storage/stream 树。
3. 收集所有 stream 的路径、大小、magic、预览与 raw bytes。
4. `streams::*` 分发解析 summary、TaggedTxtData、JSite、cluster、dynamic attributes、PSM tables、doc registry、Sheet endpoint。
5. 派生 object inventory、object graph、cross reference、layout、normalized geometry。

## 2. 证据等级

本文所有格式说明都按以下等级表达：

| 等级 | 含义 |
|---|---|
| `Decoded` | 字节布局与字段语义相对稳定，已进入 typed model 或可作为稳定消费面 |
| `PartiallyDecoded` | 已解析关键结构，但仍有未命名字段、record body 或业务语义缺口 |
| `IdentifiedOnly` | storage / stream 已识别，但结构化字段不足 |
| `Probed` | 启发式或调查性证据，只能辅助逆向，不能作为稳定格式合同 |
| `Leftover` | byte-audit 中未被 parser 声明消费的字节 |
| `AuditOnly` | 字节结构可收集与回归，但刻意不命名业务语义、不进入渲染实体 |

## 3. 顶层 Stream / Storage 地图

### 3.1 已知顶层 stream

| 顶层名 | 当前状态 | 主要 parser / 模型 |
|---|---|---|
| `\x05SummaryInformation` | `Decoded` | `streams::summary` → `PidDocument.summary` |
| `\x05DocumentSummaryInformation` | `Decoded` | `streams::summary` → `PidDocument.summary` |
| `PSMcluster0` | `PartiallyDecoded` | `streams::cluster` → `PidDocument.clusters` |
| `StyleCluster` | `PartiallyDecoded` | `streams::cluster` → `PidDocument.clusters` |
| `Dynamic Attributes Metadata` | `PartiallyDecoded` | cluster header / metadata evidence |
| `Unclustered Dynamic Attributes` | `PartiallyDecoded` | `streams::dynamic_attrs` → `PidDocument.dynamic_attributes` |
| `PSMroots` | `Decoded` | `streams::psm_tables` → `PidDocument.psm_roots` |
| `PSMclustertable` | `PartiallyDecoded` | `streams::psm_tables` → `PidDocument.psm_cluster_table` |
| `PSMsegmenttable` | `PartiallyDecoded` | `streams::psm_tables` → `PidDocument.psm_segment_table` |
| `DocVersion2` | `Decoded` | `parsers::doc_version2` → `PidDocument.doc_version2_decoded` |
| `DocVersion3` | `Decoded` | `parsers::doc_version` → `PidDocument.version_history` |
| `AppObject` | `Decoded` | `parsers::app_object` → `PidDocument.app_object_registry` |
| `JTaggedTxtStgList` | `Decoded` | `parsers::tagged_stg_list` → `PidDocument.tagged_storages` |

### 3.2 已知 storage prefix

| Prefix | 当前状态 | 说明 |
|---|---|---|
| `TaggedTxtData` | `IdentifiedOnly` storage；子 stream 可 decoded | `/TaggedTxtData/Drawing`、`/TaggedTxtData/General` 是 XML 元数据 |
| `JSite*` | `IdentifiedOnly` storage；子 stream partial/probed | 符号实例、`JProperties`、OLE payload 等 |
| `Sheet*` | 混合：decoded + audit-only + probe | 图面数据主体；部分 PSM/IGDS record 已 typed decode，但仍有未知 family |
| `PSMspacemap` | `IdentifiedOnly` | 识别到 `tseg` magic 相关页面，暂无完整结构化 parser |

## 4. Magic / Signature 对照

| 值 | ASCII / 名称 | 当前解释 |
|---|---|---|
| `0x6C90_F544` | 非 printable | cluster-family 公共 magic，见于 `PSMcluster0`、`StyleCluster`、DA metadata、`Sheet*` |
| `0x746F_6F72` | `root` | `PSMroots` |
| `0x7473_6C63` | `clst` | `PSMclustertable` |
| `0x6261_7473` | `stab` | `PSMsegmenttable` |
| `0x7261_6D53` | `Smar` | `DocVersion` / SmartPlant 相关 |
| `0x6D78_3F3C` | `<?xm` | XML declaration |
| `0x6765_7374` | `tseg` | `PSMspacemap` segment table 类 evidence |
| `0x5345_4C4F` | `OLES` | OLE storage block evidence |

## 5. 主要 Stream 格式

### 5.1 SummaryInformation / DocumentSummaryInformation

- 类型：OLE Property Set。
- 状态：`Decoded`。
- 输出：`PidDocument.summary`。
- Writer：支持部分 Summary property 创建、更新、删除；未触碰字段保持 passthrough。
- 限制：DocumentSummary section 2 等仍需谨慎对待，writer 不应假设所有 property set 子结构都可重写。

### 5.2 TaggedTxtData/Drawing 与 TaggedTxtData/General

- 类型：XML metadata stream。
- 状态：`Decoded`。
- 路径：
  - `/TaggedTxtData/Drawing`
  - `/TaggedTxtData/General`
- 输出：
  - `PidDocument.drawing_meta`
  - `PidDocument.general_meta`
- 已知内容：图号、模板、项目、文件路径、模板/规则/格式相关 tag。
- Writer：支持 XML 替换、单 tag 编辑、drawing number 编辑。

### 5.3 JTaggedTxtStgList

- 类型：UTF-16LE storage list 索引。
- 状态：`Decoded`。
- 观察布局：
  - `list_name`：UTF-16LE printable ASCII run。
  - `u32 count`。
  - 每条 entry：`u32 char_count` + UTF-16LE `storage_name`，通常包含结尾 `L'\0'`。
- 输出：`PidDocument.tagged_storages`。

### 5.4 AppObject

- 类型：COM / DLL 插件注册表。
- 状态：`Decoded`。
- 观察布局：
  - `u32 leading value`，观察值常为 `5`，不强称为 entry count。
  - 多条可变长记录：
    - 16 字节 COM CLSID。
    - `u32 path_char_count`。
    - UTF-16LE path。
    - 1-3 字节 filler / resync 区域，按 `Probed` 处理。
- 输出：`PidDocument.app_object_registry`。

### 5.5 DocVersion2

- 类型：紧凑版本日志。
- 状态：`Decoded`。
- 布局：
  - 12 字节 header：
    - `u32 magic = 0x0001_0034`。
    - 8 字节 reserved，观察为 0，按 `Probed` 记录。
  - N 条 9 字节记录：
    - `u8 op_type`：`0x82 = SaveAs`，`0x81 = Save`。
    - 3 字节 fixed，观察为 `00 00 09`，按 `Probed`。
    - 1 字节 separator。
    - `u32 version`。
- 输出：`PidDocument.doc_version2_decoded`。

### 5.6 DocVersion3

- 类型：人类可读版本历史日志。
- 状态：`Decoded`。
- 布局：48 字节定长记录数组。
  - `+00..+0F`：product，zero-padded ASCII。
  - `+10..+1B`：version string。
  - `+1C..+1F`：operation code。
  - `+20..+2F`：timestamp。
- 输出：`PidDocument.version_history`。

### 5.7 PSMroots

- 类型：PSM root table。
- 状态：`Decoded`。
- 布局：
  - `u32 magic = 'root'`。
  - N 条记录：`u32 id` + `u32 char_count` + UTF-16LE name。
  - `id=0, char_count=0` 可作为 sentinel。
- 输出：`PidDocument.psm_roots`。

### 5.8 PSMclustertable

- 类型：cluster 权威清单 / 索引。
- 状态：`PartiallyDecoded`。
- 布局：
  - `u32 magic = 'clst'`。
  - `u32 count`。
  - 后续为可变长记录；当前通过 UTF-16LE ASCII name run 定位记录边界。
- 当前可用字段：
  - cluster / sheet name。
  - record offset / len。
  - prefix bytes。
  - decoded candidate fields。
- byte-audit 口径：
  - magic/count/name/null terminator 可 `Decoded`。
  - record prefix 与候选字段仍按 `Probed` / partial 看待。
  - trailing 未识别区为 `Leftover`。
- 限制：SmartPlant 字段语义尚未完全闭环。

### 5.9 PSMsegmenttable

- 类型：segment table / segment flags。
- 状态：`PartiallyDecoded`。
- 布局：
  - `u32 magic = 'stab'`。
  - `u32 count`。
  - `u8 × count` flag array。
- 当前可用字段：
  - segment index。
  - offset。
  - flag。
  - 当 segment 与 cluster entry 数量 1:1 时，保守填充 `candidate_owner_cluster_index/name`。
- 限制：flag 语义与 segment 类型仍未完全命名。

### 5.10 Cluster-family 公共头

适用：

- `PSMcluster0`
- `StyleCluster`
- `Dynamic Attributes Metadata`
- `Sheet*`

状态：公共头 `Decoded`，body 依 stream family 分层处理。

16 字节公共头：

```text
+00..+03  u32 magic = 0x6C90_F544
+04..+07  u32 record_count
+08..+09  u16 stream_type
+0A..+0D  u32 body_len
+0E..+0F  u16 flags
```

`PSMcluster0` 额外通过启发式定位 indexed UTF-16LE string table：

```text
u32 index
u32 byte_len
UTF-16LE payload
```

table 前的 locator 区域按 `Probed` 处理。

## 6. Dynamic Attributes 与关系证据链

### 6.1 Unclustered Dynamic Attributes

- 状态：`PartiallyDecoded`。
- 输出：`PidDocument.dynamic_attributes`。
- 当前能力：
  - 扫描 ASCII / UTF-16LE 字符串。
  - 提取 class name。
  - 提取 relationship-like tag。
  - 解析启发式 attribute record。
  - 提取 `P&IDAttributes` record landmarks。
  - 提取 `DrawingID`。
  - 提取 31 字节 per-record trailer。

### 6.2 DA 31 字节 Trailer

状态：trailer signature 与核心字段为 `Decoded`。

识别条件：

- `0x89 0x00` marker。
- 8 字节零 padding。
- `0xFFFF` separator。
- 尾部 `0x14 0x00 0x00`。

字段：

- `size`
- `record_id`
- `field_x`
- `class_id`
- `drawing_id`（从 record body 中的 `DrawingID\0<32hex>` 回看提取）
- `relationship_guid`（从 `Relationship.<GUID>` tag 与 `class_id=0xF6` trailer 配对）

### 6.3 Relationship Endpoint Record

路径：`Sheet*` stream。

状态：`Probed` / 结构签名稳定，但仍依赖 DA relationship `field_x` 进行严格解析。

26 字节 signature：

```text
+00  u32 rel_field_x
+04  u32 0x00000006
+08  [u8; 6] zero padding
+14  u16 0x0002
+16  u32 endpoint_a
+20  u16 0x0001
+22  u32 endpoint_b
```

用途：将 relationship 的 `field_x` 与两个 endpoint object `field_x` 连接起来。注意：这证明的是语义连接，不自动证明 CAD 坐标。

## 7. Sheet* 图面数据

`Sheet*` 是 `.pid` 格式中最复杂的区域。当前实现必须按 record family 分层说明，不能整体宣称“完整解码”。

### 7.1 已 typed decoded 的几何家族

当前 `AGENTS.md` 记录 Phase 14 decoder suite 已落地多类 PSM / IGDS record：

| PSM Type | Decoder | 几何类型 | 状态 |
|---|---|---|---|
| `0x3FE6` | `decode_primitive_lines` | `GLine2d` | `Decoded` |
| `0x0018` | `decode_iglines` | `igLine2d` | `Decoded` |
| `0x0084` | `decode_iglinestrings` | `igLineString2d` | `Decoded` |
| `0x005E` | `decode_igpoints` | `igPoint2d` | `Decoded` |
| `0x004D` | `decode_igtextboxes` | `igTextBox` | `Decoded` |
| `0x00CE` | `decode_igsymbols` | `igSymbol2d` | `Decoded` |
| `0x0030` | `decode_jstyle_overrides` | `JStyleOverride` / Annotation | `Decoded`，但部分 field 仍带 union/probe 解释 |

输出路径：

- `SheetGeometry::decoded_*`
- `geometry::build_normalized_geometry`
- `PidGraphicEntity { confidence: Decoded, ... }`

### 7.2 Audit-only 家族

| Type | 当前含义 | 状态 |
|---|---|---|
| `0x00FA` | `GraphicGroup` header + raw variable tail | `AuditOnly` |
| `0x0010` | polymorphic sub-record family | `AuditOnly` |

`0x0010` 约束：

- 当前仅命名 `leading_word` 这类字节位置字段。
- 不命名 `sub_kind`。
- 不创建 typed business DTO。
- 不引入 `PidGraphicKind`。
- 需要后续 IDA / runtime hook / controlled diff 证据才能升级。

### 7.3 Probe-only 证据

包括：

- Sheet chunk boundary。
- text runs。
- coordinate hints。
- field_x windows。
- text placement candidates。
- coordinate/page metadata top evidence。
- endpoint pair scan。

这些证据服务逆向和报告，不等于稳定几何格式。

### 7.4 Page Transform Guardrail

当前 `NormalizedPidGeometry.page_dimensions_mm` 只是模板推断出的页面尺寸证据，不等于 source-to-page transform。

`PidPageTransform::Available` 只有在同时具备以下证据时才能出现：

- source coordinate space。
- units。
- transform direction。
- origin / scale / bounds。
- bounded byte provenance。

Phase 24 已对 coordinate/page metadata 做 negative 收口：当前 cross-fixture `page_dimension_scalar_matches = 0`，不能 promotion page transform。

## 8. 下游模型映射

| 模型 | 角色 |
|---|---|
| `PidDocument.streams` | 所有 stream 的路径、大小、magic、preview |
| `PidPackage.streams` | raw bytes preservation，writer / diff / audit 事实源 |
| `PidDocument.summary` | OLE summary metadata |
| `drawing_meta` / `general_meta` | TaggedTxtData XML metadata |
| `jsites` | JSite storage 与符号路径 / JProperties 线索 |
| `clusters` | cluster-family stream header / string table / basic evidence |
| `dynamic_attributes` | P&IDAttributes、DA records、trailers、relationship evidence |
| `sheet_streams` | Sheet geometry / endpoint / probe evidence |
| `psm_roots` / `psm_cluster_table` / `psm_segment_table` | PSM 索引表 |
| `object_inventory` | 从 DA 派生的对象清单 |
| `object_graph` | 对象与关系图 |
| `cross_reference` | cluster / symbol / DA / sheet provenance 拼接 |
| `layout` | 拓扑推导的可读布局，不是 CAD 原始几何 |
| `NormalizedPidGeometry` | source-backed geometry projection |
| `PidImportView` | 下游导入视图，暴露 relationship endpoint provenance |

## 9. 验证方法

### 9.1 Coverage

查看顶层 stream/storage 解析状态：

```bash
cargo run --bin pid_inspect -- path/to/file.pid --coverage
cargo run --bin pid_inspect -- path/to/file.pid --coverage --json
```

coverage report 解决的问题是：哪些顶层结构已知、partial、identified-only 或 unknown。

### 9.2 Byte Audit

查看每个 stream 的 consumed / leftover：

```bash
cargo run --bin pid_inspect -- path/to/file.pid --byte-audit
cargo run --bin pid_inspect -- path/to/file.pid --byte-audit --json
```

byte-audit 解决的问题是：每个 parser 声明消费了哪些字节，哪些字节仍 leftover。

高 coverage ratio 不等于语义完整；`Probed` 字节仍可能只是定位证据。

### 9.3 Sheet Probe

查看 Sheet 调查证据：

```bash
cargo run --bin pid_inspect -- path/to/file.pid --probe-sheet
cargo run --bin pid_inspect -- path/to/file.pid --probe-sheet-chunks Sheet6
```

### 9.4 JSON / Schema

导出完整模型或 schema：

```bash
cargo run --bin pid_inspect -- path/to/file.pid --json
cargo run --bin pid_inspect -- path/to/file.pid --schema
```

### 9.5 Round-trip

验证 writer passthrough：

```bash
cargo run --bin pid_inspect -- path/to/file.pid --round-trip out.pid --verify
```

## 10. 当前未知区与下一步证据需求

| 区域 | 当前状态 | 下一步证据需求 |
|---|---|---|
| `PSMclustertable` record prefix | `Probed` / partial | 更多 fixture 横向字段稳定性、IDA 或 controlled diff |
| `PSMsegmenttable` flags | `Probed` / partial | segment 与 Sheet / relationship 的更多映射证据 |
| `PSMspacemap` | `IdentifiedOnly` | structural parser 与 byte-audit trace |
| `0x0010` sub-record family | `AuditOnly` | IDA-confirmed Read/DoIO、sub-kind discriminator、cross-fixture validation |
| `GraphicGroup` tail | `AuditOnly` | child reference / OID list 的来源证据 |
| coordinate/page metadata | `Probe` / negative evidence | 同 marker 跨 fixture kind 一致 + `page_dimension_scalar_matches > 0` |
| text probe placement | `Probe` | text payload 与 decoded igTextBox / DA label 的稳定映射 |
| page transform | unavailable | source coordinate space + units + transform direction + bounded provenance |

## 11. 文档维护规则

新增格式说明或 parser 时，应同步检查：

1. `src/inspect/mod.rs` 是否需要登记 known stream/storage。
2. `src/inspect/coverage.rs` 状态是否准确。
3. `src/byte_audit/aggregate.rs` 是否需要注册 `_with_trace` parser。
4. `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md` 是否需要新增或调整对应条目。
5. 是否需要在 README 或 `docs/format-notes.md` 中更新入口。

任何 probe → decoded promotion 都必须有测试、fixture 或 IDA / controlled diff 证据支撑。

## 12. Phase 26-C 快照状态

本轮尝试为格式说明补充真实 fixture 的 coverage / byte-audit 快照，但当前工作树未发现可用 `.pid` 样本：

- `Glob(test-file/**/*.pid)`：0 个文件。
- `Glob(repo/**/*.pid)`：0 个文件。
- `git status --short --ignored -- test-file`：无 ignored fixture 输出。

因此本阶段未生成：

- `docs/analysis/2026-06-03-pid-format-coverage-snapshot.json`
- `docs/analysis/2026-06-03-pid-format-byte-audit-snapshot.json`

这不影响主文档的代码事实源说明；但若后续恢复 `test-file/DWG-0201GP06-01.pid`、`test-file/D06.pid` 或其它真实 fixture，应补跑：

```bash
cargo run --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --coverage --json
cargo run --bin pid_inspect -- test-file/DWG-0201GP06-01.pid --byte-audit --json
```

并把摘要写回本节。
