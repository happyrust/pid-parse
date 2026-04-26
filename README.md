# pid-parse

[![CI](https://github.com/happyrust/pid-parse/actions/workflows/ci.yml/badge.svg)](https://github.com/happyrust/pid-parse/actions/workflows/ci.yml)

`pid-parse` 是一个 Rust 编写的 SmartPlant / Smart P&ID `.pid` 文件解析器。

## 文档与产品现状

- [`docs/prd-pid-parse-current-state.md`](docs/prd-pid-parse-current-state.md) — **当前状态 PRD**：解析现状、能力分层、缺口与下一阶段需求（P0–P5）。新读者建议先看此文档对齐边界。
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
- **Publish Data XML writer**（Stage-1，A12 → A39）：通过本地克隆的 `vendor/oxidized-mdf` 直接读取 MDF 内的 `T_Drawing` / `T_ModelItem` / `T_Representation` / `T_Relationship` 等表，发出 SmartPlant 兼容的 `_Data.xml` + `_Meta.xml`。15 类 PID tag 已支持（PIDBranchPoint / PIDControlSystemFunction / PIDDrawing / PIDNote / PIDNozzle / PIDPipeline / PIDPipingBranchPoint / PIDPipingComponent / PIDPipingPort / PIDProcessPoint / PIDProcessVessel / PIDRepresentation / PIDSignalConnector / PIDSignalPort）；A01 reference 上共享 tag 的接口集 / 属性集继续由 A23 / A27 守门，DWG reference 的 writer coverage 已达 `108/108`
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
# 1. 从 SmartPlant 备份（Export.dmp）剥离 MTF 头得到 .mdf
cargo run --bin pid_backup_extract -- Export.dmp --out Export.mdf

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

# 6. 跟 SmartPlant 参考 _Data.xml 跑 SemanticDiff（CI gate）
cargo run --bin pid_publish_xml -- Export.mdf \
    --drawing D9635C3C898840D1990B7E8BEE1D55DA \
    --plant TEST02 --diff-against reference/A01_Data.xml
```

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

15 类 PID tag 已实现 writer arm；DWG reference 已达到 `108/108` writer coverage，
`PIDBranchPoint × 5` 与 `PIDPipingBranchPoint × 4` 已从 backlog snapshot 毕业到
supported set。PIDProcessVessel 已支持 A01 "Horizontal Drum"（15 接口）和
DWG "Open top tank"（17 接口）两种 variant；当前剩余缺口集中在 mirror-gated
loader fidelity：A24 仍保留 `PIDProcessVessel` 的 tank variant 单项接口白名单，
A27b 仍保留 15 条 DWG-only style / canonical-field enrichment 差异。任何未来
SmartPlant 端漂移会以 `(tag, interface, attr)` 三元组在 CI 上失败定位。

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

