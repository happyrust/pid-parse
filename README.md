# pid-parse

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
```

## 库调用

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
