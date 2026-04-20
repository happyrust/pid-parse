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

## 5.5 批处理 via `--apply-plan <plan.json>`

当脚本化场景需要在一次调用里施加多条编辑（metadata XML + 任意 stream 替换 + 可选 sheet 补丁），把 `WritePlan` 序列化到 JSON 再交给 `pid_writer_validate --apply-plan` 最直接。

### JSON schema（关键字段）

```json
{
  "metadata_updates": {
    "drawing_xml": "<?xml version=\"1.0\"?><Drawing>...</Drawing>",
    "general_xml": null,
    "summary_updates": {}
  },
  "stream_replacements": [
    { "path": "/PlainSheet/Sheet1", "new_data": "QUJD" }
  ],
  "sheet_patches": []
}
```

要点：
- 省略字段会走默认（`{}` 是合法的 passthrough plan）
- `new_data` 和 `sheet_patches[*].chunk_patches[*].replacement` 用**标准 base64** 编码（`A-Za-z0-9+/=`），与 WebCrypto / Python `base64.b64encode` / Rust `base64::general_purpose::STANDARD` 兼容
- `metadata_updates.drawing_xml` / `general_xml` 是**整个 XML 流**的替换；如果只想改一个属性，先 parse → set_drawing_attribute → 把结果放进 plan

### CLI 调用

```bash
pid_writer_validate drawing.pid \
    --apply-plan my-edits.json \
    --out out.pid \
    --keep \
    --json
```

- `--apply-plan` 与 `--edit` / `--general-edit` **互斥**（二者语义不同，混用会 exit 1）
- `--json` 下输出结构化 `ValidateReport`，包含 `plan_applied` 字段和 `edited` / `matched` / `mismatched` 计数，适合 CI 消费

### Rust 侧：手工构造 plan 并 to_string

```rust
use pid_parse::writer::{MetadataUpdates, StreamReplacement, WritePlan};

let plan = WritePlan {
    metadata_updates: MetadataUpdates {
        drawing_xml: Some(new_drawing_xml),
        ..Default::default()
    },
    stream_replacements: vec![StreamReplacement {
        path: "/PlainSheet/Sheet1".into(),
        new_data: b"ABC".to_vec(),
    }],
    ..Default::default()
};

let json = serde_json::to_string_pretty(&plan)?;
std::fs::write("my-edits.json", &json)?;
```

`new_data` 虽然在 Rust 侧是 `Vec<u8>`，序列化到 JSON 时**自动 encode 成 base64 字符串**；反序列化也会自动 decode，对 Rust consumer 透明。

## 5.6 编辑 `SummaryInformation` / `DocumentSummaryInformation`（v0.5.0+）

Smart P&ID `.pid` 里的 `/\u{5}SummaryInformation` 和
`/\u{5}DocumentSummaryInformation` 是 **OLE 属性流**，携带 Title /
Author / Subject / Template 等标准 Office 元数据。v0.5.0 起 `WritePlan`
的 `metadata_updates.summary_updates` 字段真正生效。

### 可编辑 key（v0.5.0 起）

| key 符号名      | 目标 stream               | PROPID |
|-----------------|---------------------------|--------|
| `title`         | SummaryInformation        | 2      |
| `subject`       | SummaryInformation        | 3      |
| `author`        | SummaryInformation        | 4      |
| `keywords`      | SummaryInformation        | 5      |
| `comments`      | SummaryInformation        | 6      |
| `template`      | SummaryInformation        | 7      |
| `last_author`   | SummaryInformation        | 8      |
| `rev_number`    | SummaryInformation        | 9      |
| `app_name`      | SummaryInformation        | 18     |
| `category`      | DocumentSummaryInformation | 2      |
| `manager`       | DocumentSummaryInformation | 14     |
| `company`       | DocumentSummaryInformation | 15     |

不在表里的 key 会立刻返回错误（列出已知 key 表作为 help 文本）。

### JSON 示例

```json
{
  "metadata_updates": {
    "summary_updates": {
      "title": "Q4 Pipeline Review",
      "author": "Jane Engineer",
      "company": "ACME Inc."
    }
  }
}
```

### Rust 侧示例

```rust
use pid_parse::{MetadataUpdates, PidParser, PidWriter, WritePlan};
use std::collections::BTreeMap;

let parser = PidParser::new();
let pkg = parser.parse_package("input.pid")?;

let mut summary = BTreeMap::new();
summary.insert("title".into(), "Q4 Pipeline Review".into());
summary.insert("author".into(), "Jane Engineer".into());

let plan = WritePlan {
    metadata_updates: MetadataUpdates {
        summary_updates: summary,
        ..Default::default()
    },
    ..Default::default()
};

PidWriter::write_to(&pkg, &plan, "output.pid")?;
```

### 规则与约束

- **只改 string 类型**：source property 必须已经是 `VT_LPSTR` (0x001E)
  或 `VT_LPWSTR` (0x001F)。FILETIME / I4 等非字符串字段**拒绝写入**
  （error: `read-only VT type`），其原字节在输出里保持 byte-for-byte
  不变。
- **encoding 保持**：如果源 prop 是 `VT_LPSTR`，新值会以 ASCII 回写；
  **非 ASCII 值会被拒绝**（Phase 9m 计划扩展到 UTF-8 / CP1252）。
  源 prop 是 `VT_LPWSTR` 的话，任何 Unicode 都 OK。
- **新增 prop**：若源 section 里没有该 PROPID，会追加一条，默认用
  `VT_LPWSTR`（UTF-16LE）保证 Unicode 安全。
- **stream 必须已存在**：源 `.pid` 没有 `/\u{5}SummaryInformation` 流
  的话，会返回 `stream '/\x05SummaryInformation' does not exist` 错误；
  用户需要先走 `stream_replacements` 塞入一份基线 property-set。
- **未触的 prop 零 drift**：写回时非目标 prop 的 typed-value 原字节逐
  字节复制（含 alignment padding），因此 `diff_packages` 只会在被编辑
  的 prop 位置报差异。

### 已知局限（跟踪 Phase 9m+）

- `VT_LPSTR` 非 ASCII 不支持；
- DocumentSummaryInformation 第二个 section（user-defined dictionary）
  只做原字节透传；
- 不支持 prop 删除（未来挂 `summary_deletions` 字段）。

## 6. 保真能力矩阵

Root CLSID 保留 ✅ ；非 root storage CLSID / 时间戳 / state_bits / 目录物理顺序 ❌。详见 `writer-clsid-and-timestamps.md`。

## 7. 错误处理

所有公开 API 返回 `Result<_, pid_parse::PidError>`。常见变体：

- `PidError::MissingStream(path)`：目标流不存在（set_xml_tag / sheet_patch）
- `PidError::ParseFailure { context, message }`：XML 编辑失败（缺 tag、nested tag、非 UTF-8）或 sheet_patch 越界
- `PidError::Io`：读写文件的 I/O 错误
