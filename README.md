# pid-parse

[![CI](https://github.com/happyrust/pid-parse/actions/workflows/ci.yml/badge.svg)](https://github.com/happyrust/pid-parse/actions/workflows/ci.yml)

`pid-parse` 是一个 Rust 编写的 SmartPlant / Smart P&ID `.pid` 文件解析器。

## 文档与产品现状

- [`docs/prd-pid-parse-current-state.md`](docs/prd-pid-parse-current-state.md) — **当前状态 PRD**：解析现状、能力分层、缺口与下一阶段需求（P0–P5）。新读者建议先看此文档对齐边界。
- [`docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`](docs/analysis/2026-06-03-pid-file-format-analysis-cn.md) — **PID 文件格式分析说明**：按当前实现口径汇总 CFB 容器、已知 stream/storage、magic、字节布局、coverage、byte-audit、Sheet decoded/audit/probe 分层与未知区。
- [`docs/sppid/v0.10.x-status.md`](docs/sppid/v0.10.x-status.md) — v0.10.0 (Phase 12b-1j) 解析能力快照：顶层流 coverage、byte-audit 注册表、roadmap Phase 完成度。
- [`docs/architecture-guide.md`](docs/architecture-guide.md) — 八层架构详解。
- [`docs/byte-audit-guide.md`](docs/byte-audit-guide.md) — `--byte-audit` CLI 与 baseline 比较器使用指南。
- [`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md`](docs/sppid/2026-04-21-sppid-full-parse-roadmap.md) — 多 Phase 解析路线图。

## 功能

- **容器解析**：OLE/CFBF 复合文件遍历与流索引
- **元数据提取**：OLE Summary (应用名/作者/创建时间) + TaggedTxtData XML (图号/模板/项目)
- **对象索引**：JSite 存储解析、符号路径提取、GUID 扫描
- **二进制记录解码**：Cluster 公共头、PSMcluster0 字符串表、动态属性结构化记录
- **Sheet 流探测**：复用 Cluster 公共头 + 0x89 标记扫描（v0.2.2）
- **PSM 索引表**：`PSMroots` / `PSMclustertable` / `PSMsegmenttable` 解码，得到 cluster 权威清单（v0.2.3）
- **文档注册表**：`DocVersion3` 版本日志 / `AppObject` COM 插件注册表 / `JTaggedTxtStgList` 解码（v0.2.4）
- **Magic 识别**：自动识别 PSMroots / PSMclustertable / PSMsegmenttable / DocVersion 等顶层结构化流（v0.2.2）
- **P&ID 对象清单**：从动态属性记录中提取设备/管道/仪表统计
- **关系端点解码**（v0.3.0）：DA 31 字节 trailer（record_id/field_x/class_id）+ Sheet 端点对记录 → 关系 source/target 端到端可解
- **跨引用对象图**（v0.3.0-rc2）：PSM 声明 vs. 实际 cluster 对齐 / 符号 ↔ JSite 反向索引 / DA 属性类摘要 / PSMroots 解析状态
- **Mermaid 可视化**（v0.3.0）：`ObjectGraph` / `CrossReferenceGraph` 一键导出 mermaid 文本，直接贴到 Mermaid Live Editor / Obsidian / Notion
- **JSON Schema 导出**（v0.3.1）：`pid_parse::schema::pid_document_schema()` / `pid_inspect --schema`
- **Byte audit**：`pid_inspect --byte-audit` 输出每个 raw stream 的 consumed / leftover 字节覆盖，用于 parser 覆盖率回归和未知区盘点；详见 `docs/byte-audit-guide.md`
- **Package / Writer 层**（v0.3.2）：`PidParser::parse_package` 保留原始字节、`PidWriter::write_to` 声明式回写（passthrough round-trip + Drawing/General XML 回写 + experimental SheetPatch）
- **Root CLSID 保留 + CLI 回写**（v0.3.3）：`pid_inspect --round-trip` / `--set-drawing-number` / `--schema`；`PidPackage.root_clsid` 读写；详见 `docs/writer-clsid-and-timestamps.md`
- **通用 XML metadata editor**（v0.3.4）：`PidPackage::set_xml_tag` / `--set-xml-tag <stream> <tag> <value>` 编辑任意 `/TaggedTxtData/*` 里任意简单 tag
- **Package diff + verify**（v0.3.5）：`diff_packages` + `--diff <a> <b>` / `--round-trip --verify`，CI 友好 exit code，字节级差异 + hex context
- **Layout-first 可读整图模型**（v0.4.1）：`PidDocument.layout` 输出 `items / segments / texts / unplaced / warnings`，供 H7CAD 等下游优先生成可读整图而不是语义网格图
- **pid-only 符号证据下沉**（v0.4.1 patch）：从 `cross_reference.symbol_usage` / `jsites` 提取代表性 `.sym` 路径，补到对象级 `PidLayoutItem.symbol_path`
- **Backup 解析（offline pipeline）**：MTF 备份头剥离（`backup::mtf`）+ MSCI / MDF 元数据探针 + `pid_backup_extract` CLI，把 SmartPlant 备份还原成可被 Rust MDF loader 读取的 `.mdf`
- **Publish Data XML writer**（Stage-1，A12 → A39）：通过本地克隆的 `vendor/oxidized-mdf` 直接读取 MDF 内的 `T_Drawing` / `T_ModelItem` / `T_Representation` / `T_Relationship` 等表，发出 SmartPlant 兼容的 `_Data.xml` + `_Meta.xml`。21 类 PID tag 已支持（`.pid` 线的 registry-driven 集合，权威清单在 `src/publish/catalog.rs` 的 `PublishItemCatalog`：PIDBranchPoint / PIDControlSystemFunction / PIDDrawing / PIDHeatTransferEquipment / PIDInlineInstrument / PIDInstrument / PIDMechanicalEquipment / PIDNote / PIDNozzle / PIDPipeline / PIDPipingBranchPoint / PIDPipingComponent / PIDPipingConnector / PIDPipingPort / PIDProcessEquipment / PIDProcessPoint / PIDProcessVessel / PIDRepresentation / PIDSignalBranchPoint / PIDSignalConnector / PIDSignalPort）；A01 reference 上共享 tag 的接口集 / 属性集继续由 A23 / A27 守门，DWG reference 的 writer coverage 已达 `108/108`
- **Publish fidelity 守门**：tag-count diff（A12）+ writer coverage 分类（A15）+ 接口级 parity（A23 / A24）+ 属性级 parity（A27 / A27b）+ backlog tag inventory snapshot（A28）+ A01/DWG style 切换（A29 / A29b）+ A01 raw residual 完整 MDF 证据探针（A39）。任何 future SmartPlant 端漂移会以 `(tag, interface, attr)` 三元组或 raw synthetic slot 证据门失败定位
- **PIDProcessVessel tank 变体**（A25）：通过 `obj.fields["IsLowPressureTank"]` 路由，DWG-style "Open top tank" 17 接口形态与 A01-style "Horizontal Drum" 15 接口形态共用一套 writer
- **Probe / Decode 分层**：启发式标记与确定性解码明确分离
- **报告输出**：人类可读文本报告 + JSON 完整导出

## 使用

```bash
# 文本报告
cargo run --bin pid_inspect -- drawing.pid

# JSON 输出
cargo run --bin pid_inspect -- drawing.pid --json

# Cluster / 动态属性 / Sheet 专项探测
cargo run --bin pid_inspect -- drawing.pid --probe-cluster
cargo run --bin pid_inspect -- drawing.pid --probe-dynamic
cargo run --bin pid_inspect -- drawing.pid --probe-sheet
cargo run --bin pid_inspect -- drawing.pid --probe-sheet-chunks
cargo run --bin pid_inspect -- drawing.pid --probe-sheet-chunks Sheet6

# 关系探测 / 端点解析
cargo run --bin pid_inspect -- drawing.pid --probe-relationships
cargo run --bin pid_inspect -- drawing.pid --probe-endpoints

# 跨引用对象图
cargo run --bin pid_inspect -- drawing.pid --crossref

# Mermaid 图导出
cargo run --bin pid_inspect -- drawing.pid --graph-mermaid > object_graph.mmd
cargo run --bin pid_inspect -- drawing.pid --crossref-mermaid > crossref.mmd

# Mermaid 导出演示（无需 .pid 样本，使用合成数据）
cargo run --example mermaid_demo

# JSON Schema
cargo run --bin pid_inspect -- drawing.pid --schema > pid-schema.json

# Byte audit（consumed / leftover 字节覆盖）
cargo run --bin pid_inspect -- drawing.pid --byte-audit
cargo run --bin pid_inspect -- drawing.pid --byte-audit --json > audit.json

# Passthrough 回写（Root CLSID 保留 + 所有流字节保持）
cargo run --bin pid_inspect -- drawing.pid --round-trip drawing.copy.pid

# 改图号并写出（仅动 /TaggedTxtData/Drawing 里的 <DrawingNumber>）
cargo run --bin pid_inspect -- drawing.pid \
    --set-drawing-number NEW-0001 --output drawing.new.pid

# 改任意 <tag> 字段（通用版，drawing-number 是它的特化）
cargo run --bin pid_inspect -- drawing.pid \
    --set-xml-tag /TaggedTxtData/Drawing Template NEW_TEMPLATE.pid \
    --output drawing.new.pid

# Passthrough 写回并自动 diff 验证（retval=0 表示 0 diffs）
cargo run --bin pid_inspect -- drawing.pid --round-trip out.pid --verify

# 两个 .pid 文件的 stream 级字节 diff（retval=1 当存在差异）
cargo run --bin pid_inspect -- a.pid --diff b.pid
```

### Backup 解析 + Publish Data XML 生成（offline pipeline）

```bash
# 1. 从 SmartPlant 备份（Export.dmp）剥离 MTF 头得到 .mdf。--out 给的是输出目录；
#    加 --as-mdf 才写出 Export.mdf（旁边另有 Export.msci.json / .msci.bin / .msda.bin）
cargo run --bin pid_backup_extract -- Export.dmp --out . --as-mdf

# 2. 用 Rust MDF loader（vendor/oxidized-mdf）直接列出 drawing
cargo run --bin pid_publish_xml -- Export.mdf --list-drawings

# 3. 生成单张 drawing 的 _Data.xml（默认 A01 style）
cargo run --bin pid_publish_xml -- Export.mdf \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --out A01_Data.xml

# 4. 同时生成 _Meta.xml 配套
cargo run --bin pid_publish_xml -- Export.mdf \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --out A01_Data.xml --meta-out A01_Meta.xml

# 5. 切换到 DWG-style IObject 形态（drop ItemTag, 用 Name）
cargo run --bin pid_publish_xml -- Export.mdf \
    --drawing <UID> --plant TEST02 --style dwg --out DWG_Data.xml

# 6. 跟 SmartPlant 参考 _Data.xml 跑 SemanticDiff（CI gate）。有差异退出 1；A01 对
#    仓库里它的参考（test-file/export-test/publish-data/A01/A01_Data.xml）现在就有。
cargo run --bin pid_publish_xml -- Export.mdf \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --diff-against reference/A01_Data.xml

# 7. 单张 .pid + Oracle exp 增强：--oracle-exp 读 piping / signal 端口，
#    --datamap 补 FluidSystem，--out-dir 写出三件（见下方「三条输入线」）。
cargo run --bin pid_publish_xml -- drawing.pid \
    --oracle-exp Export.dmp --datamap SPPIDDataMap.xml --out-dir out

# 8. 备份包目录（目录里有 Manifest.txt）。Oracle exp 的 Export.dmp 读一次，
#    PlantData zip 里每张 .pid 写出三件，包级另写 _Index.json。
cargo run --bin pid_publish_xml -- <backup-dir> --out-dir <dir> --plant P01

# 9. --strict：数据库线把对象或关系写得不完整时退出 1，文件照写（见下方
#    「数据库线写得不完整时」）。A01 写得完整，这一条退出 0。
cargo run --bin pid_publish_xml -- Export.mdf \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --out A01_Data.xml --strict
```

### publish 的三条输入线与四个产出文件名

| 输入 | 命令 | 逐图产出 | 包级产出 |
|---|---|---|---|
| 单张 `.pid` | `pid_publish_xml <file.pid> --out-dir DIR` | `<图名>_Data.xml` + `<图名>_Meta.xml` + `<图名>_Coverage.json` | — |
| `.pid` + Oracle exp | `pid_publish_xml <file.pid> --oracle-exp <dump> [--datamap <SPPIDDataMap.xml 或含该条目的 zip>] --out-dir DIR` | 同上三件 | — |
| 备份包目录 | `pid_publish_xml <backup-dir> --out-dir DIR` | 三件（Oracle exp 与 SQL Server MTF 同形：`<图名>_Data.xml` + `<图名>_Meta.xml` + `<图名>_Coverage.json`） | `_Index.json` |

- **四个产出文件名**：`_Data.xml`（对象 / 值 / 关系）、`_Meta.xml`（容器与版本元数据）、`_Coverage.json`（逐图覆盖报表）、`_Index.json`（包级索引）。`.pid` 输入与 Oracle exp 备份包的每张图交付就是三件（`_Data.xml` + `_Meta.xml` + `_Coverage.json`）。
- **单张 `.pid` 输入的 `<图名>`** 是文件名去掉扩展名：`drawing.pid` 写出 `drawing_Data.xml`、`drawing_Meta.xml`、`drawing_Coverage.json`。备份包里每张图用的名字和写出的文件见 `_Index.json` 的 `name` 与 `files`。
- **SQL Server MTF 的 `Export.dmp`**：默认特性（`backup` ⊃ `backup-mdf`）下先把 MTF 头剥出 MDF，再按数据库线把每条 `T_Drawing` 行发布出来（退出 0）。
- **`--style a01|dwg` 只对数据库输入**（`Export.mdf` / legacy `Export_v2.sqlite`）有效：`.pid` 输入与备份包目录都会拒绝它（退出 2 并点名 `--style`），不再静默忽略。

### 命令行约定与退出码

- **输入是第一个参数**，旗标都跟在它后面。旗标写在前面会被当成输入：`pid_publish_xml --stdout a.pid` 是用法错误（`unknown flag: a.pid`）。
- **退出 0**：写出了；带 `--diff-against` 时还表示对照干净。
- **退出 1**：I/O 或格式错误（stdout 写不进去也算）、`--diff-against` 有差异、整包记了失败，或者带 `--strict` 时数据库线有图写得不完整。
- **退出 2**：用法错误，包括 `--oracle-exp` 给的 dump 里没有这张图的 `T_DRAWING` 行。

### 数据库线写得不完整时：告警、`--strict` 与 `shortfalls`

数据库线（`Export.mdf`、legacy `.sqlite`、SQL Server 备份包）不拒绝任何一张图。有两样东西它写得出来、但写不完整，光看 `_Data.xml` 看不出来：

- **类型没有专用写法的对象**（`Exchanger`、`Mechanical`、`EquipmentOther`、`OPC`，以及 `PublishItemCatalog` 里没有列的类型）写成只带 UID 的 `<PIDItem>` 占位，类和属性都不写。
- **有一端是对象、不是图形表示的 `T_Relationship` 行**，`<Rel>` 的定义只凭两端的对象类型决定。

每样在 stderr 上一行 `warning:`，按类型给出个数；文件照写，退出码还是 0。加上 `--strict`，文件一个字节不变，退出码变成 1，stderr 末尾多一行说明。仓库里有一份小样本可以直接试（要有 `sqlite3`；`.sqlite` 输入另有一行弃用提示）：

```bash
sqlite3 shortfalls.sqlite < tests/fixtures/publish-shortfalls.sql
cargo run --bin pid_publish_xml -- shortfalls.sqlite \
    --drawing DWG-SHORT-01 --out SHORT-01_Data.xml --strict
# 退出 1。pid_publish_xml 在 stderr 上最后说的三行（经 cargo run 跑，后面还跟着 cargo 自己的一行 error:）：
# warning: 1 object written as a <PIDItem> placeholder, class and attributes left out: Exchanger 1
# warning: 1 relationship row defined from the item types of its ends alone: Nozzle -> Exchanger as Relationship 1
# --strict: the drawing is not written in full (see the warnings above); exit 1.
```

- **`--strict` 只管数据库线。** `.pid` 输入和 `--list-drawings` 带它是用法错误（退出 2 并点名 `--strict`）。备份包目录接受它：SQL Server 包的图走数据库线，受它管；Oracle exp 包和没有 dump 的包每张图都来自 `.pid`，带不带它结果一样。
- **整包里**每句告警前面带图名（`warning: <图名>: …`）。同一句话写进 `_Index.json` 顶层的 `problems`，不带图名的那一句写进这张图自己的 `problems`；图照样算写出，`failed` 不变。
- **不在这本账里的**：数据库里没有的可选表会被跳过，只有带 `-v` 时在 stderr 上点名；整包里一张图的 `_Data.xml` 把同一个 UID 写在不止一个块上，是 stderr 上一行告警和 `_Index.json` 的 `problems` 里一条，图照样算写出。这两样 `--strict` 都不管。

SQL Server 包每张图的 `<图名>_Coverage.json` 末尾都有一段 `shortfalls`。写得完整时两个 `total` 都是 0，A01 就是这样：

```json
  "shortfalls": {
    "placeholder_objects": {
      "total": 0,
      "by_item_type": {}
    },
    "relationship_rows_defined_by_item_type": {
      "total": 0,
      "rows": []
    }
  }
```

写得不完整时，`by_item_type` 是「对象类型 → 个数」，`rows` 的每一项是 `item1` / `item2`（两端的类型）、`written_as`（写出的定义）和 `count`。`.pid` 线的覆盖报告没有这一段，它没写什么由自己的 `left_out` / `left_out_by_design` 说明。

### 文件怎么落盘

- **整份到位。** `pid_publish_xml` 写文件，是先整份写到目标旁边的临时名 `<文件名>.<进程号>.tmp`，写完再改名（例外见本节最后一条）；没到位就失败的，临时文件删掉。被打断的运行不会留下写了一半的 `_Data.xml` / `_Meta.xml` / `_Coverage.json` / `_Index.json`。
- **整包里一张图的三件，有一件写不成就一件都不留。** 三件先都写到临时名，再依次改名；有一件改不成，已经到位的删掉，这张图在 `_Index.json` 里点名，其余的图照写。这防的是写失败，不是运行被打断。
- **被打断之后可能留下的**：名字以 `.tmp` 结尾的文件（可以删），和三件不齐的图（打断在两次改名之间）。已经到位的每个文件都是整份的。
- **`--out-dir` 不先清空**：以前的运行留在那里的文件不会被删。整包以 `_Index.json` 为准看这一次写了哪些文件。
- **没有做的**：不 `fsync`，不防掉电。单个文件（`--out`、`--meta-out`、单图 `--out-dir` 的各件、`_Index.json`）的目标名下已经是设备（`NUL`、`/dev/null`）、链接或目录时不改名，照旧原地写（`--out NUL` 这样的用法不变，目录照旧失败）；整包里每张图的三件一律改名。

### 备份包与 `_Meta.xml` 的 publish 门禁
- **整包 `--diff-against <参考目录>`**：备份包目录现在也接受 `--diff-against`，逐张图对照 `<DIR>/<图名>_Data.xml`，每张打印一行 `clean` / `dirty` / `unpaired` / `revision mismatch` 判词，末尾一行汇总各判词的张数和退出码。
- 某张图 dirty、参考是同名图的另一个修订（`revision mismatch`，这张图不比）、或在该目录里没有参考文件（`unpaired`），进程退出 1：失败的图会被记账，其余图照常写出，全部写完后退出 1。
- **`_Meta.xml` 伴随门禁**：本轮写出的 `_Meta.xml`（`--out-dir` 或 `--meta-out`）还会与参考 `<图名>_Data.xml` 旁的 `<图名>_Meta.xml` 对照，比 13 个 `<Container>` 属性、顶层块顺序、三条 `Rel` 的 DefUID 及角色接线，以及兄弟 `_Data.xml` 的 DocUID。
- 参考旁没有兄弟 meta 时，meta diff 明确报告它没有运行，而不是静默判过。
- **已接受差异是数据，不是断言**：`docs/baselines/publish-diff-accepted.json` 由 `tests/publish_diff_accepted_baseline.rs` 消费，把一次 `--diff-against` 运行打印的每一行归为 registered / disappeared / unregistered。
- A01 仍按设计退出 1 —— 基线是逐项记录，不是抑制机制。

当前公开的 publish 正确性基线只承诺 `Export.mdf` 主链。
历史 `Export_v2.sqlite` mirror 仍可作为 legacy 兼容输入喂给
`pid_publish_xml`，但不再承担 publish fidelity 验收角色；CLI
对 `.sqlite` 输入会打印 deprecation 提示。

A01 `_Data.xml` 当前满足语义 diff、接口/属性/关系 parity、格式风格
和 `_Meta.xml` parity。raw byte 精确对齐只剩 3 类 A39 证据化
publish-time synthetic slots：`PIDPipingConnector` UID 家族、`Rel`
块内的 synthetic `IObject UID`、以及 `PIDRepresentation GraphicOID`。
ignored 探针会用 Rust MDF reader 枚举 TEST02 MDF 全表；当前实测
128 张表全部进入完整 Rust MDF 表扫描，`tables_skipped=0`，三类
参考残余值在 staging 表、完整表清单、以及 MDF raw ASCII /
UTF-16LE / UUID byte-form 扫描中均无命中，因此 delivery contract
只对这 3 类 slot 做窄归一化。

DWG plant 的 loader / branch-point 回归目前只依赖一份 compare-only 的
MDF 样板：
`test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf`。
这份样板仅用于 DWG `Export.mdf` 对比验证，不要求也不绑定其对应数据库。
仓内已带 `tests/publish_dwg_mirror.rs` 与 `tests/publish_meta_parity.rs`
作为入口；若该 MDF 缺失，这两组 DWG 侧测试会 soft-skip，并在
输出中明确提示“DWG canonical-field enrichment / branch-point parity 未验证”。

## 库调用

### 只读解析

```rust
let parser = pid_parse::PidParser::new();
let doc = parser.parse_file("drawing.pid")?;

// 访问 Summary
if let Some(ref summary) = doc.summary {
    println!("Application: {:?}", summary.creating_application);
}

// 访问对象清单
if let Some(ref inv) = doc.object_inventory {
    println!("Total items: {}", inv.items.len());
    for (item_type, count) in &inv.item_counts {
        println!("  {}: {}", item_type, count);
    }
}
```

### 读+写（v0.3.2+）

```rust
use pid_parse::{PidParser, PidWriter, WritePlan, MetadataUpdates};

let parser = PidParser::new();
let pkg = parser.parse_package("drawing.pid")?;  // 同时保留原始字节

// 只改图号的便捷写法（其它所有流 passthrough）
let plan = WritePlan::metadata_only(
    Some("<Drawing><DrawingNumber>NEW-001</DrawingNumber></Drawing>".into()),
    None,
);
PidWriter::write_to(&pkg, &plan, std::path::Path::new("drawing.out.pid"))?;

// 空 plan 就是 passthrough：重新序列化但保留全部字节
PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new("drawing.copy.pid"))?;
```

**当前边界**：`SummaryInformation` property set 不写；不保留原容器 CLSID / 时间戳；TaggedTxtData 按字节替换（调用方自备编码）；`SheetPatch` 只开 API，未接 CLI。

### Publish Data XML 生成（v0.9.2+）

```rust
use pid_parse::publish::{
    load_drawing_graph_from_mdf, write_data_xml, write_meta_xml, PublishStyle,
};

let mut graph = load_drawing_graph_from_mdf(
    "Export.mdf".as_ref(),
    "D9635C3C898840D1990B7E8BEE1D55DA",
)?;

// 默认是 A01 style；如果数据来自 DWG 风格的 plant：
graph.style = PublishStyle::Dwg;

let data_xml = write_data_xml(&graph, "TEST02")?;
let meta_xml = write_meta_xml(&graph, "TEST02")?;
std::fs::write("A01_Data.xml", &data_xml)?;
std::fs::write("A01_Meta.xml", &meta_xml)?;
```

### Publish fidelity 分析

```rust
use pid_parse::publish::{
    coverage_against_reference, diff_publish_xml,
    parse_attrs_per_interface_per_tag, parse_interfaces_per_tag,
};

let generated = std::fs::read_to_string("A01_Data.xml")?;
let reference = std::fs::read_to_string("reference/A01_Data.xml")?;

// A12 — tag-count 级 SemanticDiff
let report = diff_publish_xml(&generated, &reference);
println!("{report}");
assert!(report.is_clean(), "writer drift!");

// A15 — 把 reference 里的 PID tag 分成 supported / backlog
let coverage = coverage_against_reference(&reference);
println!("{coverage}");

// A23 — 接口级集合
let ifaces = parse_interfaces_per_tag(&reference);
for (tag, set) in &ifaces {
    println!("{tag}: {set:?}");
}

// A26 / A27 — 属性级集合（per (tag, interface)）
let attrs = parse_attrs_per_interface_per_tag(&reference);
```

## 示例输出

```
=== PID Document Report ===

Streams: 69
JSites:  21
Clusters: 4
Sheet streams: 1

--- Summary ---
  Application: SmartPlantPID Application
  Created: 2025-12-29T02:38:19Z
  Modified: 2026-03-16T03:24:18Z

--- Sheets ---
  Sheet6 (29594 bytes, magic=0x6C90F544)
    header: type=0x00CE, records=354, body=121

--- PSMroots (278 bytes) ---
  [@+0004] id=0x0000018C  Imagineer Document
  [@+0030] id=0x00000149  Server Document
  [@+0056] id=0x00000019  _SupportOnlyList
  [@+007E] id=0x00000014  TopVFSet
  [@+0096] id=0x00004000  Dynamic Attributes Set Table
  [@+00D6] id=0x00002000  StyleLibrarian
  [@+00FA] id=0x00000001  DocStore

--- PSMclustertable (265 bytes, declared count=5) ---
  [@+001B] PSMcluster0
  [@+0042] StyleCluster
  [@+006B] Dynamic Attributes Metadata
  [@+00AE] Sheet6
  [@+00CB] Unclustered Dynamic Attributes

--- PSMsegmenttable (12 bytes, count=4) ---
  flags: [0x01, 0x01, 0x01, 0x01]

--- Version History (192 bytes, 4 records) ---
  [SA 12/29/25 10:45] SmartPlantPID.a 090000.0144
  [SV 03/10/26 15:17] SmartPlantPID.a 090000.0077
  [SV 03/10/26 19:10] SmartPlantPID.a 090000.0144
  [SV 03/16/26 11:24] SmartPlantPID.a 090000.0077

--- App Object Registry (673 bytes, leading=0x00000005, 5 entries) ---
  {D69F42DF-7717-11D1-9790-08003655F302} -> C:\...\igrSmartLabel.dll
  {3660253E-6763-11D2-A359-08003636E802} -> C:\...\igrGluePnt.dll
  {D1E93B31-1A68-11D1-A222-080036C1C902} -> C:\...\igrConnector.dll
  ...

--- Tagged Text Storage List (70 bytes) ---
  list: TaggedTxtStorages
    -> TaggedTxtData

--- P&ID Object Inventory ---
  Project: SQLPlant1401
  Total items: 140
    PipeRun: 53
    Relationship: 64
    Nozzle: 6
    Instrument: 3

--- Cross Reference ---
  Clusters: declared=5 found=5 matched=5
  Symbols: 8 unique (21 total JSite refs)
    [4x] Valve (JSite0,JSite3,JSite5 ...)
    [3x] Instrument (JSite1,JSite7,JSite9 ...)
    ...
  Attribute classes: 10
    P&IDAttributes (records=140, attr_names=35, drawings=1, models=140)
    PipeRun (records=53, attr_names=12, drawings=0, models=0)
    ...
  PSMroots: 7 entries, 4 resolved in CFB tree
    [STORAGE] id=0x0000018C  Imagineer Document
    [MISSING] id=0x00000019  _SupportOnlyList
    ...
```

## Publish writer Stage-1 fidelity 矩阵

```
                    | tag count | interface | attribute |  spec  | style 切换
writer ⊇ A01 ref    | A12       | A23       | A27       |  --    | A29 (default A01)
writer 能输出 DWG    |  --       |  --       |  --       |  --    | A29 (style=Dwg)
A01 ref ⇄ DWG ref   |  --       | A24       | A27b      |  --    |   --
backlog (DWG only)  | A28       | A28       | A28       | + 3 grd|   --
CLI surface         |  --       |  --       |  --       |  --    | A29b (--style)
```

21 类 PID tag 已实现 writer arm（`.pid` 线的 registry-driven 集合，见 `src/publish/catalog.rs`）；DWG reference 已达到 `108/108` writer coverage，
`PIDBranchPoint × 5` 与 `PIDPipingBranchPoint × 4` 已从 backlog snapshot 毕业到
supported set。PIDProcessVessel 已支持 A01 "Horizontal Drum"（15 接口）和
DWG "Open top tank"（17 接口）两种 variant；当前剩余缺口集中在 mirror-gated
loader fidelity：A24 仍保留 `PIDProcessVessel` 的 tank variant 单项接口白名单，
A27b 仍保留 15 条 DWG-only style / canonical-field enrichment 差异。任何未来
SmartPlant 端漂移会以 `(tag, interface, attr)` 三元组在 CI 上失败定位。

## Final format-analysis and validation status (2026-06-19)

Current user-facing status is evidence-based, not a claim that the vendor PID
format is completely decoded. The authoritative artifacts are:

- `docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`: confidence
  vocabulary and ledger for `Decoded`, `TypedAudit`, `Probe`,
  `IdentifiedOnly`, and `Unknown` outputs.
- `docs/analysis/2026-06-19-ida-evidence-baseline.md`: read-only IDA evidence
  scope. Current `sppid.dll` and `core.dll` checks did not justify upgrading
  unresolved raw `.pid` families; historical `style.dll`, `OLESITE.dll`,
  `OLECRT.dll`, and `radsrvitem.dll` evidence remains scoped to the rows named
  in that document.
- `docs/plans/2026-06-19-pid-parser-roadmap-gates.md`: parser promotion gates
  and forbidden shortcuts for `PSMspacemap`, `StyleCluster`, `0x0010`,
  `DependencyObject 0x00FA`, `JSitesList.trailing_slots`, `0x0089` / class 137
  DA heads, and page transform / coordinate units.
- `docs/pid-export-bundle-contract.md` and `docs/writer-layer-plan.md`: bundle
  shape, raw-byte opt-in behavior, writer whitelist, publish separation, and
  read-only geometry/audit/probe boundaries.

Black-box CLI validation was exercised on
`test-file/DWG-0201GP06-01.pid` with `pid_inspect --json`,
`pid_inspect --byte-audit --json`, default `--export-bundle`,
`--round-trip ... --verify`, and `pid_writer_validate --json`. The default
bundle produced `manifest.json`, `raw/streams.json`,
`decoded/document.json`, `audit/confidence_ledger.json`, split geometry files,
and writer guidance without creating `raw/streams/*.bin`, preserving the raw
payload opt-in rule. The optional publish bundle and `pid_publish_xml` flow were
also exercised against `test-file/backup-test/TEST02_p/extracted/Export.mdf`
for drawing `D9635C3C898840D1990B7E8BEE1D55DA` / plant `TEST02`; semantic diff
against `test-file/export-test/publish-data/A01/A01_Data.xml` was clean. The
DWG publish MDF fixture
`test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf` was absent and
therefore soft-skipped explicitly.

Final gate evidence for this status snapshot covered formatting, library
tests, parser panic-safety, real fixture tests, clippy, workspace build,
workspace tests, and the missing-docs ratchet. Remaining blockers are the
non-decoded atlas rows listed above: raw `PSMspacemap` page bytes,
`StyleCluster` prefix semantics, `0x0010` discriminator,
`DependencyObject 0x00FA`
payload semantics, `JSitesList` trailing-slot writer meaning, `0x0089` DA head
semantics, page transform / coordinate units, and any semantic Sheet writer.

## License

The `pid-parse` source code (everything **outside** `vendor/`) is dual-licensed under
[MIT](https://opensource.org/licenses/MIT) OR
[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0),
at your option.

### Vendored dependency — GPL-3.0

`vendor/oxidized-mdf/` is a **modified** fork of
[oxidized-mdf](https://gitlab.com/schrieveslaach/oxidized-mdf)
by schrieveslaach, licensed under
[GPL-3.0-or-later](https://www.gnu.org/licenses/gpl-3.0.html).
Because `pid-parse` statically links this crate, the **combined binary**
(or any distribution that includes both components) is subject to the
terms of the GPL-3.0. Both MIT and Apache-2.0 are GPL-3.0-compatible,
so there is no license conflict.

**In practice this means:**

- **Internal / private use**: no obligations beyond keeping the license files.
- **Public distribution of binaries**: you must make the Corresponding Source
  available under GPL-3.0 (this repository already satisfies that requirement).
- **Reusing only `pid-parse` code** (without the vendored parser): the MIT / Apache-2.0
  terms apply; no GPL obligation.

Modification notices for the vendored files are recorded at the top of each
modified source file per GPL-3.0 §5(a).

