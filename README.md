# pid-parse

[![CI](https://github.com/happyrust/pid-parse/actions/workflows/ci.yml/badge.svg)](https://github.com/happyrust/pid-parse/actions/workflows/ci.yml)

`pid-parse` 是一个 Rust 编写的 SmartPlant / Smart P&ID `.pid` 文件解析器。

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
- **Package / Writer 层**（v0.3.2）：`PidParser::parse_package` 保留原始字节、`PidWriter::write_to` 声明式回写（passthrough round-trip + Drawing/General XML 回写 + experimental SheetPatch）
- **Root CLSID 保留 + CLI 回写**（v0.3.3）：`pid_inspect --round-trip` / `--set-drawing-number` / `--schema`；`PidPackage.root_clsid` 读写；详见 `docs/writer-clsid-and-timestamps.md`
- **通用 XML metadata editor**（v0.3.4）：`PidPackage::set_xml_tag` / `--set-xml-tag <stream> <tag> <value>` 编辑任意 `/TaggedTxtData/*` 里任意简单 tag
- **Package diff + verify**（v0.3.5）：`diff_packages` + `--diff <a> <b>` / `--round-trip --verify`，CI 友好 exit code，字节级差异 + hex context
- **Layout-first 可读整图模型**（v0.4.1）：`PidDocument.layout` 输出 `items / segments / texts / unplaced / warnings`，供 H7CAD 等下游优先生成可读整图而不是语义网格图
- **pid-only 符号证据下沉**（v0.4.1 patch）：从 `cross_reference.symbol_usage` / `jsites` 提取代表性 `.sym` 路径，补到对象级 `PidLayoutItem.symbol_path`
- **Backup 解析（offline pipeline）**：MTF 备份头剥离（`backup::mtf`）+ MSCI / MDF 元数据探针 + `pid_backup_extract` CLI，把 SmartPlant 备份还原成可被 OrcaMDF probe 重放的 `.mdf`
- **Publish Data XML writer**（Stage-1，A12 → A31）：从 OrcaMDF probe 产出的 SQLite mirror 加载 `T_Drawing` / `T_ModelItem` / `T_Representation` / `T_Relationship` 等表，发出 SmartPlant 兼容的 `_Data.xml` + `_Meta.xml`。13 类 PID tag 已支持（PIDProcessVessel / PIDNozzle / PIDPipeline / PIDPipingConnector / PIDPipingComponent / PIDPipingPort / PIDProcessPoint / PIDSignalConnector / PIDSignalPort / PIDControlSystemFunction / PIDNote / PIDDrawing / PIDRepresentation），每个 tag 的接口集和属性集与 A01 reference 字节级对齐
- **Publish fidelity 9 道守门**：tag-count diff（A12）+ writer coverage 分类（A15）+ 接口级 parity（A23 / A24）+ 属性级 parity（A27 / A27b）+ backlog tag inventory snapshot（A28）+ A01/DWG style 切换（A29 / A29b）。任何 future SmartPlant 端漂移会以 `(tag, interface, attr)` 三元组在 CI 上失败定位
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
# 1. 从 SmartPlant 备份（Export.dmp）剥离 MTF 头得到 .mdf
cargo run --bin pid_backup_extract -- Export.dmp --out Export.mdf

# 2. （C# 工具）OrcaMDF probe 把 .mdf 转成 SQLite mirror
#    详见 tools/orca-mdf-probe/Program.cs

# 3. 列出 SQLite mirror 里的全部 drawing
cargo run --bin pid_publish_xml -- Export_v2.sqlite --list-drawings

# 4. 生成单张 drawing 的 _Data.xml（默认 A01 style）
cargo run --bin pid_publish_xml -- Export_v2.sqlite \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --out A01_Data.xml

# 5. 同时生成 _Meta.xml 配套
cargo run --bin pid_publish_xml -- Export_v2.sqlite \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --out A01_Data.xml --meta-out A01_Meta.xml

# 6. 切换到 DWG-style IObject 形态（drop ItemTag, 用 Name）
cargo run --bin pid_publish_xml -- Export_v2.sqlite \
    --drawing <UID> --plant TEST02 --style dwg --out DWG_Data.xml

# 7. 跟 SmartPlant 参考 _Data.xml 跑 SemanticDiff（CI gate）
cargo run --bin pid_publish_xml -- Export_v2.sqlite \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --diff-against reference/A01_Data.xml
```

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
    load_drawing_graph, sqlite_load::open_readonly,
    write_data_xml, write_meta_xml, PublishStyle,
};

let conn = open_readonly("Export_v2.sqlite".as_ref())?;
let mut graph = load_drawing_graph(&conn, "D9635C3C898840D1990B7E8BEE1D55DA")?;

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

13 类 PID tag 已实现 writer arm，在 A01 reference 上接口集 + 属性集字节级对齐；
PIDProcessVessel 已支持 A01 "Horizontal Drum"（15 接口）和 DWG "Open top tank"
（17 接口）两种 variant；A01 vs DWG 跨 fixture 的 15 项已知 IObject /
loader-side 富化列差异已在 whitelist 文档化。任何未来 SmartPlant 端漂移
会以 `(tag, interface, attr)` 三元组在 CI 上失败定位。

未支持 backlog tag：PIDBranchPoint × 5 + PIDPipingBranchPoint × 4（DWG 实例数；
spec 已在 A28 snapshot 中 pin），实施依赖 DWG plant SQLite mirror 进入仓库。

