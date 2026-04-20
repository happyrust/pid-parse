# 开发计划：Phase 10j — DocumentSummaryInformation section 2 编辑

> 起稿：2026-04-21
> 目标版本：v0.9.0（minor bump）
> 前置：Phase 10i (v0.8.0) — VT_LPSTR 多 code page
> 估计工时：4-6 hr

## 动机

`/\x05DocumentSummaryInformation` 是 OLE property-set stream 的 **multi-section** 格式：

- **Section 1**（FMTID = `D5CDD502-2E9C-101B-9397-08002B2CF9AE`）：标准预定义 property（category, manager, company, content type, slide count, ...）— 已由 Phase 9l-10g 完整支持
- **Section 2**（FMTID = `D5CDD505-2E9C-101B-9397-08002B2CF9AE`）：**user-defined property dictionary** — SmartPlant / Office 堆自定义项目属性的地方

当前 (v0.7.0+) Writer 层处理 DocumentSummaryInformation 时，**section 2 字节 verbatim 透传、不可编辑**。这导致：

1. **SmartPlant 项目自定义属性无法改**：比如 `SP_ProjectID` / `SP_DrawingRevision` / `SP_Discipline` 等工程侧 metadata 就在 section 2
2. **CI / 自动化管道受限**：想批量把一批图纸的 `SP_ProjectID` 从 "PROJ-001" 改 "PROJ-002"，现在只能走 stream_replacements 整流重写（丢失 property 结构）
3. **Writer 层语义残缺**：section 1 有 CRUD、section 2 没有，API surface 不对称

Phase 10j 补齐 section 2 的 read / write / delete。

## 非目标

- **不改** section 1 行为（Phase 9l-9n-10g 已完整）
- **不支持** 第 3 及以后 section（样本未见；若出现 verbatim 透传）
- **不支持** user dict 之外的 custom FMTID（section 2 FMTID 是固定的 user-defined dict FMTID）
- **不做** property type 扩展（仅支持 VT_LPSTR / VT_LPWSTR / VT_I4 / VT_FILETIME / VT_BOOL — user dict 里 SmartPlant 实测 95%+ 是字符串或整数）
- **不做** property dictionary 名字冲突自动 namespace（user 自己管）

## 范围

| 文件 | 改动 | 行数（估）|
|---|---|---|
| `Cargo.toml` | version 0.8.0 → 0.9.0 | ±1 |
| `src/writer/summary_write.rs` | `SummaryPropertySet` 扩展到多 section；`UserDefinedDictionary` 新类型；`apply_user_property_updates` / `apply_user_property_deletions` | +280 |
| `src/writer/plan.rs` | `MetadataUpdates` 新字段 `summary_user_updates`, `summary_user_deletions` | +40 |
| `src/writer/metadata_write.rs` | `apply_metadata_updates` 调用顺序扩展 | +20 |
| `src/streams/summary.rs` | Reader 端解 section 2 → `SummaryInfo.user_properties: BTreeMap<String, PropertyValue>` | +90 |
| `src/model.rs` | `PropertyValue` 枚举 + `SummaryInfo` 新字段 | +80 |
| `src/bin/pid_writer_validate.rs` | `--set-user-summary KEY=VALUE` + `--delete-user-summary KEY` | +60 |
| `tests/writer_roundtrip.rs` | 多 section 端到端 round-trip 3 条 | +180 |
| `tests/writer_validate_cli.rs` | CLI user summary 4 条 | +120 |
| `CHANGELOG.md` | `[0.9.0]` 段 | +80 |
| `docs/writer-quickstart.md` | 新 5.8 节 | +70 |
| **本 plan** | | +本文件 |

~1030 行，核心逻辑约 500 行。

## 设计

### 1. OLE property-set multi-section 布局复习

参考 [MS-OLEPS] §2.21：

```
PropertySetStream {
    ByteOrder: u16 = 0xFFFE
    Version: u16 = 0 or 1
    SystemIdentifier: u32
    CLSID: 16B = {00000000-0000-0000-0000-000000000000}
    NumPropertySets: u32                       ← 关键：>= 1
    FMTID0: 16B
    Offset0: u32                               ← section 0 起始字节
    [FMTID1: 16B]                              ← 若 NumPropertySets >= 2
    [Offset1: u32]
    [... section N-1]
    PropertySets[0]: PropertySet
    [PropertySets[1]: PropertySet]
}

PropertySet {
    Size: u32
    NumProperties: u32
    PropertyIdAndOffsets: [(PROPID: u32, Offset: u32); NumProperties]
    Properties: [TypedPropertyValue; NumProperties]
    [Dictionary: DictionaryProperty (if DICTIONARY_PROPERTY_IDENTIFIER present)]
    [CodePage: I2_Property (if CODE_PAGE_PROPERTY_IDENTIFIER present)]
}
```

**DocumentSummaryInformation section 2 特征**：
- FMTID = `D5CDD505-2E9C-101B-9397-08002B2CF9AE`
- **Dictionary** property (PROPID 0x00000000) 必在最前 — name → prop_id 的映射表
- 后续 property 的 PROPID 按 Dictionary 里声明的 id 走

### 2. `UserDefinedDictionary` 类型

```rust
// src/model.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UserDefinedDictionary {
    /// name → PROPID 映射（PROPID 从 2 起分配，0 是 dictionary 自身、1 是 code page）
    pub entries: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "value")]
pub enum PropertyValue {
    Lpstr(String),
    Lpwstr(String),
    I4(i32),
    Bool(bool),
    Filetime(u64),
    /// 不识别的 VT：保留原始字节，writer 端 verbatim 透传
    Raw { vt: u16, bytes: Vec<u8> },
}
```

`PidDocument.summary.user_properties: BTreeMap<String, PropertyValue>` 是用户视角的扁平 map（namespace = section 2 dictionary + name）。

### 3. Writer 端 API

```rust
// src/writer/plan.rs
pub struct MetadataUpdates {
    pub drawing_xml: Option<String>,
    pub general_xml: Option<String>,
    pub summary_updates: BTreeMap<String, SummaryValue>,        // Phase 10i
    pub summary_deletions: Vec<String>,                          // Phase 9n
    pub summary_user_updates: BTreeMap<String, PropertyValue>,  // NEW
    pub summary_user_deletions: Vec<String>,                     // NEW
}
```

执行顺序（`apply_metadata_updates`）：

```
1. drawing_xml
2. general_xml
3. summary_deletions
4. summary_updates
5. summary_user_deletions      ← NEW
6. summary_user_updates         ← NEW
```

**冲突 guard**：同一 user key 同时出现在 `summary_user_deletions` 和 `summary_user_updates` → pre-check fail（对齐 Phase 9n 的 set/delete 冲突拒绝）。

### 4. Dictionary 管理

- **新增 property**：如果 key 不在 dictionary，分配新 PROPID（现有最大 + 1），加 dictionary 条目 + 写 property
- **删除 property**：移除 dictionary 条目 + 移除 property；PROPID 不回收（现有 SmartPlant 实测允许 dictionary gap）
- **覆盖 property**：保留原 PROPID，仅换 property value

### 5. CLI

```
--set-user-summary KEY=VALUE          # 默认 VT_LPWSTR（跨 code page 最安全）
--set-user-summary-encoded KEY:TYPE=VALUE  # 显式 type: lpstr / lpwstr / i4 / bool
--delete-user-summary KEY             # 移除 user property
```

示例：

```bash
pid_writer_validate in.pid \
    --set-user-summary "SP_ProjectID=PROJ-002" \
    --set-user-summary-encoded "SP_Revision:i4=3" \
    --delete-user-summary "SP_LegacyField" \
    --out out.pid
```

### 6. 保真度保证

- **不触碰 section 1**：section 2 改动期间，section 0（section 1 的 byte offset）地址可能变 — 需要重写 `NumPropertySets / FMTID0 / Offset0 / FMTID1 / Offset1` header。这一步**必须保证 section 1 字节不变**（靠 `apply_summary_updates` 已有的 byte-level 保真）
- **section 2 未触碰的 property 字节不变**：只改 dictionary 和 target property 的 offset 表，其他 property byte-for-byte
- **Code page（PROPID 1 = CODE_PAGE_PROPERTY_IDENTIFIER）**：保留源文件值；Phase 10i 的 code page 探测可复用到 section 2 的 VT_LPSTR decoding

## 实施步骤

### W1 — `UserDefinedDictionary` + `PropertyValue` 类型（~45min）

- `src/model.rs` 加类型 + `#[derive(JsonSchema)]`
- `SummaryInfo.user_properties` 新字段，`#[serde(default)]` 向后兼容
- 单测：serialize / deserialize / default 向后兼容

### W2 — Reader 端 section 2 解码（~75min）

- `src/writer/summary_write.rs::SummaryPropertySet` 重构支持多 section
- `src/streams/summary.rs` 接入 section 2 → `SummaryInfo.user_properties`
- 单测：多 section fixture（合成 + 若有真实 fixture 则一并）

### W3 — Writer 端 `apply_user_property_updates` / `apply_user_property_deletions`（~90min）

- dictionary 管理（add / remove / PROPID 分配）
- property 编解码（VT_LPSTR/LPWSTR/I4/BOOL/FILETIME）
- 冲突 guard：set/delete 同 key 互斥
- 单测：8-10 条覆盖 CRUD + 冲突 + raw-VT 透传

### W4 — CLI（~45min）

- `pid_writer_validate` 3 个新 flag
- 集成测试 4 条（add / update / delete / type override）

### W5 — 真实 fixture 验证（~30min，条件）

- `tests/writer_real_files.rs` 条件测试：fixture 有 section 2 时验证端到端

### W6 — docs + CHANGELOG（~45min）

- `docs/writer-quickstart.md` 新 5.8 节 "编辑 DocumentSummaryInformation user dictionary"
- `CHANGELOG.md` `[0.9.0]` 段

### W7 — ship v0.9.0

- `cargo fmt/clippy/test` 三零；预期 test count 340+ → 355+
- `git commit -m "feat(writer): v0.9.0 — DocumentSummaryInformation section 2 editing (Phase 10j)"`
- `git tag v0.9.0`

## 预计工时

- W1 45min
- W2 75min
- W3 90min
- W4 45min
- W5 30min（条件）
- W6 45min
- W7 10min
- **合计 5.0-5.5 hr**（预留 30min buffer = 4-6 hr 区间）

## 验证清单

- [ ] section 2 新增 VT_LPWSTR property 端到端 round-trip
- [ ] section 2 覆盖已有 property value（PROPID 保留）
- [ ] section 2 删除 property（dictionary + property 双清）
- [ ] section 1 不受 section 2 改动影响（byte-for-byte）
- [ ] section 2 未触碰 property 不受改动影响（byte-for-byte）
- [ ] CLI `--set-user-summary` / `--delete-user-summary` 3 个 flag 工作
- [ ] set/delete 同 key 冲突 pre-check fail
- [ ] `SummaryInfo.user_properties` serialize / deserialize 向后兼容
- [ ] test count: 340+ → 355+
- [ ] clippy / fmt 双零
- [ ] `CHANGELOG.md` [0.9.0] 段完整

## 风险缓解

| 风险 | 缓解 |
|---|---|
| section 2 dictionary PROPID 分配错误 | 按源文件最大 PROPID + 1 安全分配；单测 dict gap 场景 |
| section 1 / section 2 offset 重算 bug | parse → serialize → byte-level diff；必须零差异（untouched path）|
| SmartPlant 实测 section 2 格式差异 | 单真实 fixture 硬约束；若 section 2 缺失则测试 skip；多 fixture 是硬约束留给 Phase 11+ |
| user dict key 有中文 / 非 ASCII | 走 VT_LPWSTR（UTF-16LE）默认，避免 code page 问题 |
| PROPID 回收 vs gap | 选 gap 策略（SmartPlant 实测允许）；doc 里明示用户该字段 id 不回收 |
| `PropertyValue::Raw` 变体持久化 | writer 端字节 verbatim；若用户试图 update 成新 type，先 delete 再 add 最干净 |

## SemVer 判定

- 新增 `MetadataUpdates` 字段（additive via `#[serde(default)]`）：minor
- 新增 `SummaryInfo.user_properties` 字段（同上）：minor
- 新增 `PropertyValue` / `UserDefinedDictionary` 类型：minor
- 新增 CLI flag：minor
- 行为：Writer 之前 silently verbatim 透传 section 2 → 现在 active 编辑。不 break 既有 consumer；仅扩展能力

综合：**minor bump 0.8 → 0.9.0**。

## Next 候选（10j 完成后）

Phase 10 系列结束（Writer 全功能 CRUD 达顶）；下个周期推进 Parser 结构化深化：

- **Phase 11a**：PSMclustertable per-record 结构化（roadmap 2.2）
- **Phase 11b**：PSMsegmenttable 结构化（roadmap 2.3）
- **Phase 11c**：Sheet 深层几何解码（roadmap 2.5，大 Phase）

## 交叉引用

- 上游总 roadmap：`docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` 阶段 B
- 前置 Phase：`docs/plans/2026-04-21-phase-9l-summary-info-writer.md`（section 1 Writer）+ `2026-04-21-phase-9n-summary-deletions.md`（CRUD D）+ `2026-04-21-phase-10g-lpstr-utf8.md`（UTF-8）+ `2026-04-21-phase-10i-vtlpstr-codepage.md`（多 code page）
- SPPID 战略：`docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` — 本 Phase 属于 Writer 边界打磨，不在 roadmap Phase 2-5 主线
