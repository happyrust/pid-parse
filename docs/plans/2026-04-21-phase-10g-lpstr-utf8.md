# 开发计划：Phase 10g — VT_LPSTR UTF-8 支持

> 起稿：2026-04-21
> 背景：Phase 9l (v0.5.0) 发布 SummaryInformation property-set writer
> 时，对 `VT_LPSTR` 字段做了 ASCII-only gate：非 ASCII 值会被 reject
> 并提示 "tracked for Phase 9m UTF-8 support"。Phase 9m/9n 都没动到
> 这一条，一直作为 "known limitation" 挂在 CHANGELOG 里。本轮兑现。
> 目标：v0.7.0 minor ship（语义放宽：之前报错的 case 现在成功）。

## 动机

- SmartPlant 真实 fixture 里 `VT_LPSTR` 字段**常见非 ASCII**（UTF-8
  中文 title / author）。目前 pid-parse 只能更新 VT_LPWSTR 对应的
  prop；VT_LPSTR 即使是标准 OLE PropSet 常用类型也被 writer 拒绝。
- Phase 9l 的 note 已经帮用户写好了升级 roadmap 路径，现在补齐。
- Phase 10f 刚 ship byte-dimension，session 回到 Writer 层做一个
  小结语义层级的 extension 是合理收束。

## 非目标

- 不做 CP1252 / GBK / Shift-JIS 等传统 code page
- 不改 VT_LPWSTR 行为
- 不变 `summary_updates` 的 API 面（还是 `BTreeMap<String, String>`）
- 不动 reader 端 `parse_property_set` 的 ASCII→UTF-16LE 转换——reader
  已经是 lossy-utf8 默认行为，无需变

## 范围

| 文件 | 改动 | 行数 |
|---|---|---|
| `src/writer/summary_write.rs::encode_string` | `VT_LPSTR` arm 接受 UTF-8 编码 | +20 |
| `src/writer/summary_write.rs::tests` | 更新 `encode_lpstr_rejects_non_ascii` 变为 `encode_lpstr_accepts_utf8`，新增 round-trip 测试 | +70 |
| `CHANGELOG.md` | `[0.7.0]` | +30 |
| `Cargo.toml` | 0.6.5 → 0.7.0 | ±1 |
| `src/writer/plan.rs` 文档 | 去掉 `summary_updates` 注释里 "non-ASCII rejected" 描述 | ±3 |
| **本 plan** | | +本文件 |

~120 行。

## 关键决策

### A. VT_LPSTR 编码策略：UTF-8 bytes

OLE spec 说 VT_LPSTR 是 "NUL-terminated single-byte character string
in the encoding of the OS code page"。在 Windows 上 OS code page 通常
是 UTF-8（Win10+ 可启用 UTF-8 as system code page）或 CP1252（西欧
默认）或其他。我们无法可靠推断 fixture 的 code page，但：

- **现代 SmartPlant fixture 实测多为 UTF-8**（读端的 `parse_property_set`
  对 bytes 直接尝试 `String::from_utf8_lossy`，保留 bytes 原样）
- **CP1252 合法 UTF-8 子集**：ASCII 0x00-0x7F 部分 CP1252 和 UTF-8
  完全一致；只有 0x80-0xFF 不同。所以如果 fixture 是 CP1252 而
  pid-parse 用 UTF-8 写入，对 0x80+ 的字节**会产生不同字节序列**，
  下游 SmartPlant 再读时看到中文 e.g. 是乱码

**决策**：v0.7.0 只支持 UTF-8，文档里明确"如果下游要求 CP1252 请
保留原值或切到 VT_LPWSTR"。Phase 10h 视需再加 code page 识别。

### B. 新 encoding 规则

```rust
VT_LPSTR => {
    // Phase 10g: accept any UTF-8 string. NUL-terminated bytes +
    // 4-byte alignment. If the fixture's consumers require CP1252
    // encoding, use a VT_LPWSTR property instead or pre-encode the
    // value externally (out of scope for v0.7.0).
    let mut bytes = value.as_bytes().to_vec();
    bytes.push(0);
    // NOTE: char_count is BYTE count for LPSTR per [MS-OLEPS], not
    // Unicode code point count; this matches the parse side.
    ...
}
```

去掉 ASCII 校验，保持 byte-length + 4-byte pad 不变。

### C. `encode_lpstr_rejects_non_ascii` 改名 + 重写

原测试变为 `encode_lpstr_accepts_utf8`，断言：
- "公司 Co." 能 encode
- decoded UTF-8 匹配原字符串
- 字节长度 = utf8_byte_count + 1 (NUL) + pad_to_4

新增 `encode_lpstr_roundtrip_through_property_set`：
- 构造 fixture bytes，prop 是 LPSTR "Old Title"
- set_string "新标题 中文"
- parse 回，assert 能解码回 "新标题 中文"

## 实施步骤

### W1 — 改 `encode_string` VT_LPSTR arm

### W2 — 测试更新

### W3 — 文档 + ship v0.7.0

## 预计工时

- W1: 15 min
- W2: 40 min
- W3: 15 min
- **合计 ~1 hr**

## 验证清单

- [ ] fmt/clippy/test 全 0
- [ ] test count 332 → 334+
- [ ] Cargo.toml 0.7.0

## Next 候选

- **Phase 10h**: CP1252 fallback + encoding auto-detection
- **Phase 10i**: DocumentSummaryInformation section 2 编辑
  （Phase 9n parked）
- **Phase 11a**: roadmap Phase 3 规范化语义图层（大 Phase）
