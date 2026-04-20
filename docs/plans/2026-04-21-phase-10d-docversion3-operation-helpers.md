# 开发计划：Phase 10d — DocVersion3 operation 语义化 + report 渲染升级

> 起稿：2026-04-21
> 背景：roadmap Phase 2.1 关注 `DocVersion3` 结构化解析升级。现状是：
>
> - 48-byte record 的 4 个字段（product/version/operation/timestamp）
>   已解码到 `VersionRecord` 的 `String` 上
> - `operation` 只保留 raw 2-char 字符串（`"SA"` / `"SV"`），语义
>   隐藏；与 Phase 9f 的 DocVersion2 `op_type_label(0x82) = "SaveAs"` 不对称
> - `timestamp` 只保留 raw ASCII 字符串（e.g. `"12/29/25 10:45"`），
>   没做格式校验或 structured 提取
> - `generate_report` 的 `--- Version History ---` 段直接打印 raw `"SA"`，
>   可读性差
>
> Phase 10d 在**不破坏现有 serde schema** 的前提下补齐这些接口，
> 让下游消费者（H7CAD 加载器 / 报告工具）拿到结构化而非 raw 字符串。
> 目标：v0.6.3 patch ship。

## 非目标

- 不改 `VersionRecord` 字段类型（保持 `operation: String`），避免
  对 JSON / JsonSchema consumer 的 breaking change
- 不实现完整 timezone 推断
- 不改变 parser 的记录截断逻辑

## 范围

| 文件 | 改动 | 行数 |
|---|---|---|
| `src/model.rs` | `VersionRecord` 加 `impl` 方法：`is_save_as` / `is_save` / `is_recognized_operation` / `parsed_timestamp` | +80 |
| `src/model.rs::tests` 或 inline | +5 单测 | +100 |
| `src/inspect/report.rs` | Version History section 用 `{operation_label} ({raw})` 格式 | +10 |
| `src/inspect/report.rs::tests` | +1 断言 Version History 段含 "SaveAs" | +25 |
| `tests/unit_parsers.rs` 或 `tests/parse_real_files.rs` | 若 existing DV2/DV3 cross-validate 测试，补一条 operation 对齐断言 | +20 |
| `CHANGELOG.md` | `[0.6.3]` | +25 |
| `Cargo.toml` | 0.6.2 → 0.6.3 | ±1 |
| **本 plan** | | +本文件 |

~250 行。

## 关键决策

### A. Operation 语义化：helper 方法而非 enum 新类型

```rust
impl VersionRecord {
    /// True iff `operation == "SA"` — matches DocVersion2 op_type 0x82
    /// (SaveAs, observed in Phase 9f reverse-engineering).
    pub fn is_save_as(&self) -> bool { self.operation == "SA" }

    /// True iff `operation == "SV"` — matches DocVersion2 op_type 0x81
    /// (Save).
    pub fn is_save(&self) -> bool { self.operation == "SV" }

    /// True iff `operation` is one of the known-labeled values; when
    /// false, `operation` is either empty, non-ASCII, or a code not
    /// seen in SmartPlant samples. Callers that want to render
    /// "SaveAs"/"Save"/"<unknown>" can drive off this + the raw
    /// string.
    pub fn is_recognized_operation(&self) -> bool {
        self.is_save_as() || self.is_save()
    }

    /// Human label for `operation`, mirroring Phase 9f's
    /// `doc_version2::op_type_label`: "SaveAs" / "Save" / raw echo.
    pub fn operation_label(&self) -> &'static str {
        if self.is_save_as() { "SaveAs" }
        else if self.is_save() { "Save" }
        else { "unknown" }
    }
}
```

Enum 新类型会触发 serde 输出变化（向后不兼容）；方法调用不触发。

### B. Timestamp 结构化提取

```rust
impl VersionRecord {
    /// Parse `"MM/DD/YY HH:MM"` into `(month, day, year, hour, minute)`.
    /// Returns `None` if the raw string does not match the format
    /// observed in SmartPlant samples. Year is the raw two-digit
    /// value; callers decide on century convention.
    pub fn parsed_timestamp(&self) -> Option<(u32, u32, u32, u32, u32)> {
        // implementation: split on ' ' / '/' / ':', parse each field as u32
    }
}
```

仅为本期加 `parsed_timestamp`，不加 `timestamp_is_well_formed`（后者
是 `parsed_timestamp().is_some()` 的 trivially-derived call，不值得
独立 API surface）。

### C. `generate_report` 渲染

现状（line 291+ 左右）：

```text
  [SA] SmartPlantPID.a  090000.0144  12/29/25 10:45
```

目标：

```text
  [SaveAs] SmartPlantPID.a  090000.0144  12/29/25 10:45
  [Save]   SmartPlantPID.a  090000.0144  12/30/25 09:12
  [unknown (XY)] SmartPlantPID.a  090000.0144  12/30/25 11:00
```

展示规则：用 `operation_label()` 输出人类标签；如果 raw != label 的
对齐关系（`unknown`），把 raw 值用 `(XY)` 形式附在后面。

### D. Cross-validate 强化（如果现有测试存在）

Phase 9f 应该已有 integration test `doc_version2_decoded_matches_version_history`。如果它没断言 operation 映射（例如 doc_version2
`op_type=0x82` 对应 DV3 record `operation="SA"`），补一条。

## 实施步骤

### W1 — `VersionRecord` helpers + 单测

`model.rs`：新增 `impl VersionRecord` 块，3 个 helper 方法 +
`parsed_timestamp`。

单测（在 model.rs 里新 `#[cfg(test)]` mod，或者加到现有 test）：

1. `version_record_is_save_as_matches_sa_literal`
2. `version_record_is_save_matches_sv_literal`
3. `version_record_operation_label_echoes_unknown_string`
4. `version_record_parsed_timestamp_happy_path`
5. `version_record_parsed_timestamp_returns_none_for_malformed`

### W2 — Report section 渲染

`src/inspect/report.rs` Version History 段用 helper 输出新标签。加
report 测试验证 "SaveAs" / "Save" 字样。

### W3 — Cross-validate 测试扩展

grep existing DV2↔DV3 cross-validate test，补 operation 映射断言：
op_type=0x82 ↔ "SaveAs" / op_type=0x81 ↔ "Save"。若无此类测试，
本轮 skip。

### W4 — ship v0.6.3

bump + CHANGELOG + commit + tag。

## 预计工时

- W1: 40 min
- W2: 25 min
- W3: 20 min
- W4: 15 min
- **合计 ~1.5 hr**

## 验证清单

- [ ] cargo fmt/clippy/test 全 0
- [ ] test count 318 → 324+
- [ ] Cargo.toml 0.6.3
- [ ] v0.6.3 tag

## Next 候选

- **Phase 10e**：`PSMclustertable` per-record 精确映射
- **Phase 10f**：字节级 consumed/leftover 验证框架（roadmap Phase 4）
- **Phase 10g**：`DocVersion3Decoded` 上层模型（如果 operation label 化不够用，考虑再做 full enum 升级版）
