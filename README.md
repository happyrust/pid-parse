# pid-parse

`pid-parse` 是一个 Rust 编写的 SmartPlant / Smart P&ID `.pid` 文件解析器。

## 功能

- **容器解析**：OLE/CFBF 复合文件遍历与流索引
- **元数据提取**：OLE Summary (应用名/作者/创建时间) + TaggedTxtData XML (图号/模板/项目)
- **对象索引**：JSite 存储解析、符号路径提取、GUID 扫描
- **二进制记录解码**：Cluster 公共头、PSMcluster0 字符串表、动态属性结构化记录
- **Sheet 流探测**：复用 Cluster 公共头 + 0x89 标记扫描（v0.2.2 新增）
- **Magic 识别**：自动识别 PSMroots / PSMclustertable / PSMsegmenttable / DocVersion 等顶层结构化流（v0.2.2 新增）
- **P&ID 对象清单**：从动态属性记录中提取设备/管道/仪表统计
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

--- Top-level Unidentified Streams ---
  /PSMroots (278 bytes, magic=0x746F6F72 'root' [PSMroots root table])
  /PSMclustertable (265 bytes, magic=0x74736C63 'clst' [PSMclustertable index])
  /PSMsegmenttable (12 bytes, magic=0x62617473 'stab' [PSMsegmenttable index])
  /DocVersion3 (192 bytes, magic=0x72616D53 'Smar' [DocVersion (SmartPlant)])

--- P&ID Object Inventory ---
  Project: SQLPlant1401
  Total items: 140
    PipeRun: 53
    Relationship: 64
    Nozzle: 6
    Instrument: 3
```
