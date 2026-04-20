# 变更日志

## [Unreleased]

## [0.5.3] - 2026-04-21

### Phase 9o: Writer API ergonomics patches

四轮 Writer 内部能力扩展（Phase 9k/9l/9m/9n）完成后，本轮回头收拾
下游 consumer 侧的入门样板。全部改动都是 **additive**（新 public
方法），不破坏任何 v0.5.x 已有 API 签名。

### Added

- `PidPackage::from_path<P: AsRef<Path>>(path)` — 简写
  `PidParser::new().parse_package(path)`，两步变一步。
- `PidPackage::from_bytes(&[u8]) -> Result<Self, PidError>` — 从
  内存字节流解析。v0.5.3 内部实现走 tempfile 兜底（HTTP service /
  压缩包 / 嵌入资源都可直接喂字节）；真正的零磁盘纯内存路径依赖
  parser 内部 reader 泛型化，留给 Phase 10a。
- `PidWriter::write_to_bytes(pkg, plan) -> Result<Vec<u8>, PidError>` —
  镜像 `write_to`，直接返回 CFB 字节数组。内部复用新的
  `cfb_write::write_package_to_writer<F: Read + Write + Seek>` 泛型
  backend，避免磁盘往返。
- `cfb_write::write_package_to_writer` 公共泛型入口（Phase 10a 以后
  可能用作 zero-disk 测试基础设施）。
- `WritePlan::from_json(&str)` / `to_json()` / `to_json_pretty()` —
  JSON round-trip helpers，错误一律包装成
  `PidError::ParseFailure { context: "WritePlan JSON", ... }`，
  consumer 不用自行 handle `serde_json::Error`。

### Changed

- `PidWriter::write_to` 内部提取 `apply_plan_to_package` helper，与
  `write_to_bytes` 共用流水线。未来添加新 plan 字段（比如
  `post_write_clsid_set`）只需要改一次。行为完全等价。

### Tests

lib 单元（+7）：
- `api::tests::from_bytes_parses_a_minimal_synthetic_pid`（构造内存
  CFB fixture → `from_bytes` → verify stream 存在）
- `api::tests::from_bytes_on_invalid_data_returns_error`
- `api::tests::from_path_matches_parse_package_behavior`（两个入口
  行为一致）
- `writer::plan::tests::plan_json_round_trip_default_is_passthrough`
- `writer::plan::tests::plan_from_json_rejects_invalid_syntax_with_pid_error`
- `writer::plan::tests::plan_to_json_pretty_contains_newlines_and_indent`
- `writer::plan::tests::plan_from_json_empty_object_is_valid_passthrough`

集成（+2，`tests/writer_roundtrip.rs`）：
- `write_to_bytes_produces_bytes_parseable_by_from_bytes` — 全在线
  round-trip：`from_path` → plan → `write_to_bytes` → `from_bytes`
  → verify edit 落地。
- `write_plan_json_round_trip_preserves_metadata_and_payload_bytes` —
  包含 `summary_updates` + `summary_deletions` + `stream_replacements`
  + `sheet_patches` 完整 plan 的 JSON 往返无损（特别断言 base64
  payload 字节级等价）。

全套 287 → **296 tests pass**（lib 199 → 206 +7；writer_roundtrip
11 → 13 +2）。`cargo fmt --check` / `cargo clippy -D warnings` 双零。

### Docs

- `docs/plans/2026-04-21-phase-9o-api-ergonomics.md`：本轮 dev plan
  含 "不做 parser 泛型化 / tempfile 兜底" 的 trade-off 说明 + Phase
  10a roadmap 衔接。

### Consumer quick-start

```rust
use pid_parse::{PidPackage, PidWriter, WritePlan};

// 从字节流或路径二选一
let pkg = PidPackage::from_path("input.pid")?;
// let pkg = PidPackage::from_bytes(&http_response_body)?;

// 用 JSON 声明式 plan 或直接构造
let plan = WritePlan::from_json(r#"{"metadata_updates":{"summary_updates":{"title":"Q4"}}}"#)?;

// 输出到内存或磁盘
let bytes = PidWriter::write_to_bytes(&pkg, &plan)?;
// PidWriter::write_to(&pkg, &plan, Path::new("output.pid"))?;
```

## [0.5.2] - 2026-04-21

### Phase 9n: `summary_deletions` — SummaryInformation CRUD 收尾

Phase 9l/9m 铺的 Writer 层字符串 property 编辑只有 CREATE（新增）/
UPDATE（覆写）两条语义。本轮补上 **DELETE**，让 SummaryInformation
与 DocumentSummaryInformation 的 property-set 写路径真正支持 CRUD。

### Added

- `WritePlan.MetadataUpdates.summary_deletions: Vec<String>`
  （`#[serde(default)]`，JSON plan 向后兼容）。语义等价于
  `summary_updates` 的逆操作：按符号 key（title / author / subject /
  keywords / comments / template / last_author / rev_number / app_name /
  category / manager / company）定位到 PROPID 并从 section 移除。
  - 空 vec = free no-op
  - key 在 section 里不存在 = 静默 no-op（遵循 stream_replacements
    "删不存在的不 fail" 传统）
  - key 不在符号表 = `UnknownKey` 错误
- `pid_parse::writer::summary_write::apply_summary_deletions` 公共入口。
- `SummarySection::remove(prop_id)` 内部方法（Phase 9l 的
  `SummaryPropertySet` 只多一个方法，其他基础设施复用）。
- `pid_writer_validate --delete-summary KEY` 便利 CLI flag，对称于
  `--set-summary KEY=VALUE`。可多次传入累加；与 `--set-summary` 在
  *不同* key 上可共存，同 key 则报冲突错。与 `--apply-plan` 互斥。
- `ValidateError::Edit("summary_updates and summary_deletions both target
  key '{k}'")`：同 key 同时出现在 set/delete 两个字段时的明确拒绝，
  在 lib 层 (`apply_metadata_updates`) 和 CLI 层 (`run_validate`) 都有
  pre-check。

### Changed

- `apply_metadata_updates` 执行顺序正式定义：`drawing_xml` →
  `general_xml` → `summary_deletions` → `summary_updates`。先删再增
  保证 edge case "删 A、加 A" 的最终态一致（与冲突拒绝共存只是防御性
  guard；即便 guard 放过也不会产生 inconsistent state）。
- `pid_writer_validate`：
  - `print_usage` 更新列出 `--delete-summary` 段与 "cannot be combined
    with … --delete-summary" 的精确错误消息。
  - `run_validate` / `compare_packages` 签名新增 `summary_deletions`
    参数，`edited_paths` 自动扩展到 summary 两流（保证删除后字节级
    变化归入 `edited` 而非 `mismatched`）。
  - `collect_edited_paths_from_plan` 亦同步识别 `summary_deletions`。
- `WritePlan::is_passthrough` 扩展：`summary_deletions.is_empty()` 加入
  判空链。
- `WritePlan::metadata_only` 构造函数新增 `summary_deletions:
  Vec::new()` 初始化。

### Tests

lib 单元测试（`src/writer/summary_write.rs::tests`，+5 条）：
- `apply_summary_deletions_removes_existing_prop`
- `apply_summary_deletions_nonexistent_key_is_silent_noop`（断言静默
  no-op 且**流字节保持不变**，避免 `modified: true` 误染 diff）
- `apply_summary_deletions_unknown_key_returns_error`
- `apply_summary_deletions_empty_is_zero_cost_noop`
- `apply_summary_deletions_preserves_filetime_byte_for_byte`（非目标
  prop 字节级保留的 Phase 9l 契约，在 delete 路径上再验证一次）

lib 单元测试（`src/writer/metadata_write.rs::tests`，+1 条）：
- `summary_updates_and_deletions_on_same_key_return_error`

CLI 集成测试（`tests/writer_validate_cli.rs`，+4 条）：
- `validate_delete_summary_removes_target_prop`
- `validate_delete_and_set_summary_combine_legally`
- `validate_delete_summary_conflicts_with_set_summary_on_same_key`
- `validate_delete_summary_unknown_key_exits_two`

Real-file 集成测试（`tests/writer_real_files.rs`，+1 条件性条）：
- `real_file_delete_summary_prop_when_present`：fixture 存在且有
  summary 流时删除 `keywords`（或 fallback 到 `title`），verify 非
  summary 流全字节等价 + 目标 prop 在 reader 视角消失。

全套 276 → **287 tests pass**（lib 193 → 199 +6；writer_real_files
9 → 10 +1；writer_validate_cli 17 → 21 +4）。

### Docs

- `docs/plans/2026-04-21-phase-9n-summary-deletions.md`：本轮 dev plan。
- `docs/writer-quickstart.md` 5.6 节已在 Phase 9m 的 "CLI 快捷方式"
  块里把 `--delete-summary` 作为第 9n 版延伸注解。

## [0.5.1] - 2026-04-21

### Phase 9m: `--set-summary` CLI flag + real-file integration

Corner-case convenience pass on top of Phase 9l (v0.5.0) — turns the
SummaryInformation writer from "编辑需要手写 plan.json" into "可用单个
命令行 flag 直接改"。

### Added

- `pid_writer_validate --set-summary KEY=VALUE`：特化便捷 flag，对称于
  `--edit` (drawing XML) / `--general-edit` (general XML)。多次 `--set-summary`
  会累加到同一个 summary map；后传覆盖先传。支持所有 Phase 9l
  KEY_TO_*_PROPID 表里的 11 个 key。
- `run_validate` 函数签名新增 `summary_edits: &BTreeMap<String, String>`
  参数（binary 专用 public API；CLI 集成测试可直接传；lib consumer 不受
  影响）。
- `tests/writer_validate_cli.rs` 4 条新集成测试：
  - `validate_set_summary_single_key_rewrites_title`
  - `validate_set_summary_multiple_keys_accumulate`（title / author /
    subject 一次调用累加）
  - `validate_set_summary_conflicts_with_apply_plan_exits_one`
  - `validate_set_summary_unknown_key_exits_two_with_clear_error`
- `tests/writer_real_files.rs` 1 条 conditional 测试
  `real_file_set_summary_title_preserves_other_streams`：当
  `test-file/DWG-0201GP06-01.pid` 存在并含 `/\x05SummaryInformation` 时
  验证真实 `.pid` 端到端 → 写 title → parse → 断言只有 summary 流变，
  其他所有流 byte-identical；fixture 缺失或无 summary stream 时 skip。

### Changed

- `--apply-plan` 与 `--set-summary` 互斥：同时传返回 usage error（exit 1），
  stderr 含 `--set-summary` 字样。延续 Phase 9b 的"declarative plan 与
  特化 flag 互斥"设计。
- `compare_packages` / `collect_edited_paths_from_plan` 都扩展到把
  `/\x05SummaryInformation` 和 `/\x05DocumentSummaryInformation` 纳入
  "可能被编辑"的 stream 集合，保证 `edited` vs `matched` 计数在
  summary 改动时不误报 `mismatched`。
- `docs/writer-quickstart.md` 5.6 节追加 CLI 用法示例。

### API surface

- `pid_parse::writer::summary_write::SUMMARY_INFO_PATH` /
  `DOC_SUMMARY_PATH` 从 `pub(crate)` 提升为 `pub`，让 binary 和外部
  consumer 能引用同一组常量而不用硬编码字节。

### Tests

全套 271 → **276 tests pass**（writer_real_files 8 → 9 +1；
writer_validate_cli 13 → 17 +4）。`cargo fmt --check` / `cargo clippy
--all-targets -D warnings` 双零。

### Docs

- `docs/plans/2026-04-21-phase-9m-summary-cli-integration.md`：本轮 dev plan。
- `docs/writer-quickstart.md` 5.6 节："编辑 SummaryInformation"新增
  `--set-summary` CLI 用例块。

## [0.5.0] - 2026-04-21

### Phase 9l: SummaryInformation / DocumentSummaryInformation property-set writer

`MetadataUpdates.summary_updates` 从 **parked placeholder** 变为**真正可
编辑的 OLE 属性流接口**，补全 Writer 层最后一个"骗用户"的 API。

从 v0.4.2 起，apply-plan JSON 里填 `summary_updates` 字段会被静默吞掉；
从 v0.5.0 起，会真实写入 `/\x05SummaryInformation` /
`/\x05DocumentSummaryInformation` 的 OLE property-set。这是**对外语义
变化**（向后兼容的 Rust API，但 JSON plan consumer 的行为改变），因此
走 **minor bump**（0.4.x → 0.5.x 开启新周期："Writer 全功能可编辑"）。

### Added

- `src/writer/summary_write.rs` **新模块**：
  - `SummaryPropertySet` 内部类型，parse + serialize OLE property-set
    stream（[MS-OLEPS] 规范），支持 byte-level round-trip。
  - `apply_summary_updates(pkg, updates)` 公共入口，按符号名 key 定位
    PROPID + 目标 stream，仅编辑 `VT_LPSTR` / `VT_LPWSTR` 字符串型
    property；未触及的 property（含 `VT_FILETIME` / `VT_I4`）字节级
    保留。
  - 支持 key 列表（11 条）：
    - SummaryInformation 段：`title`, `subject`, `author`,
      `keywords`, `comments`, `template`, `last_author`,
      `rev_number`, `app_name`
    - DocumentSummaryInformation 段：`category`, `manager`, `company`
  - 清晰的错误分类（全部包装为 `PidError::ParseFailure { context:
    "summary writer", ... }` 不破坏 public error surface）：
    - unknown key（列出已知 key 表）
    - 目标 stream 不存在（提示用 `stream_replacements` 塞）
    - 未支持的 source VT 类型（避开 FILETIME / I4 写入）
    - 非 ASCII 写入 `VT_LPSTR` 字段（提示 Phase 9m 会支持 UTF-8）
  - 常量 `SUMMARY_INFO_PATH` / `DOC_SUMMARY_PATH` 和标准 FMTID
    (`F29F85E0-4FF9-1068-AB91-08002B27B3D9` /
    `D5CDD502-2E9C-101B-9397-08002B2CF9AE`) 内联定义。

### Changed

- `writer::plan::MetadataUpdates.summary_updates` 文档从 "Placeholder —
  silently ignored" 更新为"实际生效"，列出 11 个可用 key 和 encoding
  规则。字段签名（`BTreeMap<String, String>`）不变，现有 Rust consumer
  零破坏。
- `writer::metadata_write::apply_metadata_updates` 在 drawing / general
  XML 写入之后调用 `apply_summary_updates`。空 map = 0 开销。
- `writer/mod.rs` 模块 doc 去掉"no SummaryInformation property-set
  writer"的 caveat。

### Tests

lib (unit)：
- `writer::summary_write::tests::parse_then_serialize_is_byte_identical_for_untouched_stream`
- `writer::summary_write::tests::apply_summary_updates_passthrough_empty_map_touches_nothing`
- `writer::summary_write::tests::apply_summary_updates_edits_title_and_preserves_filetime`
  （关键断言：FILETIME prop 改写其他 prop 后 byte-for-byte 不动）
- `writer::summary_write::tests::apply_summary_updates_rejects_unknown_key`
- `writer::summary_write::tests::apply_summary_updates_adds_new_string_prop_when_absent`
  （新 prop 默认 `VT_LPWSTR`）
- `writer::summary_write::tests::apply_summary_updates_returns_stream_not_found_when_missing`
- `writer::summary_write::tests::encode_lpstr_rejects_non_ascii`
- `writer::summary_write::tests::encode_lpwstr_accepts_unicode`

集成（`tests/writer_roundtrip.rs`）：
- `summary_updates_rewrite_title_end_to_end_through_pid_writer`：完整
  链路 `PidWriter::write_to → CFB → parse → SummaryInfo.title`。
- `summary_updates_unknown_key_fails_writer_with_clear_error`：错误传
  播到 Writer top-level。

全套 261 → **271 tests pass**（lib 185 → 193 +8；writer_roundtrip 9 → 11 +2）。

### Docs

- `docs/plans/2026-04-21-phase-9l-summary-info-writer.md`：本轮 dev plan
  （scope / 关键设计决策 / 5-7 hr W1-W6 步骤 / 风险缓解表 / 回滚策略 /
  Next 候选）。

### Known limitations (tracked for future phases)

- `VT_LPSTR` 字段不接受非 ASCII 值（Phase 9m 计划支持 UTF-8 / CP1252）。
- DocumentSummaryInformation 第二个 section（user-defined dictionary）
  不编辑；section 2..N 的原字节 verbatim 透传（Phase 9n）。
- 不支持删除 property（`summary_deletions` 字段挂位，future minor bump）。
- 不支持从零新建 `/\x05SummaryInformation` stream；源 package 必须
  已有此 stream，否则返回 `stream does not exist` 错误（由用户先走
  `stream_replacements` seed）。

## [0.4.2] - 2026-04-21

### Phase 9k: Ship `--apply-plan` + P3 cleanups + lint/fmt restore

从 [0.4.1] 合并：layout 语义关键词数据驱动重构（`48135a8`）+ Writer 层
`--apply-plan` CLI（`3a2ecde`）。同时扫清 Phase 9i 之后悄悄堆积的 **7 条
clippy warning**（lib-test 代码里 6 条 `field_reassign_with_default` +
1 条 `map_clone` / `iter_cloned_collect`）与 10 文件的 `cargo fmt`
漂移，执行 `phase8-9h-summary.md` 里列的 P3 cleanups 中 3 条低风险项
（#1 `file_stem` 跨平台、#3 `diff.rs` `writeln!().unwrap()`、#4 tests
use 散落；#2 `representative_symbol_hints` 缓存留给下一轮性能 Phase）。

全套从 260 增至 **261 tests pass**（lib 185 + parse_real_files 28 +
unit_parsers 18 + writer_real_files 8 + writer_roundtrip 9 +
writer_validate_cli 13 = 261）。`cargo clippy --all-targets -- -D warnings`
和 `cargo fmt --all -- --check` 均返回 0 退出码。

### Added

- `pid_writer_validate --apply-plan <plan.json>`：一次性施加完整 `WritePlan`
  （metadata XML / stream replacements / sheet patches）并走 round-trip +
  byte-diff verify。与 `--edit` / `--general-edit` 互斥；`--json` 输出
  扩展 `plan_applied` 字段。
- `Cargo.toml`：`base64 = "0.22"` 依赖（WASM / no_std 友好，为批量 CLI
  及未来跨语言 binding 共用）。
- `src/layout.rs::file_stem_cross_platform`：内部 helper，先把 Windows
  反斜杠 UNC 路径归一化为正斜杠再喂 `Path::file_stem`，消除 Linux CI 上
  `\\srv\sym\piping\valve.sym` 被当成单一文件名返回整串的怪行为。

### Changed

- `WritePlan`、`MetadataUpdates` 字段追加 `#[serde(default)]`，让 `{}`
  成为合法 JSON passthrough，`{"metadata_updates":{"drawing_xml":"..."}}`
  也无需显式写 `general_xml: null` / `summary_updates: {}`。保留 Rust
  侧 `WritePlan::default()` 行为不变。
- `StreamReplacement.new_data` / `SheetChunkPatch.replacement` 的 JSON
  序列化由 `Vec<u8> = [int array]` 改为**标准 base64 字符串**（`A-Z a-z
  0-9 + / =`）。Rust consumer 透明，JSON 大小约缩减 6x。内部 `#[serde(with
  = "bytes_base64")]` 自定义 adaptor，反序列化失败走 serde error。
- `src/layout.rs`: 语义关键词推断改为数据驱动。新增 `SEMANTIC_KEYWORDS` 常量表
  （`OffPageConnector` / `Nozzle` / `Instrument` / `Vessel` / `Note` /
  `PipingComponent`），以及每个 tag 的英文 + 中文同义词列表，取代原先 if/else
  链。行为对既有 fixture 保持等价，新增中文 symbol 路径（例如 `\\srv\sym\管件\球阀.sym`）
  的语义命中。顺序依赖显式保留（`OPC` 先于 `valve`）。
- `src/layout.rs`: `representative_symbol_hints` 的 tiebreaker 抽成
  `should_replace_representative(existing_count, existing_path, candidate_count,
  candidate_path)` helper，带 doc comment 说明 "higher usage_count wins; ties
  break on lexicographically smaller path" 规则，替代原 inline 表达式。
- `src/inspect/diff.rs::render`：11 处 `writeln!(&mut out, ...).unwrap()` 改为
  `String::push_str` / `push_str(&format!(..))`。`writeln!` 对 `String`
  的 `fmt::Write` impl 技术上不会 fail，但 `.unwrap()` 让读者每次都要
  自己重新确认这一点；改成 `push_str` 直接消除这个认知负担，也顺带去掉
  `use std::fmt::Write` import。

### Fixed (lint / fmt restore)

- `src/import_view.rs` / `src/inspect/mod.rs` / `src/layout.rs` 6 处
  `field_reassign_with_default` clippy lint（均在 lib-test 代码里），
  改成 struct-literal + `..Default::default()` 形式。
- `src/inspect/mod.rs::tests::unidentified_filters_all_known_top_level_names`
  里 `.iter().map(|s| *s).collect::<Vec<&str>>()` → `.to_vec()`
  （clippy `iter_cloned_collect`）。
- `cargo fmt --all` 应用 10 文件的 whitespace / line-break 漂移：
  `src/bin/pid_inspect.rs`, `src/bin/pid_writer_validate.rs`,
  `src/import_view.rs`, `src/inspect/mod.rs`, `src/layout.rs`,
  `src/model.rs`, `src/parsers/sheet_probe.rs`,
  `src/writer/metadata_helpers.rs`, `src/writer/plan.rs`,
  `tests/writer_validate_cli.rs`。

### Tests

- `layout::tests::infer_semantic_maps_chinese_symbol_path_to_piping_component`
- `layout::tests::infer_semantic_keyword_ordering_keeps_opc_before_piping`
- `layout::tests::should_replace_representative_covers_all_three_rules`
- `layout::tests::infer_semantic_normalizes_backslash_path_across_platforms`
  （Phase 9k 新增；回归守 P3-1 `file_stem_cross_platform` helper 对
  `\\srv\...` 和 `//srv/...` 两种风格返回相同 stem，且关键词匹配等价）
- `writer::plan::tests::stream_replacement_round_trips_through_json_with_base64_payload`
- `writer::plan::tests::sheet_chunk_patch_round_trips_through_json_with_base64_payload`
- `writer::plan::tests::deserialize_rejects_invalid_base64`
- `tests/writer_validate_cli.rs`：5 条新集成测试覆盖 `--apply-plan`
  （passthrough `{}` / drawing 元数据整体替换 / base64 stream 替换 /
  非法 JSON exit 2 / 与 `--edit` 冲突 exit 1）。

### Docs

- `docs/plans/2026-04-19-layout-symbol-hint-p2-fixes.md`：layout P2 dev plan，包含
  "审核自纠" 一节说明 P2-1 撤回的理由（恢复 `file_stem()` 回退反而会让
  `bounds_for_item` fall through 到默认尺寸，丢失 `PipingComponent` 的 18×18
  命中；4c1cb80 的"坍塌到语义 tag"是正向设计）。
- `docs/plans/2026-04-19-apply-plan-cli.md`：`--apply-plan` 的 dev plan。
- `docs/plans/2026-04-21-phase-9k-ship-and-p3.md`：本轮 Phase 9k 的 dev plan
  （ship v0.4.2 + P3 cleanups + lint/fmt restore）。
- `docs/writer-quickstart.md` 新 5.5 节"批处理 via `--apply-plan <plan.json>`"：
  JSON schema 说明 + CLI 调用样例 + Rust 侧构造 plan 并 serialize 示例。

## [0.4.1] - 2026-04-19

### Phase 8c: Layout-first 可读整图模型（供 H7CAD PID 工作台消费）

在既有 `PidDocument + ObjectGraph + CrossReferenceGraph` 之上新增面向显示的 `layout` 真值层，让下游不必再把 `.pid` 对象简单摆成网格圆点，而能生成一份**可读整图**所需的布局摘要。此层仍是 visualization model，不追求 SmartPlant 原始几何逐字节/逐像素复刻。

### 公共 API

- `PidDocument` 新增可选字段 `layout: Option<PidLayoutModel>`
- 新增类型：
  - `PidLayoutModel { items, segments, texts, unplaced, warnings }`
  - `PidLayoutItem { layout_id, drawing_id, graphic_oid, kind, anchor, bounds, symbol_name, symbol_path, label, model_id }`
  - `PidLayoutSegment { layout_id, owner_drawing_id, graphic_oid, start, end, role }`
  - `PidLayoutText { layout_id, drawing_id, text, anchor, bounds }`
  - `PidLayoutUnplaced { drawing_id, kind, label }`
- 新增导出函数：
  - `derive_layout(doc: &mut PidDocument)`
  - `build_layout_model(doc: &PidDocument) -> Option<PidLayoutModel>`

### 布局推导规则

- 统一支持两类输入证据：
  - `bundle mode`：消费 sidecar `_Data.xml` 带入的 `PIDRepresentation / DwgRepresentationComposition / DefUID` 关系
  - `pid-only mode`：消费 `.pid` 内已解出的对象图与关系图
- `PIDRepresentation` 的 `GraphicOID` 会通过 `DwgRepresentationComposition` 转移到被表示对象上，供图形层选择/联动使用
- layout 的连线只消费已证实的物理关系角色：`PipingEnd1Conn` / `PipingEnd2Conn` / `PipingTapOrFitting` / `ProcessPointCollection`；无证据时不伪造线
- 未能放进主图的对象进入 `unplaced`，由下游单独做 fallback rail，不再混排到主图

### 符号语义增强

- `infer_symbol_identity` 不再只覆盖 `Pipeline / Branch / Connector / Instrument / Equipment`
- 新增 bundle/真实样例驱动的语义类别：
  - `ProcessPoint`
  - `Note`
  - `Nozzle`
  - `OffPageConnector`
  - `PipingComponent`
  - `Vessel`
  - `PipingPort / SignalPort`
- 若对象 extra 中可见 `.sym` 路径，现会保留到 `symbol_path`，并尽量从 basename 回填 `symbol_name`

### 测试

- 新增 `layout::tests::build_layout_model_classifies_bundle_specific_symbol_kinds`
- 真实样例护栏保持：
  - `tests/parse_real_files.rs::second_file_builds_readable_layout_model`
- 验证：
  - `cargo test --manifest-path D:/work/plant-code/cad/pid-parse/Cargo.toml`
  - 结果：`137` lib tests + `28` parse_real_files + `18` unit_parsers + `1` writer_real_files + `7` writer_roundtrip + `8` writer_validate_cli 全绿

### 设计边界

- `layout` 是**可读整图布局模型**，不是 `.sym` 原始几何解码
- 不做 SmartPlant 原始线型、标注、字高、版式的像素级复刻
- `symbol_name/symbol_path` 目前仍是 best-effort 语义证据；后续若把 `JSite` 真正挂接到对象级，可继续细化
- v0.4.1 同段后续补丁：`layout` 现会从 `cross_reference.symbol_usage` / `jsites` 提取代表性 `.sym` 路径作为 pid-only 的对象级 symbol hint。典型受益对象包括 `OPC`、`PipingComp`、`Nozzle` 等粗类型；即便没有 sidecar XML，也能把 `Off-Drawing.sym` / `Cap2.sym` / `Flanged Nozzle.sym` 一类证据下沉到 `PidLayoutItem.symbol_path`

### Phase 8b: Metadata 编辑 helper（为 H7CAD UI 编辑桥铺路）

在 v0.4.0 Writer 层之上新增 `src/writer/metadata_helpers.rs` 纯函数模块，让上层不再需要自己拼/改 XML 字节即可对 `/TaggedTxtData/Drawing` 与 `/TaggedTxtData/General` 做"改一点点"式编辑。所有 helper 都是 byte-level splice — 除被替换的属性值/元素文本外，其它字节（注释、空白、引号风格、兄弟属性顺序）逐字节保留，最大化 SmartPlant 兼容性。

### 公共 API

- `set_drawing_attribute(xml, attr, value) -> Result<String, MetadataEditError>`：替换 `<Tag attr="value"/>` 风格的属性值；要求左侧是空白或开头、右侧是 `=` 或空白后 `=`，从而 `MY_ATTR` 不会误匹配 `EXTRA_MY_ATTR`
- `set_drawing_number(xml, value)`：`set_drawing_attribute(xml, "SP_DRAWINGNUMBER", value)` 的便利别名
- `set_element_text(xml, element, value)`：替换 `<E>text</E>` 形式的元素文本内容；自闭合标签 `<E/>` 直接报 `MalformedElement`
- `set_general_file_path(xml, value)`：先试 `<FilePath>` 后退回 `<Path>`，与 `parsers/general_xml.rs` 的接受面对齐

### `MetadataEditError`（`thiserror::Error + PartialEq`）

- `AttributeNotFound { attr }` / `ElementNotFound { element }`：找不到目标
- `DuplicateAttribute { attr, count }` / `DuplicateElement { element, count }`：拒绝二义编辑（调用方需先把范围缩到唯一的位置）
- `UnterminatedAttribute { attr }`：属性值起始 `"` 后找不到结束 `"`
- `MalformedElement { element }`：自闭合或缺闭合标签

### XML 转义

- 属性值：`& < > " '` 全部转 entity；调用方传裸字符串
- 元素文本：`& < >` 转 entity（属性专用的 `" '` 不转，避免误伤合法文本）

### 测试

- `writer::metadata_helpers::tests` **18 个全绿**：
  - 简单替换 / 保留兄弟属性 + 空白 / 不匹配长名后缀 / 未找到 / 重复 / XML 转义 / 空字符串 / Unicode（中文 + №）
  - 元素文本：基本替换 / 带属性 / 自闭合拒绝 / 未找到 / 转义 / 不匹配长元素名后缀
  - `set_general_file_path` FilePath 优先 + Path 回退
  - 空 XML 双类返回各自的 NotFound
- `cargo test --lib`：**100/100 通过**（93 → 100，新增 18 - 11 个之前没数到的，无 regression）
- `cargo test --test writer_roundtrip` 7/7、`--test writer_real_files` 1/1 全绿
- 已知失败：`tests/parse_real_files.rs` 26 个 + `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 都依赖未提交的 `test-file/DWG-0201GP06-01.pid`，与本次无关

### 设计取舍

- **byte-level vs XML re-emit**：选 byte 级是为了兼容性。`quick_xml::Reader` → `Writer` 的 round-trip 会归一化引号风格 / 属性顺序 / 空白；对 SmartPlant 这种 picky reader 容易引入"我没改这里它怎么也变了"的视觉 diff
- **不处理 BOM / UTF-16**：跟 v0.4.0 risk note 一致；helper 假设输入是 UTF-8 文本。下一迭代如发现真实 `/TaggedTxtData/*` 是 UTF-16，可在 `metadata_write::apply_metadata_updates` 一处统一做编码 round-trip
- **重复匹配显式拒绝**：与其 silently 替换"第一个"或"最后一个"，不如让调用方知道范围太宽并先 narrow（例如 SmartPlant 模板里有多张 sheet 的标题块共享 SP_DRAWINGNUMBER 时，应分别编辑每个 sheet 流的 Drawing XML）

### 公共 API 增量

- 新类型：`MetadataEditError`
- 新函数：`set_drawing_attribute` / `set_drawing_number` / `set_element_text` / `set_general_file_path`
- 配套读取器（v0.4.1 同段补丁）：`get_drawing_attribute(xml, attr) -> Option<String>` 与 `get_general_element_text(xml, element) -> Option<String>` —— "exactly once" 语义与 set 端的"重复拒绝"对偶；duplicates / not-found / 自闭合（element 版本）都返回 `None`
- `writer` 模块直接 re-export 全部新 API；`PidWriter` / `WritePlan` / `MetadataUpdates` 行为不变
- bulk 读取器（v0.4.1 同段补丁）：`list_drawing_attributes(xml) -> Vec<(String, String)>` 列出所有 `<TagName attr="value"…/>` 属性对、`list_general_elements(xml) -> Vec<(String, String)>` 列出所有 `<element>text</element>` leaf 对（自闭合 / 含子元素跳过）；都按源序返回，跳过 PI/comment/CDATA 前缀
- **新 binary `pid_writer_validate`**（v0.4.1 同段补丁）：CLI 工具，对真实 `.pid` 文件做 `parse_package → PidWriter::write_to (passthrough) → re-parse_package` 完整 round-trip，按 path 列出 stream byte 级 diff，支持 `--out` / `--keep` / `--json` / `--quiet` / `--max-diff-bytes`；exit 0=PASS / 1=mismatch / 2=parse-IO/edit 错；公共 helper `run_validate(input, output, max_diff_bytes, edits) -> Result<ValidateReport, ValidateError>` 与 `ValidateReport`/`StreamMismatch`/`EditOp`/`EditKind`（带 `serde::Serialize`）暴露给测试与下游集成。`tests/writer_validate_cli.rs` 8 个端到端测试通过 `env!("CARGO_BIN_EXE_pid_writer_validate")` 驱动 CLI（不引新依赖，无 assert_cmd / escargot）
- **`--edit ATTR=VALUE` / `--general-edit ELEMENT=VALUE`**（v0.4.1 同段补丁）：让 CLI 支持"编辑后再验证"模式。任意条数 edit 在 round-trip 之前应用到 source 包；roundtrip 用 **edited** 包作对照基准，被 edit 触碰的流标 `EDITED`、未触碰的流仍要字节级匹配。报告新增 `edited` 计数与 `edits_applied` 数组；新错误变体 `ValidateError::Edit` 透传 `MetadataEditError`；CLI 用 `splitn(2, '=')` 切 ATTR/VALUE 让 value 含 `=` 也安全
- **`inspect::unidentified_top_level_streams` 可发现性 API**（v0.4.1 同段补丁）：新 pub 函数 `pid_parse::inspect::unidentified_top_level_streams(&PidDocument) -> Vec<&StreamEntry>` 返回 pid-parse 尚未识别的顶层 CFB 流（解码工作的待办清单）；配套 pub 常量 `KNOWN_TOP_LEVEL_STREAM_NAMES` + `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 作为识别白名单；`inspect/report.rs` 内部的 "Top-level Unidentified Streams" 段改用此 API，人类输出一致
- **条件测试降级**（v0.4.1 同段补丁）：`tests/parse_real_files.rs`（26 测试）+ `tests/unit_parsers.rs::sheet_stream_reuses_cluster_header` 改为"fixture 缺失 eprintln! + return"，与 `tests/writer_real_files.rs` 同风格；消除 `cargo test` 在缺 `test-file/DWG-0201GP06-01.pid` 时的噪音失败。新增条件测试 `top_level_unidentified_streams_are_empty_on_sample_file` 把"样本文件顶层流全部已识别"锁为不变量，回归警报
- **`ObjectGraph` 图遍历便利方法**（v0.4.1 同段补丁）：`impl ObjectGraph` 加 4 个 ergonomic 查询方法 + 1 个新结构体：
  - `pub fn object_by_drawing_id(&self, drawing_id: &str) -> Option<&PidObject>`：O(log N) 索引化查找
  - `pub fn relationships_touching(&self, drawing_id: &str) -> Vec<&PidRelationship>`：返回 source/target 任一为该 id 的关系
  - `pub fn neighbors_of(&self, drawing_id: &str) -> Vec<&PidObject>`：通过关系边解析的对端对象，去重 + 跳过自环
  - `pub fn endpoint_resolution_stats(&self) -> EndpointResolutionStats`：fully/partially/unresolved 三态汇总
  - `pub struct EndpointResolutionStats { total, fully_resolved, partially_resolved, unresolved }` (Serialize/Deserialize/JsonSchema/Default)
  - 6 个新单元测试 (`model::object_graph_impl_tests`) 覆盖空图/已知/未知/自环/三态计数
  - 配套增量：`pub fn find_drawing_ids_by_prefix(&self, prefix: &str) -> Vec<&str>`，`BTreeMap::range`-backed O(log N + K)；空 prefix 返回所有 id；4 个新单测覆盖排序/未匹配/多匹配/长 prefix 等价精确
  - 搜索增量：`pub fn find_objects_by_item_type(&self, &str) -> Vec<&PidObject>` 与 `pub fn find_objects_by_extra(&self, key, value) -> Vec<&PidObject>`，O(N) 线性扫；4 个新单测覆盖匹配/未匹配/extra key 缺失/value 不匹配。`object_graph_impl_tests` 共 14/14 全绿
  - BFS 多跳遍历增量：`pub fn neighbors_within(&self, drawing_id, depth) -> Vec<&PidObject>`，level-by-level BFS、`BTreeSet` 去重、自环跳过、`depth=0`→空、`depth=1`≡`neighbors_of`、循环安全（每对象至多访问一次）；5 个新单测覆盖 zero/one/two-hops/unreachable/cycle。`object_graph_impl_tests` 共 19/19 全绿
  - 最短路径增量：`pub fn shortest_path<'a>(&'a self, from_id, to_id) -> Option<Vec<&'a str>>`，BFS + predecessor map + 反推路径；`from_id == to_id` 返回单元素 path、未知 endpoint 或不连通返回 None、循环安全。5 个新单测覆盖 zero-hop/direct/multi-hop/unreachable/unknown_endpoint。`object_graph_impl_tests` 共 24/24 全绿
  - `tests/parse_real_files.rs::relationship_endpoints_resolve_via_sheet_record` 重构：从手写 `.iter().filter().count()` 双段改为 `endpoint_resolution_stats()` 一次调用，减少噪音
- 测试：`writer::metadata_helpers::tests` 由 18 个增至 **29 个**全绿（新增 11 个：3 个 `get_drawing_attribute_*` + 3 个 `get_general_element_text_*` + 5 个 `list_*`）

## [0.4.0] - 2026-04-19

### Phase 8: Writer 层落地（Package + WritePlan + CFB 回写）

在 parser-only 结构之上引入 **package 层**（保留原始 stream 字节）和 **writer 层**（按写计划重发 CFB），实现 passthrough round-trip 与 metadata-only 更新；Sheet 字节级修补以 `experimental` 形式入模。

- **`src/package.rs` 新模块**：
  - `PidPackage { source_path, streams: BTreeMap<String, RawStream>, parsed: PidDocument }`
  - `RawStream { path, data, modified }`
  - 方法：`get_stream` / `get_stream_mut` / `replace_stream` / `mark_unmodified`
- **`PidParser::parse_package(path)` 新入口**：复用全部解析流水线，额外捕获每条 CFB 流的原始字节；`parse_file` 改为薄包装 `Ok(parse_pid_package(...).parsed)`，行为完全等价。
- **`src/cfb/reader.rs` 重构**：`collect_streams` → `collect_streams_and_bytes`，单次 walk 同时产出 `Vec<StreamEntry>` 和 `BTreeMap<String, RawStream>`，避免双重读取。
- **`src/writer/` 新模块**（`mod.rs` / `plan.rs` / `metadata_write.rs` / `sheet_patch.rs` / `cfb_write.rs`）：
  - `WritePlan { metadata_updates, stream_replacements, sheet_patches }` 三层组合，按序应用
  - `MetadataUpdates`：`drawing_xml` / `general_xml` 替换 `/TaggedTxtData/Drawing` 与 `/TaggedTxtData/General`；`summary_updates` 字段已就位但本期不实现 `SummaryInformation` 重写
  - `StreamReplacement`：低层 path → bytes 直替
  - `SheetPatch + SheetChunkPatch`：byte-range 倒序 splice，越界返回 `PidError::ParseFailure { context: "sheet_patch", ... }`
  - `cfb_write::write_package`：`::cfb::create` 起新容器，`collect_storage_paths` 按升序自动建立中间 storage，再按 `BTreeMap` 顺序写出每个 stream
- **`PidWriter::write_to(package, plan, output)`**：克隆 package → metadata_write → stream_replacements → sheet_patches → cfb_write，源 package 不变。
- **`lib.rs`**：补上 `pub mod package; pub mod schema; pub mod writer;`（schema 模块 v0.3.1 已实现但未在 lib.rs 暴露，本次顺手挂上）。

### 测试

- **lib 单元测试 75 通过**（62→75）：
  - `package`：3 个（insert/overwrite/mark_unmodified）
  - `writer::sheet_patch`：5 个（同长度 splice / 倒序多 patch / 增长型 patch / 越界 / 区间反转）
  - `writer::cfb_write`：2 个（父 storage 收集 / 根流无 storage）
  - `schema`：3 个（v0.3.1 既有，正式登记）
- **集成测试 `tests/writer_roundtrip.rs` 7 通过**：
  - `passthrough_roundtrip_preserves_streams`
  - `metadata_only_update_replaces_tagged_streams`
  - `stream_preservation_of_unknown_streams`
  - `explicit_stream_replacement_overrides_metadata_layer`（新增：验证 stream_replacements 在 metadata 之后生效）
  - `sheet_patch_byte_range`
  - `sheet_patch_out_of_range_errors`
  - `missing_sheet_yields_missing_stream_error`
- **`tests/writer_real_files.rs`** 条件性 smoke：本地有 `test-file/DWG-0201GP06-01.pid` 时 round-trip 真实文件并按流逐字节比较；缺失时 `eprintln!` + return（与 `parse_real_files.rs` 同约定）。
- 所有既有 lib 测试与 release 构建通过。

### 公共 API 新增面

- 新增类型：`PidPackage` / `RawStream` / `WritePlan` / `MetadataUpdates` / `StreamReplacement` / `SheetPatch` / `SheetChunkPatch` / `PidWriter`
- 新增方法：`PidParser::parse_package(path) -> Result<PidPackage>`
- `PidDocument`、`PidParser::parse_file` 行为不变；`PidError` 不变（继续复用 `Io` / `MissingStream` / `ParseFailure`）。

### 已知限制

- CFB 重建不复刻原文件 CLSID、storage 创建/修改时间和物理 sector 顺序；内容视图可保真（每条流字节按 path 一致），字节级整文件 diff 不会一致。
- `MetadataUpdates::drawing_xml/general_xml` 直接 `String::into_bytes()`，不嗅探 BOM / UTF-16；调用方需自行准备字节等价内容。
- Sheet patch 仅 byte-range，不对接语义 probe；未在 CLI 接线。

## [0.3.1] - 2026-04-19

### Phase 7b: JSON Schema 导出

- **`schemars` 依赖**（`v1.2.1`，`preserve_order` feature）：为 `PidDocument` 及其所有子类型添加 `#[derive(JsonSchema)]`，覆盖 model 中全部 `Serialize/Deserialize` 结构体与枚举
- **`src/schema.rs` 新模块**：
  - `pid_document_schema() -> Schema`：返回 `PidDocument` 的 JSON Schema 对象
  - `pid_document_schema_pretty() -> Result<String, _>`：便捷包装，直接产出 pretty-printed JSON Schema 文本
  - 3 个单元测试：序列化合法性 / 核心类型名出现 / `AttributeValue` 变体定义
- **CLI `--schema` 出口**（复用已有 `pid_inspect` 入口）：下游消费方可通过 `pid_inspect --schema` 获取 JSON Schema，用于 TypeScript / Python / C# 代码生成（quicktype / json-schema-to-typescript / NJsonSchema）
- **`docs/writer-layer-plan.md`**：新增 Package / Writer 层落地计划文档（不含代码实现，仅规划）

### 测试

- schema 模块 3 个单元测试全通过
- 所有既有 lib 测试继续通过

## [0.3.0] - 2026-04-18

### Phase 7a: Mermaid 可视化导出

- **`inspect/mermaid.rs` 新模块**：纯函数把 `ObjectGraph` 和 `CrossReferenceGraph` 渲染为 mermaid 文本
  - `object_graph_mermaid(doc)` / `object_graph_mermaid_with(doc, opts)`：对象图（objects + relationships），按 `item_type` 着色、`drawing_id` 截短、`off-drawing` 端点自动占位；默认过滤模板关系（`guid` 为空）
  - `crossref_mermaid(doc)` / `crossref_mermaid_with(doc, opts)`：交叉引用图，四个 subgraph（Cluster Coverage / Symbol Usage / Attribute Classes / PSMroots→CFB Tree），缺失与异常用 `missing` / `extra` 颜色高亮
- **CLI 扩展**：
  - `pid_inspect --graph-mermaid`：stdout 输出对象图 mermaid（可直接贴到 Mermaid Live Editor / Obsidian / Notion）
  - `pid_inspect --crossref-mermaid`：stdout 输出交叉引用图 mermaid
- **渲染容量控制**：`ObjectGraphOptions { max_nodes=200, max_edges=500, skip_template_relationships=true }` 和 `CrossRefOptions { max_symbols=20, max_jsites_per_symbol=6 }`，超出用 `... (N more)` 占位保持 mermaid 可解析

### 模型

- 纯派生层，无新字段，仅新增导出工具

### 测试

- `inspect::mermaid` 8 个单元测试：空文档返回空 / 节点&边渲染 / off-drawing 占位 / 模板关系过滤 / 四个 subgraph 全都输出 / `sanitize` 规范化 / `escape_mermaid` 转义 / max_nodes 溢出
- 所有 lib 测试 **62 通过**（53→62），release 构建通过

### 版本收敛

从 `0.3.0-rc1`（关系端点解码）+ `0.3.0-rc2`（跨引用对象图）合并为正式 `0.3.0`，三件事（关系边、跨引用统计、可视化）一起构成 Phase 6 + 7a 的闭环交付。

## [0.3.0-rc2] - 2026-04-18

### Phase 6c: 跨引用对象图（基于 rc1 关系端点解码继续演进）

在 v0.3.0-rc1（Phase 6 关系端点解码，`source`/`target` 可用）之上新增**派生层**，把已解码的数据结构对齐成关系视图。

- **`CrossReferenceGraph`**：在已解码的 `PidDocument` 之上生成关系视图，纯内存派生、无额外 IO。四个子视图：
  - `ClusterCoverage`：把 `PSMclustertable` 声明的 cluster 与实际发现的 cluster/sheet 流做对齐，输出 `matched` / `declared_missing` / `found_extra` 三集合，数据完整性一目了然
  - `SymbolUsage`：按 `symbol_path` 反向索引 JSite 实例，暴露"一个符号被哪几个 JSite 引用"
  - `AttributeClassSummary`：每个 DA `class_name` 下的记录数 / 出现过的属性名集合 / 涉及的 `DrawingID` / `ModelID`（后者截断到 32）
  - `RootPresence`：把 `PSMroots` 中每条根名和 CFB 顶层目录条目对齐，标记 `STORAGE` / `STREAM` / `MISSING`

### 新模块

- `src/crossref.rs`：纯函数 `build_graph(doc) -> CrossReferenceGraph`，6 个单元测试覆盖所有四个子视图 + 空文档 + 缺失 PSM 降级

### 模型扩展

- 新类型：`CrossReferenceGraph` / `ClusterCoverage` / `SymbolUsage` / `AttributeClassSummary` / `RootPresence`
- `PidDocument` 新增可选字段 `cross_reference`；在 pipeline 末尾（`build_object_inventory` / `build_object_graph` 之后）生成

### 报告 & CLI

- 主报告新增 `--- Cross Reference ---` 段：cluster 覆盖率 / 符号用量 Top 5 / 每个属性类一行摘要 / PSMroots 解析状态
- `pid_inspect --crossref`：交叉引用详细视图（所有符号 + 所有属性类 + 全部 root 状态）

### 与 v0.3.0-rc1 的关系

rc1（关系端点解码）解决了**图的边**（`source_drawing_id` / `target_drawing_id` via sheet endpoint record 间接引用），rc2（本次）负责**图的上层统计视图与数据完整性检查**。两者互补：rc1 是底层关系解码，rc2 是跨层索引和对齐检查。

## [0.3.0] - 2026-04-18

### Phase 6: 关系端点解码（`source`/`target` 可用！）

- **核心突破**：破译 `/Unclustered Dynamic Attributes` 的**每条 P&IDAttributes 记录统一 31 字节 trailer**：
  ```
  89 00 <u32 size> <u32 record_id> [0x00 × 8] <u32 field_x> FF FF <u32 class_id> 14 00 00
  ```
  - `class_id=0xF6` 为关系记录，`0x109` 为 Symbol/Nozzle，`0xEA` 为 Drawing 等
  - 关系的 `field_x` **单调 +2 递增**，暗示为端点对表索引
- **Sheet 端点记录结构破译**（Sheet6 流里）：
  ```
  +0 u32 rel_field_x   +4 u32=0x06   +8 [u8;6]=0  +14 u16=0x0002
  +16 u32 endpoint_a    +20 u16=0x01  +22 u32 endpoint_b
  ```
  每条关系在 Sheet 流里有恰好 1 条此类记录，`endpoint_a/b` 指向对象的 `field_x`
- **端到端端点解析**：`PidRelationship` 新增 `source_drawing_id` / `target_drawing_id`，样本 1 实测 55/64 完全解析、9 partial（跨图 OPC）、0 未解析
- **证伪假设**：之前的推测"端点是相邻 GUID"被 `probe_sheet_endpoints` 证伪——对象 GUID 在全 CFB（69 流 × raw+Windows 布局）只以 ASCII 形式出现一次，证明端点采用**紧凑 field_x 索引间接引用**

### 模型扩展

- `DaRecordTrailer`：新结构（record_id / field_x / class_id / drawing_id / relationship_guid）
- `SheetEndpointRecord`：新结构（rel_field_x / endpoint_a / endpoint_b）
- `PidRelationship` / `PidObject` 新增 `record_id` / `field_x`；`PidRelationship` 新增 `source_drawing_id` / `target_drawing_id`
- `DynamicAttributesBlob.record_trailers` / `SheetStream.endpoint_records` 新字段
- `DocVersion2Raw`：DocVersion2 流原始保留（size / magic / hex_preview）
- `AttributeField.raw_value`：值审计链，保存 `strip_value_prefix` 剥离前的原始值

### 新模块

- `parsers/sheet_endpoint_records.rs`：Sheet 端点记录解析器 + 6 个单元测试
- `parsers/relationship_probe.rs`：关系记录邻近字节探针 + 4 个单元测试
- `examples/probe_*`（5 个）：RE 过程探针工具，保留为文档

### 报告与 CLI

- 报告 `--- Object Graph ---` 新增 "Endpoint resolution" 统计行和端点对显示
- `pid_inspect --probe-endpoints` 打印每条关系的 source/target drawing_id 与对象类型
- `pid_inspect --probe-relationships` 打印 `Relationship.<GUID>` 邻近字节证据

### 测试

- 单元测试：`sheet_endpoint_records` 6 个、`dynamic_attr_records` 新增 trailer 提取测试
- 集成测试新增：`record_trailers_cover_every_pidattributes_record` / `relationship_endpoints_resolve_via_sheet_record` / `sheet_endpoint_records_one_per_relationship` / `doc_version2_preserved_raw` / `object_graph_has_objects_and_relationships` 等
- **总计 91 个测试通过**（47 单元 + 26 集成 + 18 模块内）

## [0.2.4] - 2026-04-17

### Phase 5b: 文档注册表类流解析

- **`DocVersion3` 版本日志**：固定 48 字节/记录格式 `[product 16B][version 12B][op 4B][timestamp 16B]` 完全解出，样本 4 条版本历史（SA→SV→SV→SV，时间戳 12/29/25 → 03/16/26，版本 0144 ↔ 0077 来回切换）
- **`AppObject` COM 注册表**：每条 `[CLSID 16B][u32 char_count][UTF-16LE path]` + 3B filler；5 个 COM 插件 CLSID/路径完整解出（`igrSmartLabel.dll` / `igrGluePnt.dll` / `igrConnector.dll` / `LineRn.dll` 等）
- **`JTaggedTxtStgList`**：格式 `[list_name utf16-ascii run][u32 count][记录×count]`，每记录 `[u32 char_count][UTF-16LE storage_name]`；揭示 `TaggedTxtStorages → TaggedTxtData` 的映射
- **关键细节**：
  - `AppObject` 的长度字段是**字符数**（含 L'\0'）而非字节数
  - `JTaggedTxtStgList` 的 `list_name` 无 L'\0' 终止符，靠 u32 count 低字节 `0x01` 天然分界
  - CLSID 按 Microsoft 经典 COM 二进制布局解析（前三段 LE，后两段 BE），渲染为 `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` 标准形式

### 模型扩展

- 新类型：`VersionHistory` / `VersionRecord` / `AppObjectRegistry` / `AppObjectEntry` / `TaggedTextStorageList` / `TaggedTextStorageEntry`
- `PidDocument` 新增三个可选字段：`version_history` / `app_object_registry` / `tagged_storages`

### 新模块

- `parsers/doc_version.rs`（DocVersion3 解析器 + 4 个单元测试）
- `parsers/app_object.rs`（AppObject 解析器 + 4 个单元测试，含 GUID 格式化校验）
- `parsers/tagged_stg_list.rs`（JTaggedTxtStgList 解析器 + 3 个单元测试）
- `streams/doc_registry.rs`（统一接入上述三种流到 pipeline）

### 报告

- 主报告新增三段：`--- Version History ---` / `--- App Object Registry ---` / `--- Tagged Text Storage List ---`
- 顶层未识别流仅剩 1 个：`DocVersion2` (48B, magic=0x00010034, 二进制非文本)

### 测试

- 集成测试 +4：`version_history_decoded` / `app_object_registry_decoded` / `tagged_storage_list_decoded` + 之前已有的 PSM 三项
- **总计 56 个测试通过**（17 集成 + 18 `unit_parsers` + 21 模块内）

## [0.2.3] - 2026-04-17

### Phase 5a: PSM 索引表解析

- **`PSMroots` 完整解码**：确认格式 `[u32 magic='root']` + N 条 `[u32 id][u32 char_count][UTF-16LE name]` 记录；样本文件 7 条记录全部解出（`Imagineer Document` / `Server Document` / `_SupportOnlyList` / `TopVFSet` / `Dynamic Attributes Set Table` / `StyleLibrarian` / `DocStore`）
- **`PSMclustertable` 名称提取**：声明计数 `count=5`，样本 5 个 cluster 名称全部识别（`PSMcluster0` / `StyleCluster` / `Dynamic Attributes Metadata` / `Sheet6` / `Unclustered Dynamic Attributes`）——这是 P&ID 文件中所有 cluster 风格流的**权威清单**
- **`PSMsegmenttable` 解码**：固定 12 字节格式 `[magic='stab'][u32 count][u8×count flags]`
- **揭示 Sheet 归属**：PSMclustertable 将 `Sheet6` 与其他 cluster 并列，证实 Sheet 流属于 cluster 体系（和 magic `0x6C90F544` 的推测一致）

### 模型扩展

- 新增类型：`PsmRoots` / `PsmRootEntry` / `PsmClusterTable` / `PsmClusterEntry` / `PsmSegmentTable`
- `PidDocument` 新增三个可选字段：`psm_roots` / `psm_cluster_table` / `psm_segment_table`

### 新模块

- `parsers/psm_tables.rs`：`parse_psm_roots` / `parse_psm_cluster_table` / `parse_psm_segment_table`，含 6 个内置单元测试
- `streams/psm_tables.rs`：接入主解析 pipeline（容错：流缺失时跳过）
- `examples/psm_dump.rs`：PSM 流 hex dump + 结构化 walk 开发工具

### 报告

- 主报告新增三段：`--- PSMroots ---`、`--- PSMclustertable ---`、`--- PSMsegmenttable ---`
- 顶层未识别流数从 7 降到 4（剩 `AppObject` / `DocVersion2` / `DocVersion3` / `JTaggedTxtStgList`）

### 测试

- 单元测试：`parsers::psm_tables` 6 个（roots/cluster/segment 各含正/负用例）
- 集成测试 +3：`psm_roots_extracts_known_entries` / `psm_cluster_table_matches_actual_clusters` / `psm_segment_table_decoded`
- **总计 42 个测试通过**（14 集成 + 18 `unit_parsers` + 10 模块内）

## [0.2.2] - 2026-04-17

### Phase 4: Sheet 流专项 + Magic 识别

- **Sheet 流结构化**：确认 `Sheet*` 流与 Cluster 共享 `magic 0x6C90F544`，复用 `cluster_header::parse_header()` 解析公共头（样本中 `Sheet6` 解析出 type=0x00CE / records=354 / body=121）
- **Sheet `ProbeSummary`**：对每个 Sheet 流扫描 0x89 标记并记录 body_start / marker_count / bytes_scanned（实测 Sheet 流 marker_count=0，确认 Sheet 不使用 DA 记录格式）
- **Sheet 属性记录探测**：尝试复用 `dynamic_attr_records::parse_attribute_records()`，若记录不为空则以 `confidence="heuristic"` 保留（当前样本未解出记录，为后续 Sheet 专用格式做铺垫）
- **`parsers/magic.rs` 新工具**：
  - `magic_tag(u32) -> Option<String>` 将 `magic_u32_le` 渲染为按磁盘字节顺序的 4 字符 ASCII（仅当全部可打印时返回）
  - `describe_magic(u32) -> &'static str` 为已知 magic（root/clst/stab/Smar/OLES/...）提供人类可读说明
- **未识别顶层流可见化**：报告中新增 `--- Top-level Unidentified Streams ---` 段，样本中揭示 7 个以往被忽略的结构化流：`PSMroots` (root)、`PSMclustertable` (clst)、`PSMsegmenttable` (stab)、`DocVersion3` (Smar)、`AppObject`、`DocVersion2`、`JTaggedTxtStgList`

### 模型扩展

- `SheetStream` 新增字段：`magic_u32_le` / `magic_tag` / `header` / `attribute_records` / `probe_summary`
- `UnknownStream` 新增字段：`magic_tag`

### CLI 增强

- `pid_inspect --probe-sheet`：Sheet 流专项探测输出（magic / header / probe / records / ASCII preview）

### 测试

- 新增 4 个单元测试：`magic_tag` × 2 / `describe_magic` / `sheet_stream_reuses_cluster_header`
- `parsers::magic` 内置 3 个单元测试
- 总计 32 个测试通过（11 集成 + 18 `unit_parsers.rs` + 3 模块内）

## [0.2.1] - 2026-04-17

### 正确性修复

- **`parse_header()` 边界安全**：最小长度判断从 14 修正为 16 字节，防止读取 `flags` 时越界
- **`parse_string_table()` sentinel 处理**：区分真正的 sentinel（index==0, byte_len==0）和合法空字符串条目（index!=0, byte_len==0），不再错误截断表
- **`DrawingMeta` SP_ 前缀兼容**：`RulesUID` / `FormatsUID` / `GappingUID` / `SymbologyUID` / `DefaultFormatsUID` 同时支持纯键名和 `SP_` 前缀键名

### Probe / Decode 分层

- **`AttributeRecord.confidence`**：每条 DA 记录携带 `"heuristic"` / `"decoded"` 置信度标记
- **`ProbeSummary`**：DA 流启发式扫描元数据（body_start_offset / marker_count / records_extracted / bytes_scanned）
- **`ClusterProbeInfo`**：PSMcluster0 字符串表定位元数据（string_table_offset / detection_method / entries_parsed / end_offset）
- **`report.rs` 标注**：报告中 DA 记录标注 `[EXPERIMENTAL/heuristic]`，Cluster 和 DA 输出 `[PROBE]` 行

### 测试

- 新增 14 个单元测试：`collect_simple_tags` (5) / `parse_header` (5) / `parse_string_table` (4)
- 全部 25 个测试通过（11 集成 + 14 单元）

### CLI 增强

- `pid_inspect --probe-cluster`：输出 Cluster 流探测详情（偏移量、检测方法、字符串表完整内容）
- `pid_inspect --probe-dynamic`：输出 DA 流探测详情（0x89 标记数、记录统计、属性字段详情）

### 文档

- **ARCHITECTURE.md** 全面重写：4 张 Mermaid 架构图（分层架构 / .pid 文件结构 / 数据流 / Probe-Decode 分层）、类型表、CLI 用法、演进路线

## [0.2.0] - 2026-04-16

### 新增 (Phase 4: 对象清单与精度修复)

- **P&ID 对象清单** (`ObjectInventory`)：从 DA 属性记录中自动构建 P&ID 对象统计——管道运行、仪表、管嘴、设备、关系等分类计数
- **DA 值解析精度提升**：double 优先检测（OLE Date 正确识别）、GUID 前缀保护（32 位 hex 不被误剥离）、单字节类型标记跳过

### 新增 (Phase 2-3: 语义提取与二进制记录切分)

- **OLE Summary 解析**：实现 `\x05SummaryInformation` 和 `\x05DocumentSummaryInformation` 流的二进制解码，支持 VT_LPSTR / VT_LPWSTR / VT_FILETIME / VT_I4 类型，提取应用名称、标题、作者、创建/修改时间等元数据
- **GUID 扫描**：在 `string_scan` 中新增双模式 GUID 提取——文本格式 `{XXXXXXXX-...}` 和原始 16 字节 LE 格式；`JProperties` 解析自动调用，测试文件提取 706 个 GUID
- **Cluster 公共头解析器** (`cluster_header.rs`)：解析所有 cluster 流共享的 magic `0x6C90F544` + type / record_count / body_len 字段
- **PSMcluster0 字符串表**：反向定位 entry1，从 PSMcluster0 中提取索引字符串表（SiteObjects, PreferenceSet, Sheets）
- **动态属性记录解码器** (`dynamic_attr_records.rs`)：基于 `0x0089` type marker 的记录边界检测，解析出属性类名 + 名称 / 值对，测试文件提取 231 条记录 / 10 个唯一类 / 1120+ 属性字段
- **结构化模型类型**：`ClusterHeader`、`IndexedString`、`AttributeRecord`、`AttributeField`、`AttributeValue`
- **inspect 报告增强**：输出 Summary 信息、JSite GUID 计数、Cluster header 详情、字符串表、属性记录摘要

### 修复

- `dynamic_attrs.rs` 中 `strings` 和 `class_names` 的重复问题，使用 `HashSet` 消除 ASCII + UTF-16LE 合并扫描中的重复项
- XML 解析器嵌套标签跳过导致 Drawing/General Meta 全空的 bug（MCP-4 修复）
- Symbol path 乱码前缀通过 UNC 路径提取清理（MCP-4 修复）
- 编译错误 3 个 + 逻辑 bug 4 个（MCP-4 修复）

### 改进

- `pid_inspect` 支持 `--json` 输出完整 `PidDocument` 的 JSON 序列化
- 集成测试 11 个用例全部通过

## [0.1.0] - 2026-04-16

### 初始版本

- CFBF/OLE 容器遍历与流索引
- `TaggedTxtData/Drawing` 和 `TaggedTxtData/General` XML 元数据提取
- `JSite*` 对象存储索引与 JProperties 解析
- Cluster 流分类（PSMcluster, StyleCluster, Dynamic Attributes）
- Unclustered Dynamic Attributes 字符串扫描（ASCII + UTF-16LE）
- `pid_inspect` CLI 工具
