# Writer 快速入门

本页演示 `pid-parse` v0.3.2+ Writer 链路的典型使用姿势：**parse → edit → write → verify**。覆盖率换算成代码大约 20-30 行。

## 0. 前提

- `pid-parse = "0.3.5"`（writer 链路稳定）
- 输入是 SmartPlant / Smart P&ID 产出的 `.pid` 文件
- 下游消费者（SPPID 加载器、SPPIDReader 等）能接受 "相同 stream 字节 + 相同 root CLSID" 的文件（时间戳会刷新，详见 `writer-clsid-and-timestamps.md`）

## 1. Passthrough round-trip：不改任何内容，零差异写回

最小验证 writer 是否可用。

```rust
use pid_parse::{PidParser, PidWriter, WritePlan, diff_packages};

fn main() -> Result<(), pid_parse::PidError> {
    let parser = PidParser::new();
    let pkg = parser.parse_package("drawing.pid")?;

    PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new("out.pid"))?;

    // 回读验证
    let pkg_back = parser.parse_package("out.pid")?;
    let diff = diff_packages(&pkg, &pkg_back);
    assert!(diff.is_empty(), "{} diff(s)", diff.diff_count());
    Ok(())
}
```

或者从 CLI：

```bash
pid_inspect drawing.pid --round-trip out.pid --verify
# → round-trip ok: drawing.pid -> out.pid
#   streams written: 69
#   root CLSID preserved: {16ce6023-5f5b-11d1-9777-08003655f302}
#   verified: 0 diffs
```

## 2. 改 drawing number / project：metadata-only 更新

`PidPackage::set_xml_tag` 精确替换一个简单 tag 的文本内容，其他所有流字节保持不变。

```rust
use pid_parse::{PidParser, PidWriter, WritePlan};

let parser = PidParser::new();
let mut pkg = parser.parse_package("drawing.pid")?;

let old = pkg.set_drawing_xml_tag("DrawingNumber", "NEW-001")?;
println!("changed DrawingNumber: {old} -> NEW-001");

PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new("out.pid"))?;
```

CLI 便捷命令：

```bash
pid_inspect drawing.pid --set-drawing-number NEW-001 --output out.pid
# set-drawing-number ok: DrawingNumber "DWG-0201GP06-01" -> "NEW-001"  (...)
```

通用版（任意 `/TaggedTxtData/*` 里任意 tag）：

```bash
pid_inspect drawing.pid \
    --set-xml-tag /TaggedTxtData/Drawing Template NEW_TEMPLATE.pid \
    --output out.pid
```

## 3. 声明式写计划：一次多项修改

当你想在一次写入里做多种修改时，构造一个完整 `WritePlan`：

```rust
use pid_parse::{MetadataUpdates, PidParser, PidWriter, WritePlan};

let parser = PidParser::new();
let mut pkg = parser.parse_package("drawing.pid")?;

// 先用 set_xml_tag 预处理 xml 字段（返回新字节已写入 streams）
pkg.set_drawing_xml_tag("DrawingNumber", "NEW-001")?;
pkg.set_drawing_xml_tag("Template", "NEW_TEMPLATE.pid")?;

// 其它任意 stream 替换
// pkg.replace_stream("/MyCustom/Blob", vec![1, 2, 3, 4]);

PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new("out.pid"))?;
```

或者用 `WritePlan` 直接指定：

```rust
let plan = WritePlan {
    metadata_updates: MetadataUpdates {
        drawing_xml: Some(new_drawing_xml),  // 整个流替换
        general_xml: None,
        summary_updates: Default::default(), // 预留，第一版不写
    },
    stream_replacements: vec![],   // 任意流替换
    sheet_patches: vec![],          // experimental byte-range splice
};
PidWriter::write_to(&pkg, &plan, out)?;
```

两种风格可混用：`set_xml_tag` 针对"改一小段文本"，`WritePlan` 针对"整个流替换 + 二进制补丁"。

## 4. 对比 / 验证：diff_packages

写出去后怎么知道"只动了想动的地方"？`diff_packages` 提供 stream 级字节 diff。

```rust
use pid_parse::diff_packages;

let before = parser.parse_package("drawing.pid")?;
let after = parser.parse_package("out.pid")?;
let diff = diff_packages(&before, &after);

if diff.is_empty() {
    println!("no changes");
} else {
    println!("{}", pid_parse::inspect::diff::render(&diff));
}
```

CLI：

```bash
pid_inspect a.pid --diff b.pid
# === Package Diff ===
# root CLSID:  match  (a={...}, b={...})
# summary:     1 diff(s) — 0 only-in-a / 0 only-in-b / 1 modified
#
# --- Modified Streams ---
#   /TaggedTxtData/Drawing  len=3619 vs 3611  first_diff@0x4D0
#     a: 44 57 47 2d 30 32 30 31 ...    ("DWG-0201...")
#     b: 4e 55 4d 2d 4e 45 57 3c ...    ("NUM-NEW<")
```

## 5. Experimental：Sheet byte-range 补丁

Sheet 流的几何数据尚未语义解码。如果你已经通过 RE 拿到确切的 byte-range 要改写，可以走 `SheetPatch`（**不做**语义验证）：

```rust
use pid_parse::writer::{SheetChunkPatch, SheetPatch};
use pid_parse::{PidParser, PidWriter, WritePlan};

let parser = PidParser::new();
let pkg = parser.parse_package("drawing.pid")?;

let plan = WritePlan {
    sheet_patches: vec![SheetPatch {
        sheet_path: "/Sheet6".to_string(),
        chunk_patches: vec![
            SheetChunkPatch {
                start: 0x100,
                end: 0x104,
                replacement: vec![0xAA, 0xBB, 0xCC, 0xDD],
            },
        ],
        experimental: true, // 明确确认这是实验性用法
    }],
    ..Default::default()
};
PidWriter::write_to(&pkg, &plan, out)?;
```

详细语义和多补丁的顺序保障见 `src/writer/sheet_patch.rs`。

## 6. 保真能力矩阵

Root CLSID 保留 ✅ ；非 root storage CLSID / 时间戳 / state_bits / 目录物理顺序 ❌。详见 `writer-clsid-and-timestamps.md`。

## 7. 错误处理

所有公开 API 返回 `Result<_, pid_parse::PidError>`。常见变体：

- `PidError::MissingStream(path)`：目标流不存在（set_xml_tag / sheet_patch）
- `PidError::ParseFailure { context, message }`：XML 编辑失败（缺 tag、nested tag、非 UTF-8）或 sheet_patch 越界
- `PidError::Io`：读写文件的 I/O 错误
