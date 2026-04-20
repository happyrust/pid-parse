# 开发计划：Phase 9l — SummaryInformation property-set Writer

> 起稿：2026-04-21
> 背景：Phase 9k 刚 ship v0.4.2，Writer 层唯一仍在**骗用户**的接口是
> `MetadataUpdates.summary_updates`——字段存在、被 serde 接收，但
> `writer/metadata_write.rs` 的 `apply_metadata_updates` 内部是 no-op。
> apply-plan JSON 里设了 `summary_updates` 的用户得到的是"静默吞掉"。
> `phase8-9h-summary.md` "未完成候选" 列此项为 **4-8 hr / 中风险**。

## 动机

- 产品义务：`summary_updates` 对 API consumer 是可见的承诺，必须兑现或
  移除。本 Phase 选择**兑现**。
- Writer 层唯一的缺口：`/TaggedTxtData/Drawing`、`/TaggedTxtData/General`、
  任意 stream 替换、Sheet byte-range patch 均已支持；唯独
  `/\x05SummaryInformation` 和 `/\x05DocumentSummaryInformation` 两个 OLE
  标准属性流只能 passthrough 原 bytes，无法选择性编辑。
- 为 v0.5.0 定基线：`apply_summary_updates` 真正生效后，Writer 层 minor
  version bump 合理（`no-op → active` 是语义扩展）。

## 非目标

- **不改 FILETIME / I4 类型 property**（create_time / last_save_time /
  page_count 等）。理由：时区、DST 等隐患多，超出 string 扩展的稳健边界；
  FILETIME 字段原字节 passthrough，`summary_updates` 里给此类 key 直接
  返回错误。
- **不改 VT_CLSID / VT_BLOB 等罕见类型**：SmartPlant fixture 里未见，
  遇到时原字节 passthrough + `raw_unsupported_write` 日志。
- **不重写 DocumentSummaryInformation 的 property section**：按 MS-OLEPS
  规范该流可以有多个 section（第二个是 user-defined）；本期只编辑
  第一个 standard section。二期再扩 user-defined section。
- **不实现 property-set stream 新增**：如果源 `.pid` 没有
  `/\x05SummaryInformation` stream，本 Phase 不凭空创造；返回错误给
  用户，要求他们 base0（空 fixture）走 `stream_replacements` 手动塞。
- **不删除 property**：`summary_updates` 只支持"改"与"新增"（如果原
  section 里没这个 prop ID 就追加），不支持"删除"。为删除留
  `summary_deletions` 字段挂位，本期不实现。
- **不动 apply-plan CLI 协议**：`summary_updates` 字段已经接到了
  `WritePlan` JSON schema，CLI 不需要任何 argv 改动；生效是纯底层事。

## 范围

| 文件 | 改动类型 | 行数估计 |
|---|---|---|
| `src/streams/summary.rs` | 扩 reader 保留 FMTID + section bytes + 原 PropValue + prop 顺序 | +60 |
| `src/writer/summary_write.rs` **新增** | property-set serializer (header / FMTID / section / typed values) | +250 |
| `src/writer/metadata_write.rs` | `apply_summary_updates` 分派 | +40 |
| `src/writer/mod.rs` | `pub mod summary_write;` | +1 |
| `src/model.rs` | `SummaryInfo` 追加 `raw_property_set_bytes` / `document_summary_raw_bytes` 字段（skip serde） | +6 |
| `src/writer/plan.rs` | `summary_updates` doc 从 "parked" 改为生效说明 | ±10 |
| `tests/writer_summary.rs` **新增** | round-trip + targeted edit 集成测试 | +200 |
| `tests/writer_real_files.rs` | 新增 `real_file_set_summary_title_preserves_other_props` | +60 |
| `docs/writer-quickstart.md` | 新 5.6 节 "编辑 SummaryInformation" | +50 |
| `CHANGELOG.md` | `[Unreleased]` → `[0.5.0]` 条目 | +40 |
| `Cargo.toml` | version `0.4.2` → `0.5.0` | ±1 |
| **本 plan** | 起草本 plan | +本文件 |

合计 ~750 行改动。仍不触 H7CAD。

## 关键设计决策

### A. key 的命名规范：符号名，不是 PROPID

`summary_updates: BTreeMap<String, String>` 的 key 使用人类可读符号名，
小写蛇形：

| 符号 key | FMTID section | PROPID | 已有 reader 字段 |
|---|---|---|---|
| `title`       | SummaryInformation   | 2  | `SummaryInfo.title` |
| `subject`     | SummaryInformation   | 3  | `raw["PID_SUBJECT"]` |
| `author`      | SummaryInformation   | 4  | `raw["PID_AUTHOR"]` |
| `keywords`    | SummaryInformation   | 5  | `raw["PID_KEYWORDS"]` |
| `comments`    | SummaryInformation   | 6  | `raw["PID_COMMENTS"]` |
| `template`    | SummaryInformation   | 7  | `SummaryInfo.template` |
| `last_author` | SummaryInformation   | 8  | `raw["PID_LASTAUTHOR"]` |
| `rev_number`  | SummaryInformation   | 9  | `raw["PID_REVNUMBER"]` |
| `app_name`    | SummaryInformation   | 18 | `SummaryInfo.creating_application` |
| `category`    | DocumentSummaryInformation | 2 | `raw["DocSummary.PID_CATEGORY"]` |
| `manager`     | DocumentSummaryInformation | 14 | `raw["DocSummary.PID_MANAGER"]` |
| `company`     | DocumentSummaryInformation | 15 | `raw["DocSummary.PID_COMPANY"]` |

**why 符号名 vs 数字 PROPID**：
- JSON plans 由人 + CI 共同维护，`{"title": "X"}` 比 `{"2": "X"}` 可读
- 与未来 schemars 生成 JSON schema 对齐；数字 key 不能做 enum 校验
- 已知 key 表可以在 doc 里一次列清，schema-evolution 成本低

**未知 key 处理**：直接 reject 并返回错误
`SummaryWriteError::UnknownKey { key, known_keys }`。不 fuzzy match，不
自动映射；用户写错 key 必须早失败。

### B. 无损 round-trip：必须保留原 bytes 作为 fallback

当前 reader 只把 property 展开到 `SummaryInfo.title` 等字段 + `raw`
字符串映射，**原始字节丢失**。这意味着任何 round-trip 必须从头重构
整个 property-set stream—— 非字符串 prop（FILETIME / I4）的原始
bit-exact 值会 drift。

**方案**：reader 在解析后把原 stream 的完整字节保留：

```rust
pub struct SummaryInfo {
    // ... 既有 5 字段 ...
    pub raw: BTreeMap<String, String>,
    /// Phase 9l: 原 `/\x05SummaryInformation` 字节原封不动的副本，
    /// writer 以此为 base，仅替换 `summary_updates` 里指定的 prop。
    /// 供 round-trip 保真。`None` 表示源 fixture 没有此流。
    #[serde(skip)]
    pub raw_property_set_bytes: Option<Vec<u8>>,
    /// Same but for `/\x05DocumentSummaryInformation`.
    #[serde(skip)]
    pub document_summary_raw_bytes: Option<Vec<u8>>,
}
```

`#[serde(skip)]` 保证 JSON 输出干净（不 dump 二进制），但 Rust 内存
侧可用。所有 consumer 不受影响（既有 5 字段 + raw map 保持）。

**替代方案**：让 Writer 从 `PidPackage.streams` map 里取
`/\x05SummaryInformation` 的原 bytes，不经 `SummaryInfo` 字段。
这样就**零修改 reader**。**选这个**：因为 writer 本来就拿得到
`PidPackage`，原 bytes 就在 `pkg.streams`；Model 层不用动。删除上面
`raw_property_set_bytes` 字段的 plan。

**修正 A'**：reader 端不需要扩展，Writer 直接从 `PidPackage.streams`
读原 bytes 作 base。

### C. property-set serializer 算法

输入：原 stream bytes（作 base）+ `summary_updates` map
输出：新 stream bytes

**算法**：

1. 解析原 header：byte_order / version / system_id / clsid / num_sections
2. 解析第一 section：section_size / num_props / id+offset 表 / packed
   values。保留所有 `(prop_id, offset_in_section)` 对和对应原 typed value
   bytes（含 VT tag + padding）
3. 对每个 `summary_updates` key：
   - map 到 PROPID（未知 key reject）
   - 若原 section 已有该 PROPID 且类型是 VT_LPSTR 或 VT_LPWSTR：生成新
     typed value bytes，标记该 entry "replaced"
   - 若原 section 已有该 PROPID 但类型是 FILETIME / I4 / etc：reject
     `SummaryWriteError::ReadOnlyPropType { key, vt_type }`
   - 若原 section 没有该 PROPID：新增一条 entry（追加到 prop list 末尾）
4. 以 entry 的新字节们重建 section：header + id+offset 表 + packed values
   （4-byte aligned）
5. 更新 stream header 的 section_offset / 新 section_size
6. 返回新 bytes

**VT_LPSTR (0x001E) 编码**：
```
| VT tag (u32 LE = 0x001E) | char count (u32 LE) | bytes (NUL-terminated) | padding to 4-byte |
```

**VT_LPWSTR (0x001F) 编码**：
```
| VT tag (u32 LE = 0x001F) | char count (u32 LE) | UTF-16LE chars (NUL-terminated) | padding to 4-byte |
```

**策略**：新增/替换 string property 时，**保持原 VT**。如果原字段是
`VT_LPWSTR`，新值用 UTF-16LE 编码；如果原是 `VT_LPSTR`，新值用 ASCII
+ 断言 `value.is_ascii()`（若含非 ASCII，reject，要求用户知情切到
`VT_LPWSTR` —— 但本 Phase 不支持类型转换，直接 reject）。

**新增 property（原 section 没有此 PROPID）**：默认用 `VT_LPWSTR`
（UTF-16LE 兼容所有字符）。

### D. DocumentSummaryInformation 的编辑

一样的 algorithm，只是作用于 `/\x05DocumentSummaryInformation`
的第一个 section，FMTID `{D5CDD502-2E9C-101B-9397-08002B2CF9AE}`。

DocumentSummary 的第二个 section（user-defined dictionary + values）
本期**不触碰**；writer 从原字节里把第二个 section 整体拷贝过去。
新 section 1 offset + 新 section 1 bytes + 原 section 2 offset + 原
section 2 bytes 重新拼接。

### E. 错误分类

```rust
pub enum SummaryWriteError {
    /// 源 fixture 缺 /\x05SummaryInformation 流。用户应走
    /// stream_replacements 塞进去后再 apply_plan。
    StreamNotFound(String),
    /// summary_updates 里有未知 key。
    UnknownKey { key: String, known: Vec<&'static str> },
    /// 原 section 里该 PROPID 的类型不在可写集合
    /// (VT_LPSTR / VT_LPWSTR) 中。
    ReadOnlyPropType { key: String, vt: u16 },
    /// 新值违反原 VT 约束（e.g. 非 ASCII 写入 VT_LPSTR 字段）。
    EncodingMismatch { key: String, vt: u16, reason: String },
    /// property-set bytes 自身损坏（header 对不上）。
    MalformedPropertySet(String),
}
```

全部包装成 `PidError` 返回给 apply-plan CLI，exit code 维持 1。

## 实施步骤

### W1 — property-set parser 扩展（读端保留 prop 顺序和 typed value 原字节）

**重述范围**：W1 实际不是"扩 reader 端"（C 节修正 A' 决策后），而是让
`src/writer/summary_write.rs` 内部能解析 property-set bytes 并跟踪
"每个 prop 的 (PROPID, VT, raw_value_bytes, offset)"。这是 writer 的
内部解析器，与 `streams::summary.rs` reader 独立。

新增 `SummaryPropertySet` 内部类型：

```rust
struct SummaryPropertySet {
    header: Vec<u8>,        // byte_order + version + system_id + clsid + num_sections
    section_fmtid: [u8; 16],
    section_offset: u32,    // 原 offset，写回时可能需要更新
    section: SummarySection,
    // 如果是 DocumentSummary 的多 section：
    extra_sections: Vec<(Vec<u8>, u32)>,  // 原字节 + FMTID offset
}

struct SummarySection {
    props: Vec<SummaryProp>,  // 原顺序
}

struct SummaryProp {
    prop_id: u32,
    vt: u16,
    raw_value: Vec<u8>,  // 含 VT tag + value + padding
}
```

验收：单测构造一个 minimum valid property-set（3 个 prop：VT_LPSTR title
/ VT_LPWSTR author / VT_FILETIME create_time），parse 后能取出
`SummaryProp` 数组，`write_back()` 回 bytes 后与原 bytes 字节等价。

### W2 — property-set serializer

输入 `SummaryPropertySet` + 新值 map → 输出新 bytes。核心在 `SummarySection::serialize`：

```rust
fn serialize(&self, base_offset: u32) -> Vec<u8> {
    // 1. 先布局：每个 prop 按 4-byte alignment 分配 offset
    // 2. 写 section header: section_size, num_props
    // 3. 写 id+offset 表
    // 4. 按布局写 typed values（带 padding）
}
```

验收：原字节 → parse → 不改 map → serialize → bytes 对等（round-trip 自检）。

### W3 — `apply_summary_updates` 对接 `apply_metadata_updates`

`writer/metadata_write.rs` 的 `apply_metadata_updates` 里新增一步：

```rust
if !updates.summary_updates.is_empty() {
    writer::summary_write::apply_summary_updates(package, &updates.summary_updates)?;
}
```

保留 passthrough 语义：`updates.summary_updates` 为空时完全跳过。

### W4 — key 语义 + mapping 表

`writer/summary_write.rs` 顶部：

```rust
const KEY_TO_SUMMARY_PROPID: &[(&str, u32)] = &[
    ("title", 2),
    ("subject", 3),
    ("author", 4),
    ("keywords", 5),
    ("comments", 6),
    ("template", 7),
    ("last_author", 8),
    ("rev_number", 9),
    ("app_name", 18),
];

const KEY_TO_DOC_SUMMARY_PROPID: &[(&str, u32)] = &[
    ("category", 2),
    ("manager", 14),
    ("company", 15),
];
```

key 先查第一表（决定目标 stream = `/\x05SummaryInformation`），miss 再查
第二表（决定目标 stream = `/\x05DocumentSummaryInformation`），都 miss
则 `UnknownKey` 错。

### W5 — 集成测试

`tests/writer_summary.rs` 新增：

1. `apply_summary_passthrough_when_empty_map`：`summary_updates = {}`，
   writer 不触碰 stream（byte 等价原输入）。
2. `apply_summary_title_roundtrips_through_real_property_set`：构造
   最小 property-set fixture（手写 bytes），`summary_updates =
   {"title": "NewTitle"}`，writer 输出 parse 后 title == "NewTitle"，
   其他 prop 字节等价原值。
3. `apply_summary_unknown_key_returns_unknownkey_error`。
4. `apply_summary_read_only_type_returns_error`：给 `create_time` 这种
   FILETIME key 设值，应返回 `ReadOnlyPropType`（—— 注：`create_time`
   不在 KEY_TO_* 表里，先是 `UnknownKey`。修正：test 换成原 section
   里有个 VT_I4 PROPID + mapping 表里可 override 的情形，构造发生
   类型检查的路径。否则本 error variant 不可达 → 简化删除此 variant）。
5. `apply_summary_adds_new_prop_when_not_in_source`：原 section 无
   `subject`，plan 给 `{"subject": "added"}`，parse 回来有 subject。
6. `apply_summary_preserves_filetime_props_byte_for_byte`：原 section
   有 `create_time` FILETIME，plan 里不改它，writer 输出保留原 8-byte
   FILETIME。
7. `apply_summary_document_summary_category_works`：走
   DocumentSummaryInformation 支路。
8. `apply_summary_missing_stream_returns_streamnotfound`：package 里
   没有 `/\x05SummaryInformation` 流，plan 里有
   `summary_updates = {"title": "X"}`，返回 `StreamNotFound`。

`tests/writer_real_files.rs` 新增：

9. `real_file_set_summary_title_preserves_other_props`：conditional
   on fixture；读真实 `.pid`，改 `title`，verify 输出 parse 后 title
   改了，其他 prop 字节级等价。

**预计新增 8-9 tests → 261 → 270 左右**

### W6 — docs + CHANGELOG + ship v0.5.0

- `docs/writer-quickstart.md` 新 5.6 节："编辑 SummaryInformation"，
  列可用 key 表 + JSON 示例 + 错误码说明。
- `CHANGELOG.md`：`[Unreleased]` → `[0.5.0] - 2026-04-21`，描述 Writer
  层 SummaryInformation 支持 + 新 error variants。
- `Cargo.toml`: 0.4.2 → 0.5.0。
- commit + tag v0.5.0。

**为什么是 0.5.0 而非 0.4.3**：
- `summary_updates` 从 no-op → active 是**语义变化**（既有 API consumer
  之前 JSON 里写了此字段被静默吞掉，现在会真实生效）。这是
  behavior-breaking change from consumer 视角。
- Rust lib public API 本身不破坏；`MetadataUpdates` 的 `summary_updates`
  字段签名不变。
- 按 Cargo SemVer 约定，"新增功能 + 向后兼容" → minor bump。0.4.x 是
  "Writer 渐进演化" 系列，0.5.x 开启 "Writer 全功能可编辑" 新系列。

## 预计工时

| 步骤 | 估时 |
|---|---|
| W1 parser 内部化 | 1.5 hr |
| W2 serializer | 2 hr |
| W3 `apply_summary_updates` 接入 | 0.5 hr |
| W4 key mapping + error 分类 | 0.5 hr |
| W5 集成测试 | 1.5 hr |
| W6 docs + CHANGELOG + ship | 0.5 hr |
| **合计** | **~6 hr** |

## 验证清单

- [ ] `src/writer/summary_write.rs::tests::round_trip_preserves_byte_equivalence_when_no_update_applied`
- [ ] 8-9 条新集成测试全过（W5 列表）
- [ ] `cargo test --all-targets` ≥ 269 tests pass
- [ ] `cargo clippy --all-targets -- -D warnings` 退出 0
- [ ] `cargo fmt --all -- --check` 退出 0
- [ ] `apply_metadata_updates` 原 4 条单测继续绿（零回归）
- [ ] `pid_writer_validate --apply-plan` 走一遍含 `summary_updates` 的
      真 JSON plan，verify 输出 parse 后属性真的改了（手工 smoke test）
- [ ] `Cargo.toml` version = "0.5.0"
- [ ] `CHANGELOG.md` 含 `## [0.5.0] - 2026-04-21`
- [ ] `git tag --list v0.5.0` 有输出

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| OLEPS 对齐规则误读 → 产生损坏 property-set | W1/W2 做 byte-for-byte
  round-trip 的 parse→serialize 自测，用真实 fixture 的
  `/\x05SummaryInformation` 字节作金标准 |
| VT_LPSTR 的 code page 不确定 | 本期 reject 非 ASCII 写入 LPSTR；若
  fixture 里的 LPSTR 字段实测是 UTF-8 或 CP1252，在 CHANGELOG 明记
  "UTF-8 支持推迟到 Phase 9m"；实际很多 SmartPlant LPSTR 都已是
  LPWSTR 所以影响面小 |
| DocumentSummaryInformation 多 section 破坏 | W2 保留 section 2..N
  原字节不动；只改 section 1，section 2 offset 随 section 1 新长度
  做相应位移（stream 里 section FMTID + offset 表在 stream 头部，需
  同步更新） |
| 新 prop 值长度变化后 section_offset 级联 drift | 完整的
  `SummaryPropertySet::write_back` 负责重新计算所有 offset；
  round-trip 自测 + real-file smoke 共同覆盖 |
| SmartPlant 特定 P&ID 用了非标准 prop ID | 非标 prop 全部 passthrough
  原字节；只有显式出现在 `summary_updates` 才触发 writer 路径 |
| 单轮对话无法 6 hr 一口气写完 | W1-W2-W3 在一轮内完成（~4 hr）→
  ship 0.5.0-beta.1；W4-W5-W6 下一轮完成 → ship 0.5.0 正式版。
  每轮单独 commit 可独立 revert。 |

## 回滚

每个 W 的改动都集中在独立文件：
- W1/W2: `src/writer/summary_write.rs` 新增（revert = 删文件）
- W3: `src/writer/metadata_write.rs::apply_metadata_updates` 新增 1 个
  分派分支，revert 单行
- W4: `src/writer/summary_write.rs` 内表常量，不影响其他模块
- W5: `tests/writer_summary.rs` 新增独立文件
- W6: Cargo.toml / CHANGELOG.md 单点回退

全部可以通过 `git revert <ship-commit>` 一键回滚。

## Next 候选（跟进排队）

- **Phase 9m**：`summary_updates` 支持 VT_LPSTR 下的 UTF-8 / CP1252 编码
  推断（本期 reject 非 ASCII）
- **Phase 9n**：DocumentSummaryInformation user-defined dictionary
  section 2 的读写
- **P3-2 representative_symbol_hints 缓存**：留在 Phase 10a 性能专题
- **v0.5.1 patch**：`summary_deletions: Vec<String>` 字段支持删除 prop
