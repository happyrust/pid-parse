# pid-parse 解析现状与下一阶段 PRD

> 日期：2026-04-26  
> 范围：`pid-parse` 当前 `.pid` 文件解析、Backup/MDF offline publish 管线、writer/byte-audit 能力与后续产品化需求。  
> 目标读者：项目 owner、解析器开发者、下游 CAD/数据导入方、QA/CI 维护者。

## 1. 背景

`pid-parse` 是一个 Rust 实现的 SmartPlant / Smart P&ID 解析与导出工具集。当前项目已经不只是单纯读取 `.pid` 容器，而是形成了三条互补能力线：

1. **`.pid` 只读解析**：读取 OLE/CFBF 复合文档，枚举 stream/storage，解析元数据、对象、关系、符号、layout 线索，并输出报告/JSON/schema/mermaid。
2. **`.pid` round-trip writer**：以 `PidPackage` 保留所有原始 stream 字节，通过 `PidWriter` 做 metadata、summary、stream replacement、sheet byte patch 等有限写回。
3. **Backup/MDF publish XML 管线**：从 SmartPlant backup 中提取 `Export.mdf`，经 vendored `oxidized-mdf` 读取 SQL Server MDF 表，再生成 SmartPlant 兼容 `_Data.xml` / `_Meta.xml`。

当前工程重点已经从“能不能读出东西”推进到“能否证明解析覆盖率、保真度和 publish 输出语义稳定”。这也是本 PRD 的核心：把现状、缺口和下一阶段验收口径统一起来。

## 2. 当前解析机制

### 2.1 `.pid` 文件解析路径

主入口是 `PidParser`：

- `PidParser::parse_file(path)` 返回结构化 `PidDocument`。
- `PidParser::parse_package(path)` 返回 `PidPackage`，同时保留原始 stream 字节、root CLSID、storage CLSID、storage timestamp、state bits，用于回写和 diff。

读取流程：

1. `cfb::open(path)` 打开 OLE/CFBF 容器。
2. `cfb::tree::build_tree` 构建 storage/stream 层级树。
3. `collect_streams_and_bytes` 一次性收集：
   - `PidDocument.streams`：路径、大小、magic、ASCII preview。
   - `PidPackage.streams`：每个 stream 的原始 bytes。
4. `streams::*` 逐类解析并富化同一个 `PidDocument`：
   - summary / document summary。
   - tagged text XML。
   - JSite / JProperties。
   - cluster family。
   - dynamic attributes。
   - PSM roots / cluster table / segment table。
   - document registry / DocVersion。
   - Sheet endpoint records。
5. 后处理派生：
   - `object_inventory`：从 dynamic attrs 统计对象类型。
   - `object_graph`：对象与 relationship 图。
   - `cross_reference`：cluster/symbol/attribute/object/sheet provenance。
   - `layout`：layout-first 的可读整图模型。

### 2.2 Probe / Decode 分层

项目明确区分两类解析结果：

- **Decode**：字段语义稳定，可进入 typed model，可作为 writer/diff/下游消费依据。
- **Probe**：格式尚未完全逆向，但能稳定定位边界、文本、record hint、coordinate hint、endpoint hint，用于继续逆向和 byte-audit。

这套分层避免把启发式结果伪装成稳定语义，是当前解析器能够长期演进的关键约束。

### 2.3 Publish XML 管线

publish 路径独立于 `.pid` 容器读取，面向 SmartPlant backup/export 数据：

1. `pid_backup_extract` 从 `Export.dmp` 剥离 MTF envelope，得到 `Export.mdf`。
2. `publish::mdf_load::open_mdf_as_sqlite` 用 vendored `oxidized-mdf` 读取 MDF 中 publish 相关表，并暂存到 in-memory SQLite。
3. `publish::sqlite_load::load_drawing_graph` 复用 SQLite 查询层组装 `PublishDrawing` DTO：
   - drawing header。
   - representation。
   - relationship。
   - model item。
   - per item-type business subtables。
   - codelist。
   - PipeRun/SignalRun endpoint connection inference。
4. `publish::xml_writer::{write_data_xml, write_meta_xml}` 生成 `_Data.xml` / `_Meta.xml`。

当前 MDF 已是公开正确性基线；旧 `Export_v2.sqlite` 仅保留为 legacy compatibility 输入。

## 3. 已实现能力

### 3.1 `.pid` 解析能力

当前已稳定支持：

- CFB 容器枚举、树结构、stream raw bytes 保留。
- OLE Summary / DocumentSummary 信息读取。
- `TaggedTxtData/Drawing` 与 `TaggedTxtData/General` XML 解析。
- JSite 符号路径、GUID、JProperties 文本线索提取。
- `PSMroots` 完整解析。
- `PSMclustertable` / `PSMsegmenttable` 部分结构化解析。
- `PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` cluster header 与部分 string table/probe。
- `Unclustered Dynamic Attributes` 的属性记录、31 字节 trailer、class name、DrawingID landmark。
- `DocVersion2` / `DocVersion3`、`AppObject`、`JTaggedTxtStgList` 结构化解析。
- Sheet stream 的 header/probe、文本 run、endpoint record 线索。
- 对象清单、对象关系图、跨引用图、layout-first 可读模型。
- `pid_inspect` CLI 的文本报告、JSON、coverage、byte-audit、schema、diff、probe、mermaid 输出。

### 3.2 Coverage / Byte Audit

项目现在有两层覆盖视图：

- **stream-level coverage**：`inspect::coverage` 按 top-level node 分类为 `FullyDecoded` / `PartiallyDecoded` / `IdentifiedOnly` / `Unknown`，并会根据 `PidDocument` 实际字段是否填充做动态降级。
- **byte-level audit**：`byte_audit_report` 记录每个 raw stream 的 `consumed_bytes` / `leftover_bytes` / `overall_coverage_ratio`，用于 CI baseline 和逆向优先级排序。

当前 byte-audit 已注册的主要 stream family 包括：

- Summary / DocumentSummary property set。
- PSM roots / cluster table / segment table。
- DocVersion2 / DocVersion3。
- AppObject。
- JTaggedTxtStgList。
- TaggedTxtData Drawing/General XML。
- JProperties。
- PSMcluster0 / StyleCluster / Dynamic Attributes Metadata。
- Unclustered Dynamic Attributes landmark/trailer。
- Sheet text run 与 endpoint record 线索。

`PSMspacemap` 已在 Unreleased 中识别为 top-level storage prefix，不再被误报为 Unknown，但仍只是 `IdentifiedOnly`。

### 3.3 Writer / Round-trip

当前 writer 已支持：

- Drawing / General XML 替换。
- 单 tag XML 编辑。
- Summary section 1 的 create/update/delete。
- 任意 stream replacement。
- experimental Sheet byte-range patch。
- root/non-root CLSID 保留。
- storage timestamp/state bits 读取并参与 package diff。
- `--round-trip --verify` 与 `diff_packages` 对比。

writer 的基本策略是 passthrough-first：未知 stream 默认原样保留，只对声明修改的 stream 做替换。

### 3.4 Backup / MDF / Publish XML

当前 publish 能力已经进入较高成熟度：

- `Export.mdf` 可由 Rust MDF loader 直接读取，不再依赖 C# OrcaMDF 作为主链。
- vendored `oxidized-mdf` 已做 nom 8 迁移、同步 API 化、panic-free hardening。
- A01/TEST02 MDF 核心表可稳定 staged 到 SQLite。
- `_Data.xml` 和 `_Meta.xml` 都可生成。
- 已支持 15 类 PID tag：
  - `PIDBranchPoint`
  - `PIDControlSystemFunction`
  - `PIDDrawing`
  - `PIDNote`
  - `PIDNozzle`
  - `PIDPipeline`
  - `PIDPipingBranchPoint`
  - `PIDPipingComponent`
  - `PIDPipingPort`
  - `PIDProcessPoint`
  - `PIDProcessVessel`
  - `PIDRepresentation`
  - `PIDSignalConnector`
  - `PIDSignalPort`
  - 以及相关 derived rel/body 结构
- Publish fidelity gates 已覆盖：
  - tag count diff。
  - writer coverage。
  - interface parity。
  - attribute parity。
  - Rel DefUID parity。
  - Rel UID soundness。
  - A01/DWG style 切换。
  - `_Meta.xml` parity。
  - raw residual evidence probe。

## 4. 当前进度判断

### 4.1 总体结论

当前项目可视为：

- **`.pid` 文件解析：工程骨架成熟，关键 top-level stream 基本覆盖，但二进制深层语义仍处于 partial/probe 阶段。**
- **`.pid` writer：metadata 与 passthrough round-trip 可用，深层语义编辑未到产品化阶段。**
- **publish XML：A01 主链已接近可交付，DWG 侧还有 fixture 与 loader enrichment 依赖。**
- **验证体系：stream coverage 与 byte audit 已成型，正在把逆向过程从经验判断推进到可量化 baseline。**

### 4.2 已完成度分层

| 领域 | 当前阶段 | 判断 |
|---|---|---|
| CFB 容器读取 | 成熟 | 已可枚举树、stream、raw bytes、CLSID、timestamp、state bits |
| 顶层 stream 识别 | 成熟 | known stream/prefix registry 已建立，`PSMspacemap` 已纳入 |
| XML / Summary / registry | 成熟 | 多数是 FullyDecoded，可读可报告，部分可写 |
| Dynamic Attributes | 中高 | 对象/关系基础可用，record body 深层字段仍有 leftover |
| PSM tables | 中 | roots 完整，cluster/segment table 仍 partial |
| Sheet 几何 | 早中期 | endpoint/text/probe 可用，几何/图元未完整命名 |
| ObjectGraph/CrossRef/Layout | 中高 | 可供下游生成可读图，但来源仍混合 decoded/probed/inferred |
| Writer | 中高 | metadata 与 passthrough 稳定，SheetPatch experimental，语义级图元编辑未完成 |
| Byte Audit | 中高 | 框架和 CLI 已具备，仍需真实 baseline 与更多 parser trace |
| Publish XML A01 | 高 | 主链、writer、fidelity gates 基本闭环，仅剩少量 synthetic slot contract |
| Publish XML DWG | 中 | writer arms 已有，loader-side enrichment 与 fixture 验证未闭环 |

### 4.3 主要剩余缺口

1. **PSMclustertable / PSMsegmenttable 尚未 FullyDecoded**  
   已有 count/name/flags 等结构，但 per-record 字段语义、cluster/segment 关系和 layout/relationship 关联还不完整。

2. **Sheet 深层几何未完成**  
   当前能识别 text run、endpoint record、record type evidence、coordinate hint，但还不能稳定输出完整图元、坐标、文本、符号引用和标注模型。

3. **规范化语义图层尚未统一**  
   object、relationship、endpoint、symbol、cluster、sheet provenance 分布在 `object_graph`、`cross_reference`、`layout`、probe 输出等多个视图里。下游可用，但不是一个统一 canonical model。

4. **byte-audit baseline 依赖真实 fixture**  
   CLI、comparison API、runner 都已存在；但真实 `.pid` fixture 可能是私有数据，缺失时只能 soft-skip，无法证明真实样本 coverage 不退化。

5. **DWG publish loader enrichment 未闭环**  
   DWG style writer、branch point writer arm 已实现，但 DWG MDF fixture 和 loader-side canonical field enrichment 仍是阻塞项。

6. **许可证边界需要产品决策**  
   `vendor/oxidized-mdf` 是 GPL-3.0。内部使用问题小；若对外分发包含 vendored MDF reader 的二进制，需要按 GPL-3.0 提供对应源码并统一合规说明。

## 5. 产品目标

### 5.1 近期目标

让 `pid-parse` 对真实 `.pid` 文件的解析状态“可证明、可回归、可排序”，并让下游能够安全消费当前已稳定模型。

近期目标不应是直接宣称“完整解析 SmartPlant PID”，而是：

- 对每个 stream 说明解析深度。
- 对每段字节说明 consumed / leftover 状态。
- 对每个对象/关系/端点说明 provenance。
- 对 publish XML 输出建立 reference parity gates。

### 5.2 中期目标

将 `PidDocument` 从“多视图聚合模型”升级为“规范化语义图 + 派生视图”：

- normalized object。
- normalized relationship。
- normalized endpoint。
- normalized symbol ref。
- normalized cluster/segment ref。
- normalized geometry/text annotation。
- provenance-first model。

### 5.3 长期目标

达到“接近完整解析”的产品级门槛：

- 已知 top-level stream 均有明确 parser 与结构模型。
- Unknown 仅剩样本特异内容。
- 代表性真实样本 leftover byte ratio 低于约定阈值。
- layout/import/export 不再依赖临时 probe 拼装。
- writer 可以基于稳定语义做更高层编辑，而不仅是 raw stream patch。

## 6. 用户与场景

### 6.1 主要用户

| 用户 | 诉求 |
|---|---|
| CAD/图形导入方 | 从 `.pid` 中提取可读整图、对象、连接关系、标签、符号路径 |
| 数据治理/迁移方 | 从 backup/MDF 生成 SmartPlant-compatible XML 或结构化数据 |
| 逆向开发者 | 量化未知字节、定位下一批 parser 优先级 |
| QA/CI | 防止 parser coverage、publish parity、round-trip fidelity 回退 |
| 产品/交付 owner | 知道当前能力边界，避免把 probe 结果当成 fully decoded 数据承诺 |

### 6.2 核心使用场景

1. **检查单个 `.pid` 文件**  
   输入 `.pid`，输出文本报告、JSON、coverage、byte-audit、layout。

2. **批量评估解析覆盖率**  
   对多份真实 fixture 生成 byte-audit JSON，比较 baseline，识别 coverage regression。

3. **生成可读图模型**  
   下游使用 `PidDocument.layout` / object graph / crossref 生成预览图或导入 H7CAD。

4. **编辑 metadata 并回写 `.pid`**  
   修改 drawing number、template、summary property 或通用 XML tag，验证 round-trip diff。

5. **从 SmartPlant backup 生成 Publish XML**  
   输入 `Export.dmp`/`Export.mdf`、drawing UID、plant、style，输出 `_Data.xml`/`_Meta.xml`，并与 reference XML 做 semantic diff。

## 7. 下一阶段需求

### P0：固化当前状态 PRD 与 baseline 入口

**目标**：让团队对现状和边界有单一事实来源。

需求：

- 保留本文档作为当前状态入口。
- README 链接本文档或在架构文档中引用。
- 对外说明 `.pid` parser 与 publish XML pipeline 是两条相关但独立的能力线。
- 明确 probe / decode / inferred 的语义差异。

验收：

- 新成员只读 README + 本 PRD + architecture guide，即可理解当前边界。
- 文档不宣称未完成的 Sheet geometry/normalized graph 已完成。

### P1：真实 fixture 的 byte-audit baseline

**目标**：把 byte-audit 从工具能力变成 CI 回归门。

需求：

- 为至少 1 个真实 `.pid` fixture 生成 `docs/baselines/*.byte-audit.json`。
- 若 fixture 含敏感数据，baseline 只保留可公开字段；无法公开时保留私有 CI artifact 策略。
- 使用 `.github/scripts/check-byte-audit-baselines.sh` 跑 baseline comparison。
- regression 规则：
  - `overall_coverage_ratio` 不得下降。
  - 已 traced stream 的 `consumed_bytes` 不得下降。
  - traced 变回 unregistered 必须失败。

验收：

- 有 fixture 的环境里 baseline runner 能实跑并失败于 regression。
- 无 fixture 的公开 CI 保持 soft-skip 成功。
- 文档说明如何新增/更新 baseline。

### P2：PSMclustertable / PSMsegmenttable 结构化加深

**目标**：把 PSM tables 从 partial/audit 提升到更稳定的 decoded model。

需求：

- `PSMclustertable`：
  - 明确 cluster id / index / flags / type tag / declared segment count 等字段。
  - 保留 raw trailer。
  - 建立 declared cluster 与实际 `doc.clusters` 的映射。
- `PSMsegmenttable`：
  - 明确 segment record 或 flags 的语义分组。
  - 尝试建立 segment 与 Sheet endpoint / layout segment / relationship 的 provenance。
- parser 输出 raw + decoded + audit 三层。
- coverage 状态只有在多样本验证后才允许升级。

验收：

- 每个 parser 有 unit tests、fixture tests、byte-audit trace。
- 结构化字段有 drift guard。
- 至少 2 个真实样本验证后，才考虑把对应 coverage 从 `PartiallyDecoded` 升级。

### P3：Sheet geometry / text / symbol 深层解码

**目标**：从可读 layout hint 走向更完整的图面几何模型。

需求：

- 扩展 `--probe-sheet-chunks`：
  - record type 频次。
  - chunk boundary。
  - coordinate candidates。
  - text run offset。
  - symbol/reference candidate。
- 按高频 record type 逐步命名字段。
- 输出 typed sheet model：
  - geometry primitive。
  - text annotation。
  - connector/pipe segment。
  - symbol placement。
  - source byte ranges。
- 不确定字段留在 audit/probe，不进入 stable model。

验收：

- 对代表性 Sheet stream，byte-audit leftover 明显下降。
- layout 输出能解释更多 item/segment/text 的真实坐标来源。
- 无法解释的 bytes 有明确 leftover inventory。

### P4：规范化语义图层

**目标**：统一对象、关系、端点、符号、cluster、sheet 的来源模型。

需求：

- 新增 normalized graph model，至少包含：
  - `NormalizedObject`
  - `NormalizedRelationship`
  - `NormalizedEndpoint`
  - `NormalizedSymbolRef`
  - `NormalizedGeometry`
  - `Provenance`
- 每个实体携带：
  - stream path。
  - byte range 或 record id。
  - field_x / class_id / cluster index。
  - drawing id / model id / guid。
  - source layer：raw / decoded / probed / inferred。
- `inspect`、`layout`、`import_view` 逐步迁移到 normalized graph。

验收：

- 下游只消费 normalized graph 即可重建 object graph、crossref 和 layout 主要信息。
- 旧 API 保持兼容或提供迁移说明。
- 新模型有 JSON schema 和 fixture snapshot。

### P5：DWG publish loader enrichment

**目标**：关闭 DWG-side publish XML 主要剩余缺口。

需求：

- 落位 DWG `Export.mdf` fixture 或等价脱敏 mirror。
- 校实 `BranchPoint` / `PipingBranchPoint` 的 loader-side item type mapping 与 subtable chain。
- 完成 DWG-only canonical fields：
  - EqType / ProcessEqCompType。
  - ConnectionFlowDirection。
  - insulation / slope fields。
  - DWG-only style/canonical-field enrichment。
- 评估 `PublishStyle` 自动决策：
  - 当前显式 `--style a01|dwg` 保留。
  - 可选增加 metadata-driven auto style。

验收：

- `tests/publish_dwg_mirror.rs` 不再 soft-skip核心 gates。
- A24/A27b tolerated divergence whitelist 明显收敛。
- DWG reference 的 branch point count / interface / attr parity 通过。

## 8. 非目标

当前阶段不承诺：

- 完整复刻 SmartPlant 私有二进制 Sheet 几何格式。
- 对所有 plant/version 的 `.pid` 文件 100% 无差异解析。
- 对任意业务对象做语义级编辑并回写 Sheet 图元。
- 自动识别所有 DWG/A01/plant flavor。
- 公共仓库提交真实 plant 私有数据。

## 9. 验收指标

### 9.1 Parser 指标

- top-level coverage 中 Unknown 数量不增加。
- `PSMspacemap` 等已识别 storage 不再噪音化为 Unknown。
- byte-audit baseline 中 `overall_coverage_ratio` 不下降。
- 新 parser 必须有 panic-safety coverage。
- 新 decoder 必须声明 confidence：decoded / probed / raw。

### 9.2 Writer 指标

- metadata-only round-trip diff 为 0 或差异可解释。
- XML tag edit 只影响目标 stream/目标字段。
- Summary edit 不破坏未触碰属性。
- SheetPatch 继续标记 experimental，不能作为语义级编辑承诺。

### 9.3 Publish XML 指标

- A01 `_Data.xml` semantic diff 继续 clean。
- A01 `_Meta.xml` parity 继续 clean。
- Rel DefUID 与 UID soundness gates 继续通过。
- DWG 缺 fixture 时必须显式 soft-skip并输出 blockage。
- DWG fixture 落位后，branch-point 与 canonical enrichment gates 应由 soft-skip 转为 hard gate。

## 10. 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| 单 fixture 过拟合 | parser 在新 plant 上误解字段 | 至少 2-3 个真实样本才升级 coverage 等级 |
| Probe 结果被下游当成稳定语义 | 导入/渲染错误 | JSON/model 中明确 source layer 与 confidence |
| Byte coverage 被误读为语义完整 | 指标虚高 | 文档强调 byte-audit 是 evidence，不等于 fully decoded |
| GPL vendored MDF reader 合规 | 对外分发受限 | 保持 README license 说明；分发二进制时按 GPL-3.0 提供源码 |
| 私有 fixture 不可提交 | CI 覆盖不足 | public CI soft-skip + private CI baseline/artifact |
| Normalized graph 改造过大 | API churn | additive model 先行，旧视图保留一个兼容窗口 |

## 11. 推荐执行顺序

1. 将本文档接入 README / architecture guide，作为状态入口。
2. 恢复或准备一份可用于 CI 的真实 `.pid` fixture baseline。
3. 继续 Phase 12b byte-audit trace 扩面，优先减少 PSM/Sheet/Dynamic Attr leftover。
4. 做 PSMclustertable / PSMsegmenttable 结构化加深。
5. 单独设计 Sheet geometry 解码计划，不与 normalized graph 同轮混做。
6. DWG MDF fixture 落位后，优先关闭 DWG publish loader enrichment 和 whitelist。
7. 在上述模型稳定后，再启动 normalized graph 迁移。

## 12. 当前状态摘要

`pid-parse` 当前已经具备可用的 `.pid` 解析骨架、metadata writer、byte-audit 回归框架，以及 MDF-first publish XML 生成能力。它距离“完整解析所有 SmartPlant PID 语义”仍有明显距离，主要缺口集中在 PSM record 深层语义、Sheet 几何、规范化语义图层和 DWG loader enrichment。

下一阶段最有价值的工作不是继续堆零散 parser，而是用 fixture baseline + byte-audit + provenance model 把每一项解析进展变成可证明、可回归、可交付的产品能力。
