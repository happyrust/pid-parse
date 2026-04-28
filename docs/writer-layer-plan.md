# pid-parse: Package / Writer 层落地计划

在现有 parser-only 结构之上，新增 `PidPackage`（原始 stream 字节 + 解析结果）和 `PidWriter`（写计划驱动的 CFB 回写），第一版支持 passthrough round-trip 与 `/TaggedTxtData/Drawing`、`/TaggedTxtData/General` 的 metadata-only 更新，`SheetPatch` 以 experimental byte-range 方式入模，暂不做语义重建。

## 范围确认
- 严格按 prompt 六步落地，不做 Step 1 前置稳定化（不改 `PidError::Cfb`、不动 `parse_header`、不扩 `drawing_xml` 的 `SP_` 字段）。
- 测试同时覆盖**内存 CFB fixture**（CI 可跑）与**条件性 `test-file/` smoke**（本地存在才跑）。

## 关联文件快照
- `@d:/work/plant-code/cad/pid-parse/src/lib.rs:1-13` — 现只导出 `api/cfb/crossref/error/inspect/model/parsers/streams`
- `@d:/work/plant-code/cad/pid-parse/src/api.rs:1-51` — 只有 `parse_file` 入口
- `@d:/work/plant-code/cad/pid-parse/src/cfb/reader.rs:1-41` — `parse_pid_file` 主流程
- `@d:/work/plant-code/cad/pid-parse/src/cfb/reader.rs:394-430` — `collect_streams` 现仅产出 `StreamEntry`，字节丢弃
- `@d:/work/plant-code/cad/pid-parse/src/error.rs:1-23` — `PidError` 枚举
- `@d:/work/plant-code/cad/pid-parse/tests/parse_real_files.rs:1-8` — 约定从 `test-file/` 读取真实 fixture
- `@d:/work/plant-code/cad/pid-parse/.gitignore:1-6` — `test-file/` 历史上曾被忽略，现已纳入仓库
- `cfb = "0.10"`（`@d:/work/plant-code/cad/pid-parse/Cargo.toml:9`），写入使用 `::cfb::create` / `create_storage` / `create_stream`。

## 实施计划

### Step A · 新增 package 层
- 新文件 `src/package.rs`：
  - `PidPackage { source_path: Option<PathBuf>, streams: BTreeMap<String, RawStream>, parsed: PidDocument }`
  - `RawStream { path, data, modified }`
  - 方法：`get_stream` / `get_stream_mut` / `replace_stream(path, data)` / `mark_unmodified()`
- `lib.rs` 追加 `pub mod package;`（`pub mod writer;` 在 Step C 再加）。

### Step B · 扩展 parser API（parse_package）
- `src/cfb/reader.rs` 重构：
  - 新增 `parse_pid_package(path, options) -> Result<PidPackage, PidError>`，把现有 `parse_pid_file` 的解析流程整体放进去，末尾返回 `PidPackage { source_path: Some(...), streams: raw_streams, parsed: doc }`。
  - 拆出 `collect_streams_and_bytes`：一次 walk 读完每个 stream 字节，同时产出 `Vec<StreamEntry>`（维持原有 `preview_ascii`、`magic_u32_le`）和 `BTreeMap<String, RawStream>`（`modified=false`）。路径归一化为 `/` 分隔。
  - 保留 `parse_pid_file` 作为薄包装：`Ok(parse_pid_package(path, options)?.parsed)`，确保现有公共行为和测试不变。
- `src/api.rs` 追加：
  - `pub fn parse_package<P: AsRef<Path>>(&self, path: P) -> Result<PidPackage, PidError>`
- 不新增 `PidError` 变体；继续用 `Io` / `MissingStream` / `ParseFailure` 等既有变体。

### Step C · 新增 writer 模块骨架
- 新文件 `src/writer/mod.rs`、`plan.rs`、`metadata_write.rs`、`sheet_patch.rs`、`cfb_write.rs`。
- `plan.rs` 定义：
  - `WritePlan { metadata_updates, stream_replacements, sheet_patches }`
  - `MetadataUpdates { drawing_xml: Option<String>, general_xml: Option<String>, summary_updates: BTreeMap<String, String> }`（第一版 `summary_updates` 仅保留字段，不实现）
  - `StreamReplacement { path, new_data }`
  - `SheetPatch { sheet_path, chunk_patches, experimental: bool }`
  - `SheetChunkPatch { start, end, replacement }`
- `mod.rs` 提供 `PidWriter::write_to(package, plan, output)`：
  1. `let mut working = package.clone();`
  2. `metadata_write::apply_metadata_updates(&mut working, &plan.metadata_updates)?;`
  3. 对每个 `stream_replacements` 调 `working.replace_stream(...)`
  4. 对每个 `sheet_patches` 调 `sheet_patch::apply_sheet_patch_to_package(&mut working, patch)?`
  5. `cfb_write::write_package(&working, output)`
- `lib.rs` 再补 `pub mod writer;`。

### Step D · metadata-only 写出
- `metadata_write::apply_metadata_updates`：
  - `updates.drawing_xml` → `package.replace_stream("/TaggedTxtData/Drawing", xml.into_bytes())`
  - `updates.general_xml` → `package.replace_stream("/TaggedTxtData/General", xml.into_bytes())`
  - `summary_updates` 不处理（第一版不动 `SummaryInformation` property set）。
- 对现有 tagged_text 字符编码保持**字节替换语义**：直接写入 `String::into_bytes()`；不负责 BOM/编码嗅探，由调用方准备好字节等价内容。

### Step E · SheetPatch（experimental byte-range）
- `sheet_patch::apply_sheet_patch`：按 `start` 倒序排序 patch，校验 `start <= end <= len`，越界时返回 `PidError::ParseFailure { context: "sheet_patch", ... }`。
- `apply_sheet_patch_to_package`：拿 `package.get_stream`（缺失返回 `PidError::MissingStream`），跑 byte-splice，写回 `replace_stream`。
- 第一版只暴露 API；不在 CLI 接线，不做 probe 对接。

### Step F · CFB 写出（passthrough）
- `cfb_write::write_package`：
  - `let file = File::create(output)?; let mut cfb = ::cfb::create(file)?;`
  - 从 `package.streams.keys()` 推导所有父级 storage 路径（`collect_storage_paths`），按升序逐个 `cfb.create_storage(path)`；忽略 "/" 根。
  - 对每个 `raw` 调 `cfb.create_stream(&raw.path)?` 后 `write_all(&raw.data)?;`
  - 结束时调 `cfb.flush()?`（cfb 0.10 的 `CompoundFile` 在 drop 时会 flush，但显式调更稳）。
- 不保留原 CLSID / 时间戳等元数据（第一版已说明）。
- 写入时优先按 `BTreeMap` 顺序以获得可复现结果。

### Step G · 测试
- **内存 CFB fixture**（新文件 `tests/writer_roundtrip.rs`）：
  - helper：`build_fixture_cfb(tmp_path)` 用 `::cfb::create` 生成包含 `/TaggedTxtData/Drawing`（小段合法 XML）、`/TaggedTxtData/General`（小段合法 XML）、`/PlainSheet/Sheet1`（任意 16 字节）的临时 `.pid`。
  - `passthrough_roundtrip_preserves_streams`：`parse_package` → `PidWriter::write_to(_, &WritePlan::default(), _)` → 再 `parse_package`，断言两次 `streams` 的 key 集合相同、每个 `RawStream.data` 字节相等。
  - `metadata_only_update_replaces_tagged_streams`：构造 `MetadataUpdates { drawing_xml: Some("<Drawing .../>".into()), ... }`，写出后重新 `parse_package`，断言 `/TaggedTxtData/Drawing` 字节等于预期，其他 stream 字节保持不变。
  - `stream_preservation_of_unknown_streams`：在 fixture 里放一个 `/UnknownStorage/Blob` 随机字节流，走 passthrough + metadata 更新，断言 blob 一字节不差。
  - `sheet_patch_byte_range`：对 `/PlainSheet/Sheet1` 用 `SheetChunkPatch { start: 4, end: 8, replacement: vec![0xAA; 4] }`，断言写出后对应字节命中预期、总长度维持，`experimental=true` 不阻塞执行。
  - `sheet_patch_out_of_range_errors`：给一个越界 patch，断言返回 `PidError::ParseFailure`。
- **条件性真实文件 smoke**（追加到 `tests/parse_real_files.rs` 或新文件 `tests/writer_real_files.rs`）：
  - 若 `test-file/DWG-0201GP06-01.pid` 存在：`parse_package` → `PidWriter::write_to` → 再 `parse_package`，断言 `streams` key 集合相同，且每个 stream 字节等于原始字节（passthrough 语义）。文件缺失时 `eprintln!` + 直接 `return`，保持与现有约定一致。

## 公共 API 变更面
- 新增：`PidPackage`、`RawStream`、`PidParser::parse_package`、`PidWriter`、`WritePlan`（及附带结构）、`PidError` 不变。
- `PidDocument` 不变；现有 `PidParser::parse_file` 行为不变（内部改经 `parse_pid_package`）。
- `lib.rs` re-export 不对外暴露 writer 便利符号（只 `pub mod writer;` / `pub mod package;`），避免一次性污染顶层命名空间。

## 风险与已知限制
- **CFB 重建丢元数据**：第一版 `cfb::create` 新容器不会复刻原文件的 CLSID、storage 创建/修改时间；对只读消费方（pid-parse 自身）足够，对 SPPID 宿主的完整兼容性有不确定性，需后续验证。
- **tagged_text 字节 vs 字符串**：直接用 `String::into_bytes()` 忽略 BOM / UTF-16 可能性；若实际 `/TaggedTxtData/*` 是 UTF-16，需要调用方自行准备字节。第一版记录风险，不处理。
- **Sheet patch 仅 byte-range**：不触碰语义；在 probe 稳定前不要接 CLI。
- **BTreeMap 顺序 ≠ 原 CFB 物理顺序**：写出后 storage/stream 的访问顺序可能变化；对消费方为内容驱动时无影响，但可能影响基于字节级 diff 的比较（真实 smoke 测试只断言 per-stream 字节而非整文件 diff）。
- **cfb 0.10 API 细节**：`create_storage` / `create_stream` 对已存在路径、路径分隔、根 storage 等行为需实现时按 crate 源码最终对齐；计划中的写出骨架属于结构草案，落地时可能微调。

## 交付物检查清单
- [x] `src/package.rs`
- [x] `src/writer/{mod,plan,metadata_write,sheet_patch,cfb_write}.rs`
- [x] `src/cfb/reader.rs` 重构 + `parse_pid_package`
- [x] `src/api.rs` 新增 `parse_package`
- [x] `src/lib.rs` 追加 `pub mod package; pub mod writer;`（顺带补 `pub mod schema;`）
- [x] `tests/writer_roundtrip.rs`（内存 fixture 全量用例 + `explicit_stream_replacement_overrides_metadata_layer` 加固）
- [x] `tests/writer_real_files.rs`（条件性真实文件 smoke）
- [x] `cargo build` / `cargo test --lib --test writer_roundtrip --test writer_real_files` 全绿（75 + 7 + 1）
- [x] 落地报告归档于 `CHANGELOG.md` v0.4.0 段
