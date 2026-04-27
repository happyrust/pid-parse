# pid-parse 当前架构与原理

> 面向 SmartPlant / Smart P&ID `.pid` 文件的解析、审计、回写与离线发布管线。

![pid-parse current architecture](diagrams/pid-parse-current-architecture.png)

SVG 版本见 [`docs/diagrams/pid-parse-current-architecture.svg`](diagrams/pid-parse-current-architecture.svg)。

## 1. 一句话概览

`pid-parse` 的核心不是把 `.pid` 文件一次性“翻译”成业务对象，而是先把 OLE/CFBF 复合文档完整拆成可追踪的流清单，再在这个稳定容器视图上逐层叠加语义解析、派生模型、字节审计和声明式写回。

这让项目可以同时满足三类需求：

- **读取**：从 `.pid` 文件得到 `PidDocument`，用于检查、JSON/schema、报告和后续业务分析。
- **回写**：从 `PidPackage` 保留原始流字节，再通过 `PidWriter` 对少量目标流做可控修改。
- **逆向推进**：用 byte-audit 标记哪些字节已经被 parser 解释，哪些仍是 leftover，避免解析能力增长后发生回退。

## 2. 输入与入口

系统有三类主要输入。

第一类是 `.pid` 文件或内存中的 `.pid` 字节。它们本质上都是 CFBF compound file，内部由 storage 和 stream 组成。公开入口是 `PidParser`：

- `PidParser::parse_file()` 返回只读的 `PidDocument`。
- `PidParser::parse_package()` 返回 `PidPackage`，同时包含 `PidDocument` 和每条 stream 的原始字节。
- `PidPackage::from_bytes()` 走纯内存 `Cursor<Vec<u8>>`，不会创建临时 `.pid` 文件，也不会给 package 暴露 scratch path。

第二类是 SmartPlant backup package。`backup` 模块负责读 `Manifest.txt`、MTF envelope、RefData 文件名和 ZIP central directory，目标是支撑后续从离线备份恢复 PlantData / RefData / MDF 相关信息。

第三类是 `Export.mdf`。`publish` 管线通过 vendored `oxidized-mdf` 读取 SQL Server MDF，再生成 SmartPlant 发布阶段需要的 `_Data.xml` 和 `_Meta.xml`。

## 3. 读取路径

读取路径从 CFB 容器开始，而不是从业务对象开始：

1. `cfb::reader` 打开 compound file，构建目录树。
2. 采集 root CLSID、storage CLSID、storage timestamp、state bits 等容器元信息。
3. 枚举所有 stream，生成 `StreamEntry` 列表，并在 package 路径中保留 `RawStream` 字节。
4. 初始化 `PidDocument`，先保存容器结构和 stream inventory。
5. 顺序执行语义流解析器，逐步填充模型。

当前主要语义 pass 包括：

- Summary / DocumentSummaryInformation：OLE PropertySetStream 元数据。
- TaggedTxtData：Drawing / General XML 元数据。
- JSite：符号站点和 JProperties 探测。
- Cluster：`PSMcluster0`、`StyleCluster`、Dynamic Attributes Metadata。
- Dynamic Attributes：属性记录、trailer、class name、DrawingID 等。
- PSM tables：roots、cluster table、segment table。
- DocVersion / AppObject / JTaggedTxtStgList：版本日志和注册信息。
- Sheet endpoint：在 dynamic attributes 提供关系字段后，二次扫描 Sheet stream 的端点记录。

读取完成后，Full profile 还会构建 object inventory、object graph、cross-reference 和 layout。`import_view` 则作为按需 DTO，从已经解码的 `PidDocument` 派生。Light profile 保留 CFB tree、stream inventory、raw streams 等低成本信息，跳过 XML、JSite 属性、dynamic attrs、crossref、layout 等较重语义和派生 pass。

## 4. `PidDocument` 与 `PidPackage`

`PidDocument` 是解码后的规范模型，适合做只读查询、报告、JSON 输出和派生图。

`PidPackage` 是回写和审计的基础，它把两件事放在一起：

- `parsed: PidDocument`：当前解析视图。
- `streams: BTreeMap<String, RawStream>`：按规范化路径保存的原始 stream 字节。

这两个视图故意不做自动同步。比如调用 `replace_stream()` 或 `set_xml_tag()` 只改变 raw stream bytes，不会刷新 `parsed`。如果调用方需要读取修改后的 typed model，推荐路径是先用 `PidWriter::write_to_bytes()` 生成新的 CFB 字节，再重新 parse。

这个约束避免了半自动 reparse 带来的不清晰失效问题，尤其是 crossref、layout、object graph 这类派生层还没有完整 invalidation 契约时。

## 5. Probe / Decode 分层

解析器把“发现证据”和“确定语义”分开：

- **Decode**：字段布局和含义足够确定，可以写入结构化模型。
- **Probe**：只能稳定定位文本、magic、record boundary 或候选坐标，但语义尚未完全命名。

这种分层让项目在逆向闭源二进制格式时可以持续前进：Probe 结果不会假装成完整业务语义，但会留下偏移、长度、confidence 和 leftover，方便后续 parser 收紧。

## 6. Byte-audit 原理

Byte-audit 回答的是更底层的问题：每条原始 stream 里，哪些字节已经被注册 parser 解释过，哪些字节仍然没有被 claim。

它的流程是：

1. 输入 `PidPackage`，遍历所有 raw streams。
2. 根据 stream path 分发到对应的 `_with_trace` parser。
3. parser 用 `ParserTraceBuilder` 记录 consumed byte ranges 和 confidence。
4. 聚合为 `ByteAuditReport`，包含 per-stream coverage、overall coverage、unregistered paths 和 leftover bytes。
5. baseline comparator 比较两份报告，将 coverage 下降、stream 变回 unregistered、consumed bytes 下降归为 regression。

这套机制不是为了证明语义已经完整，而是为了建立解析覆盖率的回归防线。新增 parser 后，原本 unregistered 的 stream 可以变成 traced；如果未来改动让 coverage 下降，CI 和人工 review 都能立即看到。

## 7. Writer 原理

Writer 不直接修改解析模型，而是对 `PidPackage` 的 raw stream 视图应用声明式 `WritePlan`。

当前 canonical 顺序是：

1. `metadata_updates`：更新 Drawing / General XML，以及 SummaryInformation / DocumentSummaryInformation 字符串属性。
2. `stream_replacements`：替换任意指定 stream 的完整字节。
3. `sheet_patches`：对 Sheet stream 做实验性的字节区间 patch。
4. `cfb_write`：重新创建 compound file，写出 storage 和 stream。

`PidWriter::write_to()` 和 `PidWriter::write_to_bytes()` 都复用同一个 `apply_plan_to_package()`，validator 也复用这条路径，避免“写回逻辑”和“验证预期逻辑”各自维护一份顺序导致漂移。

writer 的核心目标是 byte-preserving：尚未理解的 stream 仍然按原始字节 passthrough，只有 `WritePlan` 指定的目标区域发生改变。

## 8. Backup 与 Publish 边线

Backup 模块和 Publish 模块不在 `.pid` 主读取路径上，但它们补齐 SmartPlant 工程数据的外围来源。

`backup` 面向桌面工具导出的 backup package，当前能解析：

- `Manifest.txt` 的 `key<<|>>value` 表格。
- MTF envelope 和 MDF page 相关基础结构。
- `RefData~SCHEMA~ID` 文件名和 magic-byte 分类。
- RefData ZIP central directory entry index。

`publish` 面向 `Export.mdf -> drawing graph -> _Data.xml / _Meta.xml`。它依赖 vendored `oxidized-mdf` 读取 MDF，把 SQL Server 数据转成中间 drawing graph，再由 XML writer 输出发布文件。

这两条边线的长期目标是让项目不仅能理解单个 `.pid` 文件，也能从 SmartPlant 离线备份中恢复发布数据和参考数据上下文。

## 9. 质量边界

当前架构的质量边界主要由四类门禁保证：

- Parser panic-safety：生产路径避免 `panic!` / `unwrap()`，新增 byte-level parser 要进 adversarial smoke tests。
- Rustdoc：`missing_docs`、broken intra-doc links、private intra-doc links 都是 deny。
- Clippy：已清理过的 pedantic lint 升级为 deny，防止新代码引入已知反模式。
- Byte-audit baseline：有 fixture 时可比较 coverage，避免解析覆盖率回退。

因此，新增解析能力时的推荐路径是：先用 Probe 暴露证据，再把稳定字段升级为 Decode，随后接入 byte-audit trace 和回归测试，最后再考虑把结果提升到 `PidDocument` 的公开模型。
