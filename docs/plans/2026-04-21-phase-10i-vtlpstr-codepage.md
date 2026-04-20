# 开发计划：Phase 10i — VT_LPSTR 多 code page 回退

> 起稿：2026-04-21
> 目标版本：v0.8.0（minor bump）
> 起点：Phase 10g (v0.7.0) 把 VT_LPSTR 从 ASCII-only 放宽到 UTF-8 单 code page
> 结束前置：Phase 10h ship v0.7.1（session archive）
> 估计工时：3-5 hr

## 动机

Phase 10g (v0.7.0) 兑现了 Phase 9l 的 "tracked for UTF-8 support" 承诺，让 VT_LPSTR 字段接受任意 UTF-8 字节。但**现实 SmartPlant fixture 并非都是 UTF-8**：

| SmartPlant 版本 | 常见 VT_LPSTR 编码 |
|---|---|
| ≥ 2020（现代） | UTF-8 |
| 2015-2019（简中客户 fixture） | GBK / CP936 |
| 2015-2019（西欧客户 fixture） | CP1252 / Windows-1252 |
| 2015-2019（日本客户 fixture） | Shift-JIS / CP932 |

当前 (v0.7.0) 把非 UTF-8 字节喂给 reader 会得到 `from_utf8_lossy` 的替代字符（�），喂给 writer 会把字符串强制 UTF-8 编码，和源文件字节布局不一致。

Phase 10g CHANGELOG 明确列了这条已知限制：

> 仅 UTF-8。如 fixture 下游 consumer 需要 CP1252 / GBK / Shift-JIS 等传统 code page，本版本不自动识别/转换。

Phase 10i 兑现：引入 code page 探测 + 显式 encoding 声明，让 reader 可解 / writer 可写非 UTF-8 VT_LPSTR。

## 非目标

- **不改** VT_LPWSTR 行为（UTF-16LE 本就标准，无 code page 问题）
- **不改** FILETIME / I4 / 其他 VT 类型
- **不改** `summary_updates` 现有 JSON 形态的向后兼容（old input 仍能 parse）
- **不做** 自动 code page detection 的 ICU 级能力（本 Phase 用启发式白名单；完整自动化留给未来）
- **不接** 用户自定义任意 encoding 名字（仅支持 `encoding_rs` crate 已注册的 label）

## 范围

| 文件 | 改动 | 行数（估） |
|---|---|---|
| `Cargo.toml` | version 0.7.1 → 0.8.0 | ±1 |
| `src/writer/summary_write.rs` | `encode_string` VT_LPSTR 分支接 `encoding` 参数；`decode_lpstr_with_encoding` reader helper | +120 |
| `src/writer/plan.rs` | `MetadataUpdates.summary_updates` value type 从 `String` 改为 `SummaryValue`（枚举，向后兼容 serde）| +80 |
| `src/streams/summary.rs` | VT_LPSTR reader 端探测 code page（启发式 + 白名单）| +60 |
| `src/bin/pid_writer_validate.rs` | `--set-summary KEY=VALUE` 新增 `--set-summary-encoded KEY:ENCODING=VALUE` 变体 | +40 |
| `tests/writer_roundtrip.rs` | CP1252 / GBK / Shift-JIS round-trip 3 条 | +150 |
| `tests/writer_validate_cli.rs` | CLI `--set-summary-encoded` 2 条 | +80 |
| `CHANGELOG.md` | `[0.8.0]` 段 | +60 |
| `docs/writer-quickstart.md` | 新 5.7 节 "VT_LPSTR 多 code page" | +50 |
| **本 plan** | | +本文件 |

~650 行，核心逻辑约 300 行。

## 具体设计

### 1. Reader 端 code page 探测

**策略**（fallback 链，第一命中即用）：

```
1. Fixture 白名单（按 /\x05SummaryInformation 前 32B fingerprint 或 AppName prop）
   - SmartPlantPID.a 090000.* → UTF-8（2020+）
   - SmartPlantPID.a 080000.* → CP1252（2016 国际版）
   - SmartPlantPID.a 075000.* → GBK（2015 中文版，若 fixture 已收录）
2. BOM 检测（罕见于 VT_LPSTR，但做一遍）
3. 字节频次启发式：
   - 0x80-0xFF 双字节 pair 占比 > 30% → GBK / Shift-JIS（再看 lead byte 范围细分）
   - 0xA0-0xFF 单字节占比 > 5% + 无双字节序列 → CP1252
   - 全 ASCII → UTF-8（空集合兼容）
4. Fallback UTF-8（现有行为）
```

白名单优先因为 BOM / 启发式对短字符串不可靠，而 SmartPlant AppName prop 恰好是 SummaryInformation 的第一个 VT_LPSTR prop，解析它本身用 ASCII 子集就能命中。

**实现**：

```rust
// src/streams/summary.rs
pub(crate) enum LpstrCodePage {
    Utf8,
    Cp1252,
    Gbk,
    ShiftJis,
}

pub(crate) fn detect_lpstr_codepage(
    stream_prefix: &[u8],  // first ~64B of \x05SummaryInformation
    app_name_hint: Option<&str>,
) -> LpstrCodePage { ... }
```

解出的 `LpstrCodePage` 挂到 `PidPackage.streams.get("/\x05SummaryInformation").codepage_hint`（新字段）供 writer 端引用。

### 2. Writer 端 `SummaryValue` 类型

```rust
// src/writer/plan.rs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SummaryValue {
    /// 向后兼容：纯字符串等价于 `SummaryValue::Encoded { value, encoding: None }`
    Plain(String),
    Encoded {
        value: String,
        /// 可选 `encoding_rs` label（"UTF-8" / "windows-1252" / "GBK" / "Shift_JIS"）；
        /// `None` = 沿用 package 的 codepage_hint（或 UTF-8 fallback）
        encoding: Option<String>,
    },
}

pub struct MetadataUpdates {
    // ...
    pub summary_updates: BTreeMap<String, SummaryValue>,
}
```

**JSON 向后兼容验证**：
- `{"title": "X"}` 反序列化 → `SummaryValue::Plain("X")` ✓
- `{"title": {"value": "Ø Pipe", "encoding": "windows-1252"}}` → `Encoded { ... }` ✓
- serde `untagged` 让两种形态都 parse 成功

### 3. `encode_string` VT_LPSTR 分支改写

```rust
// src/writer/summary_write.rs
fn encode_string(
    vt: PropertyType,
    value: &str,
    encoding: &encoding_rs::Encoding,  // 新参数
    // ...
) -> Result<Vec<u8>, PidError> {
    match vt {
        PropertyType::VT_LPSTR => {
            let (bytes, _encoding_used, had_errors) = encoding.encode(value);
            if had_errors {
                return Err(PidError::ParseFailure {
                    context: "summary writer",
                    message: format!(
                        "value contains characters that cannot be encoded as {}: {:?}",
                        encoding.name(), value
                    ),
                });
            }
            // char_count = byte count + NUL terminator，和 Phase 10g 一致
            // ...
        }
        PropertyType::VT_LPWSTR => { /* 不变 */ }
        // ...
    }
}
```

**关键**：`had_errors == true` 时 fail-fast 而不是 silent lossy。这保证"CP1252 拒收中文"而不是拿 `?` 替代。

### 4. CLI 便捷 flag

```
--set-summary KEY=VALUE              # 用 package.codepage_hint（或 UTF-8 fallback）
--set-summary-encoded KEY:ENCODING=VALUE  # 显式指定 encoding label
```

示例：

```bash
pid_writer_validate in.pid \
    --set-summary-encoded "title:windows-1252=Ø Pipe" \
    --out out.pid
```

### 5. 错误消息改进

- `PidError::ParseFailure { context: "summary writer", message: "value '{}' contains characters not representable in encoding '{}'; try {} or a wider encoding" }`
- 推荐 encoding 的启发式：遇中文推荐 GBK / UTF-8，遇日文推荐 Shift_JIS / UTF-8，遇西欧 Latin 推荐 UTF-8

## 实施步骤

### W1 — 加 `encoding_rs` 到现有 deps，加 `SummaryValue` 枚举（~45min）

- `Cargo.toml` 已有 `encoding_rs = "0.8"`（当前是 UTF-8-only 使用），复用
- 写 `SummaryValue` 类型 + serde `untagged` 测试
- 验证旧 JSON `{"title": "X"}` 100% 向后兼容

### W2 — Reader 端 codepage 探测（~60min）

- `detect_lpstr_codepage` 纯函数
- 单测：UTF-8 / CP1252 / GBK 各一条合成 fixture
- `PidPackage.streams[<summary_path>]` 加 `codepage_hint` 字段（optional）

### W3 — Writer 端 `encode_string` 改写 + `apply_summary_updates` 串联（~75min）

- `encode_string` 增加 `&encoding_rs::Encoding` 参数
- `apply_summary_updates` resolve encoding：显式 override > package hint > UTF-8 fallback
- 单测 7-8 条（每种编码的 happy path + lossy fail-fast）

### W4 — CLI `--set-summary-encoded` + 集成测试（~45min）

- `pid_writer_validate` 新 flag parser
- 集成测试 2 条（CP1252 happy path + lossy 拒绝 exit 2）

### W5 — 真实 fixture 测试（~30min，依赖 fixture 是否可得）

- 若 repo 有 GBK / CP1252 真实 fixture：加 `tests/writer_real_files.rs` 条件测试
- 若缺：在 plan 里明确 "留 Phase 10i+ 完善"，合成 fixture 兜底

### W6 — docs + CHANGELOG（~30min）

- `docs/writer-quickstart.md` 新 5.7 节
- `CHANGELOG.md` `[0.8.0]` 段

### W7 — ship v0.8.0

- `cargo fmt --check && cargo clippy -D warnings && cargo test --all-targets` → 预期 340+ tests（从 332 加 8-10）
- `git commit -m "feat(writer): v0.8.0 — VT_LPSTR multi code page support (Phase 10i)"`
- `git tag v0.8.0`

## 预计工时

- W1 45min
- W2 60min
- W3 75min
- W4 45min
- W5 30min（条件）
- W6 30min
- W7 10min
- **合计 4.0-4.5 hr**（预留 30min buffer = 3-5 hr 区间）

## 验证清单

- [ ] `{"title": "X"}` 旧 JSON 格式 100% 向后兼容
- [ ] `{"title": {"value": "Ø", "encoding": "windows-1252"}}` 新格式写入 CP1252 字节正确
- [ ] `{"title": {"value": "中文", "encoding": "windows-1252"}}` 返回 fail-fast 错误（不是 silent mojibake）
- [ ] Reader 端探测 3 种 code page，各一个合成 fixture 回归
- [ ] `--set-summary-encoded KEY:ENCODING=VALUE` CLI 工作
- [ ] test count: 332 → 340+
- [ ] clippy / fmt 双零
- [ ] `CHANGELOG.md` [0.8.0] 段完整

## 风险缓解

| 风险 | 缓解 |
|---|---|
| 单字节 code page 区分难（CP1252 vs CP1250 vs Latin-1）| 启发式只返回 `Cp1252`；用户显式 override 优先 |
| GBK 字符串和 UTF-8 在某些短字符串上无法区分 | 白名单（按 AppName）优先；启发式仅作 fallback |
| encoding_rs 对 Shift_JIS 有多变体 | 统一用 `encoding_rs::SHIFT_JIS` 即 CP932 |
| 旧 consumer 代码 `let v: &String = updates.get("title")` 会 break | `SummaryValue` 实现 `From<String>` + `impl SummaryValue::as_plain_str() -> &str`；doc 里写迁移提示 |
| SemVer 判定有争议 | 新字段形态可 deserialize 旧输入，是 additive；但 Rust API type 变了，按保守 minor bump |

## SemVer 判定

- 旧 `BTreeMap<String, String>` consumer：**API 破坏**（type 变成 `BTreeMap<String, SummaryValue>`）
- 旧 JSON 输入：向后兼容（serde untagged）
- 编码失败路径从 `Err(ASCII non-ASCII)` 变为 `Err(not representable in encoding)`：错误消息变化，patch 级
- 新 encoding label 支持：纯 additive

综合：**minor bump 0.7 → 0.8.0**（API type 改变 + 新 encoding 能力）。未 release 到 crates.io 的话也可考虑 major bump 0.x → 1.0；暂定 minor 保守。

## Next 候选（10i 完成后）

- **Phase 10j**：DocumentSummaryInformation section 2（user-defined dict）编辑
- **Phase 11a**：PSMclustertable per-record 结构化（parser 深化）

## 交叉引用

- 上游总 roadmap：`docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` 阶段 B
- 前置 Phase：`docs/plans/2026-04-21-phase-10g-lpstr-utf8.md`（UTF-8 基础）
- SPPID 战略：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` Phase 2 范围外（Writer 边界打磨）
