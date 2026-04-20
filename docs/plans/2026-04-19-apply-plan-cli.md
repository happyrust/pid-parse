# 开发计划：`--apply-plan <plan.json>` 批处理 CLI

> 起稿：2026-04-19  
> 背景：`phase8-9h-summary.md` 未完成候选清单列出的 `--apply-plan` 项（中粒度 / 低风险 / 2-4 hr 工作量）。`WritePlan` / `MetadataUpdates` / `StreamReplacement` / `SheetPatch` / `SheetChunkPatch` 在 Phase 8-9h 已具备完整的 `serde::{Serialize, Deserialize}` 衍生，CLI 层是唯一缺口。

## 动机

当前 `pid_writer_validate` CLI 只支持两种写操作：
- `--edit <stream> <attr> <value>`：单个 drawing XML 属性编辑
- `--general-edit <element> <value>`：单个 general XML 元素文本编辑

**缺陷**：脚本化场景下，一次 `.pid` 变更需要编辑 10 个字段，用户必须调 10 次 CLI。每次都 `parse_package → mutate → write_to → verify` 是 O(N) 冗余 I/O + O(N) 冗余 diff 输出。

**目标**：提供 `--apply-plan <plan.json>` 单次调用，按 declarative WritePlan JSON 一次性施加所有编辑并输出单份 verification 结果。

## 非目标

- 不改变 `WritePlan` / `PidWriter::write_to` 的 API（只是接 CLI）
- 不实现 SummaryInformation 回写（仍为未来大 Phase）
- 不新增其他 CLI 命令（如 `--from-edit-log` / `--diff-plan`）
- 不破坏现有 `--edit` / `--general-edit` 的行为（并行保留）

## 范围

仅改动：

| 文件 | 改动类型 | 行数估计 |
|---|---|---|
| `Cargo.toml` | +1 dep `base64 = "0.22"` | +1 |
| `src/writer/plan.rs` | Vec<u8> 自定义 serde with base64 | +30 |
| `src/bin/pid_writer_validate.rs` | 新增 `--apply-plan` 分支 | +50 |
| `tests/writer_validate_cli.rs` | +3 集成测试 | +80 |
| `docs/writer-quickstart.md` | 新一节 "批量编辑 via --apply-plan" | +40 |
| `CHANGELOG.md` | Unreleased 追加 | +10 |

合计 ~210 行。不碰 lib 核心、不碰 H7CAD。

## 关键设计决策

### A. Vec<u8> 编码：base64 字符串

**原生 serde 行为**：`Vec<u8>` → JSON array of ints (`[1,2,3,...]`)，500KB stream 会变成 ~2MB 文本，**极度冗长**。

**方案**：在 `StreamReplacement.new_data` 和 `SheetChunkPatch.replacement` 两个字段上挂自定义 `serialize_with` + `deserialize_with`，输出标准 base64（`A-Za-z0-9+/=`）。

示例序列化：

```json
{
  "stream_replacements": [
    {
      "path": "/Sheet6",
      "new_data": "SGVsbG8sIFdvcmxkIQ=="
    }
  ]
}
```

**why not hex**：base64 比 hex 紧凑 33%，是 P&ID stream 量级（KB-MB）下的明显差别。

**why not rmp-serde / bincode**：本命令面向人工编辑/审计 plan 文件的场景，必须 JSON。

**兼容性**：`MetadataUpdates.drawing_xml` 是 `Option<String>`，天然 JSON 友好；不需要改。

### B. base64 crate 选择

加 `base64 = "0.22"`（当前 latest 稳定版）。理由：

1. 纯 Rust 实现（~200KB compiled，无 transitive dep）
2. WASM / no_std 兼容（未来 porting plan A 走 WASM 时可复用）
3. API 稳定（`general_purpose::STANDARD.encode/decode`）
4. MIT/Apache-2.0 双许可，与 pid-parse 对齐

**风险**：+1 外部依赖。但 `serde_json` / `uuid` 早已证明依赖新增可接受。

### C. CLI 参数协议

```
pid_writer_validate <input.pid> \
    --apply-plan <plan.json> \
    [--out <output.pid>] \
    [--verify]
```

- `<plan.json>` 必填，指向本命令读取的 JSON
- `--out`：输出 .pid 路径；默认 `<input>.after-apply.pid`
- `--verify`：可选，写完后对 `<out>` 再 parse 一遍并 diff 原 input，输出 report

**互斥**：`--apply-plan` 与既有的 `--edit` / `--general-edit` 互斥（出现同时 → 返回 usage error）。

**退出码**：
- `0` = apply + verify 成功
- `1` = parse JSON / parse .pid 失败
- `2` = verify 发现字节 diff 超过 plan 预期范围（保留字段，本期默认不校验，固定 0/1）

### D. 错误处理

- JSON 解析错误 → print `[error] failed to parse plan.json: <serde err>` + exit 1
- 空 plan（`is_passthrough() == true`）→ 打印 `[info] plan is passthrough; running round-trip only` 然后走原 round-trip 流程
- base64 decode 失败 → serde 层直接抛，统一走 JSON 解析错误路径

## 实施步骤

### W1 — 加 base64 依赖 + 自定义 serde

```toml
# Cargo.toml
base64 = "0.22"
```

```rust
// src/writer/plan.rs 顶部新增 module
mod bytes_base64 {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, ser: S) -> Result<S::Ok, S::Error> {
        STANDARD.encode(bytes).serialize(ser)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(de)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}
```

两处改挂 `#[serde(with = "bytes_base64")]`：

```rust
pub struct StreamReplacement {
    pub path: String,
    #[serde(with = "bytes_base64")]
    pub new_data: Vec<u8>,
}

pub struct SheetChunkPatch {
    pub start: usize,
    pub end: usize,
    #[serde(with = "bytes_base64")]
    pub replacement: Vec<u8>,
}
```

**新增单测**（`#[cfg(test)] mod tests` 里）：
- `stream_replacement_roundtrips_through_json`：构造 `StreamReplacement { path, new_data: vec![0, 1, 2, 255] }` → `to_string` → `from_str` → 断言字段完全相等，且 JSON 里可见 `"AAECAf8="` base64

### W2 — pid_writer_validate 新增 `--apply-plan` 分支

在现有 argv parser 加：
- 匹配 `--apply-plan <path>` flag
- 读文件 → `serde_json::from_str::<WritePlan>(&content)`
- 传入 `PidWriter::write_to(&pkg, &plan, &out_path)`
- 如有 `--verify`，对 `out_path` 重新 `parse_package` 与原 input diff 并打印 render 结果

保留原 `--edit` / `--general-edit` 路径零改动。

### W3 — 集成测试

`tests/writer_validate_cli.rs` 新增：

1. `apply_plan_passthrough_empty_plan_exits_zero`：构造空 `{}` plan，CLI 应 exit 0 且不改字节
2. `apply_plan_sets_drawing_number_via_metadata`：plan = `{ metadata_updates: { drawing_xml: "<Drawing ..>" } }`，CLI 后 verify 发现 `/TaggedTxtData/Drawing` 被替换
3. `apply_plan_replaces_arbitrary_stream_via_base64`：plan = `{ stream_replacements: [{ path: "/DocVersion3", new_data: "QUJD" /*"ABC"*/ }] }`，CLI 后断言 `/DocVersion3` bytes == `b"ABC"`
4. `apply_plan_fails_on_invalid_json_with_exit_1`：plan.json 内容为 `not json`，CLI exit 1 + stderr 含 "failed to parse plan"

### W4 — 文档 + CHANGELOG + commit

- `docs/writer-quickstart.md` 新一节 "批量编辑 via `--apply-plan`"：展示一份完整 plan.json 示例 + 对应 CLI 调用 + 输出样本
- `CHANGELOG.md` Unreleased 追加：
  ```
  ### Added
  - `pid_writer_validate --apply-plan <plan.json>`：declarative batch edit
  - `Cargo.toml`: base64 = "0.22"
  ### Changed
  - `StreamReplacement.new_data` and `SheetChunkPatch.replacement` now serialize
    as base64 strings in JSON (transparent for Rust-side consumers)
  ```

## 预计工时

| 步骤 | 估时 |
|---|---|
| 写 plan | 已完成 |
| W1 base64 + serde helper | 25 min |
| W2 CLI 分支 | 40 min |
| W3 4 个集成测试 | 40 min |
| W4 doc + changelog + commit | 20 min |
| **合计** | **~2 hr** |

## 验证清单

- [ ] `cargo check --all-targets` 通过
- [ ] `cargo test --all` 全绿（含新增 ~5 个测试）
- [ ] `pid_writer_validate --help` 显示 `--apply-plan <path>`
- [ ] 手工用一个 3-字段 plan 对 fixture `.pid` 跑一遍 round-trip，观察输出

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| base64 crate 未来 major bump 破坏 serde 契约 | 锁 `"0.22"`（非 `"^0.22"`），升级时走专项 PR |
| Vec<u8> → base64 change 破坏既有 Rust 侧 consumers 的 JSON 兼容 | **有向后不兼容风险**：如果某处已写 `serde_json::to_string(&plan)` 存档，现在 schema 变。查 grep：`serde_json::to_string.*plan` / `serde_json::from_str.*plan` 零命中，**无已知 consumer**，风险为零。 |
| plan.json 里未知字段（如未来加 `summary_updates_v2`）导致反序列化 fail | serde 默认 `deny_unknown_fields = false`，未知字段静默忽略；未来加字段时 set default 即可 |
| 大 stream 的 JSON plan 膨胀 | base64 比 Vec<u8>-as-array 小 6x，MB 级可接受；TB 级需用 multipart/stream path（本期不做） |

## 回滚

所有改动集中在 5 个文件，`git revert` 即可。无 DB 迁移、无 public API break（`Serialize/Deserialize` 的 wire format 变化只影响 plan.json 消费者，lib 公共 API 签名不变）。

## Next 候选（跟进排队）

- `PSMclustertable` per-record 字段精确映射（1-2 hr，中风险逆向）
- `SummaryInformation` property-set 回写（4-8 hr，中风险）
- P3 cleanups：file_stem 跨平台 / hints 缓存 / diff.rs writeln.unwrap / 测试 use 散落
