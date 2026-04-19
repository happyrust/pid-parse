# 变更日志

## [0.3.11] - 2026-04-19

### Phase 9i: `cargo fmt` drift 清理 + CI hard-fail

Phase 9h 的 CI 把 `cargo fmt --all -- --check` 设为 `continue-on-error: true`（non-blocking），因为 `examples/` 自第一轮起就有 fmt drift。Phase 9i 收尾处理。

- 跑 `cargo fmt --all` 一键清理 26 个文件的 pre-existing drift（+445 / -310 行，纯空白/换行，无语义改动）
- `.github/workflows/ci.yml` 移除 `continue-on-error: true` —— CI 从此对 fmt drift 硬失败
- 测试 172 passed, clippy 0 warnings, fmt 0 drift 三项全绿

自本次起新贡献者 push 前需要跑 `cargo fmt --all`（或让编辑器 format-on-save）。

## [0.3.10] - 2026-04-19

### Phase 9h: CI 工作流 + README badge

给项目加 GitHub Actions CI，自动在 push / pull_request 上跑 build + test + clippy，巩固 Phase 9d 起清零 warnings / 172 tests 的质量水位。

- **`.github/workflows/ci.yml` 新增**：
  - `test` job（Ubuntu latest, Rust stable）：`cargo build --all-targets` + `cargo test --all-targets` + `cargo clippy --all-targets -- -D warnings`
  - 使用 `Swatinem/rust-cache@v2` 缓存 cargo registry / target，加速迭代
  - 集成测试里 `test-file/*.pid` 是 gitignored 的真实样本，`writer_real_files.rs` / `parse_real_files.rs` 在 CI 环境下会优雅跳过（`fixture.exists()` 判空）
  - `RUSTFLAGS=-Dwarnings` 让所有 compiler warning 都阻断构建
  - `cargo fmt --all -- --check` 设为 non-blocking（`continue-on-error: true`）— examples/ 目录有 pre-existing fmt drift，由后续 Phase 单独清理
- **`README.md` 顶部加 CI badge**（`github.com/happyrust/pid-parse/actions/workflows/ci.yml/badge.svg`）

### 范围

- **不做**：不跑 `cargo fmt --check` 作为 hard-fail（examples/ drift 需要单独 Phase 处理）
- **不做**：不加 `cargo doc` / codecov / MSRV 多版本矩阵（等真有需求再加）

### CI 绿化（v0.3.10 首发后的 2 轮 fix）

CI 工作流首次上线（commit `ef5e108`）后揭示了一个此前未暴露的隐藏 bug：CI 环境下没有真实样本 `test-file/*.pid`，但以下测试在 fixture 缺失时 `panic` 而非优雅跳过：

- **`tests/parse_real_files.rs`**（27 个测试，commit `8da0281`）：`parse_test_file` helper 改为返回 `Option<PidDocument>`，缺失时 `eprintln! skip` + 返回 `None`；27 处调用改 `let Some(doc) = parse_test_file(...) else { return };`；对齐到 `writer_real_files.rs` 已有的 `fixture.exists()` 模式
- **`tests/unit_parsers.rs::sheet_stream_reuses_cluster_header`**（1 个测试，commit `c136494`）：同 pattern 处理

最终 CI run (`c136494`) 全绿（38s，Ubuntu latest + Rust stable，build + test + clippy --all-targets -Dwarnings 全通过）。两轮修复都只动测试，不影响公开 API 行为；本地有 `test-file/` 时测试仍全跑（172 个）、CI 环境下自动跳过真实样本类别。

## [0.3.9] - 2026-04-19

### Phase 9g: Report 层展示 Phase 9e/9f 新字段 + CLI 默认走 package 路径

Phase 9e（非 root CLSID 保留）和 9f（DocVersion2 解码）把新能力加到了 `PidPackage` / `PidDocument` 里，但 `inspect::report::generate_report` 从 v0.2.4 起没更新，用户跑 `pid_inspect drawing.pid` 看不到这些数据。本轮补齐。

- **`inspect::report::generate_package_report(&PidPackage) -> String`** 新增：调用 `generate_report(&pkg.parsed)` 后追加 `--- Container CLSIDs ---` 段（root CLSID + 非 root storage CLSIDs）
- **`generate_report` 扩展**：在 `--- Version History ---` 后加 `--- DocVersion2 (decoded, magic, records) ---` 段；展示 SaveAs/Save 标签 + 每条 version u32（十进制 + hex）
- **CLI 默认走 `parse_package`**：`pid_inspect drawing.pid` 默认报告现在包含 Container CLSIDs + DocVersion2 decoded；不破坏 `generate_report(&doc)` 公共 API，调用方若只持有 `PidDocument` 仍可直接调用

### 典型输出增量（真实样本）

```
--- DocVersion2 (decoded, magic=0x00010034, 4 records) ---
  [SaveAs] version=144 (0x90)
  [Save] version=77 (0x4D)
  [Save] version=144 (0x90)
  [Save] version=77 (0x4D)

--- Container CLSIDs ---
  root: {16ce6023-5f5b-11d1-9777-08003655f302}
  non-root storages (3):
    /JSite329  {0a1cf23d-6dca-11d2-bda6-0800369bd002}
    /JSite396  {0a1cf23d-6dca-11d2-bda6-0800369bd002}
    /JSite948  {7effbe60-44f5-11ce-83c5-08003601a74e}
```

### 测试

- 无新增测试（非行为性变更；既有 report 段落仍在）
- **仍然 172 个测试通过**

### 累计 8 轮统计（Phase 8 → 9g）

| 维度 | v0.3.1 | v0.3.9 | Δ |
|---|---|---|---|
| 测试 | 106 | 172 | **+66** |
| 模块 | — | +9 | package, writer×5/xml_edit, inspect/diff, parsers/doc_version2 |
| CLI 命令 | — | +5 | round-trip / set-drawing-number / set-xml-tag / diff / schema |
| Clippy warnings | 9 | 0 | 清零 |
| 容器保真 | stream only | stream + root CLSID + 非 root CLSID | 3 层 |
| 已识别顶层流 | —（DocVersion2 黑盒） | **全部识别** | 无残留 |
| 默认 report 展示 | v0.2.4 状态 | 含 Container CLSIDs + DocVersion2 decoded | 最新 |

## [0.3.8] - 2026-04-19

### Phase 9f: DocVersion2 结构化解码

从 v0.2.1 起 `/DocVersion2` 一直是"48B 黑盒"，只保留 `magic + hex_preview`。逆向解码这次成功，全部靠两个真实样本 + 与 DocVersion3 交叉验证。

**格式**（`src/parsers/doc_version2.rs`）：

```
Header (12 B):
   0x00  u32 LE   magic = 0x0001_0034
   0x04  u64 LE   reserved (observed all zero)

Records (9 B each × count):
   +0   u8    op_type       (0x82 = SaveAs, 0x81 = Save)
   +1   3 B   fixed         (0x00 0x00 0x09)
   +4   u8    separator     (0x00)
   +5   u32 LE  version     (u8-sized value like 0x90=144, 0x4D=77)
```

**交叉验证关键**（`tests/parse_real_files.rs::doc_version2_decoded_matches_version_history`）：
- 真实样本 `DWG-0201GP06-01.pid` 的 DocVersion3 记录是 `[SA 090000.0144, SV 0077, SV 0144, SV 0077]`
- DocVersion2 byte 对应 `[0x82 version=0x90 (144), 0x81 0x4D (77), 0x81 0x90, 0x81 0x4D]`
- `op_type 0x82/0x81 ↔ "SA"/"SV"` 完全对齐；`version` u32 低字节 ↔ DocVersion3 版本字符串最后 4 位转 decimal

### 模型扩展

- 新类型：`DocVersion2 { magic_u32_le, reserved_all_zero, records }` / `DocVersion2Record { op_type, fixed[3], separator, version }`（`#[derive(JsonSchema)]`）
- `PidDocument` 新字段 `doc_version2_decoded: Option<DocVersion2>`（与 raw `doc_version2: Option<DocVersion2Raw>` 并存，raw 仍然是 round-trip 的 ground truth；decoded 失败时为 `None`）
- `parsers::doc_version2::op_type_label(u8) -> String`：`0x82 → "SaveAs"`、`0x81 → "Save"`、unknown → `"0xNN"`

### 测试

- 模块内单元测试 +5：`parsers::doc_version2::tests::*` 覆盖 sample1 4 记录 / sample2 3 记录 / 错误 magic / 9-byte 非倍数 / label 映射
- `tests/parse_real_files.rs` +1：`doc_version2_decoded_matches_version_history` — 真实样本上的双流交叉验证
- **总计 172 个测试通过**（从 166 → 172，新增 6 个）

### 影响

- **完全消除 P&ID 文件中最后一个未识别的顶层结构化流**（`/DocVersion2`）
- 后续 `--schema` 输出能看到 `DocVersion2` / `DocVersion2Record` 的 JSON Schema 定义
- DocVersion3（文本版本日志）+ DocVersion2（二进制版本日志）两条冗余路径全部解出，可做一致性校验

## [0.3.7] - 2026-04-19

### Phase 9e: 非 root storage CLSID 保留

Phase 9a（Root CLSID 保留）的自然延续。**真实样本验证发现 3 个此前被丢失的 CLSID**：`DWG-0201GP06-01.pid` 的 `/JSite329` / `/JSite396` 携带 `{0a1cf23d-6dca-11d2-bda6-0800369bd002}`，`/JSite948` 携带 `{7effbe60-44f5-11ce-83c5-08003601a74e}` — 之前的 passthrough 虽然流字节零差异，但这 3 个 storage CLSID **悄无声息地丢失**。

- **`PidPackage.storage_clsids: BTreeMap<String, Uuid>`** 新字段：非 root + 非 nil 的 storage CLSID 映射；真实样本里稀疏（典型为空或 1-3 条），nil 条目被过滤以保持 map 简洁
- **`with_storage_clsids(...)`** builder 方法
- **`parse_pid_package` 在 CFB 打开后 `walk` 遍历所有 entries**：对 `is_storage() && path != "/"` 捕获非 nil CLSID；路径规范化为 `/` 分隔
- **`writer::cfb_write::write_package`** 新增 step 4：对每个 `storage_clsids` 条目调 `cfb.set_storage_clsid(path, clsid)`
- **`PackageDiff.storage_clsid_diffs: Vec<StorageClsidDiff>`**：diff 模型扩展；`is_empty()` / `diff_count()` 纳入该维度；`inspect::diff::render` 新增 `--- Non-root Storage CLSID Diffs ---` 段
- **re-export**：`pid_parse::{StorageClsidDiff}`

### 修复：passthrough 此前的隐藏 bug

在 Phase 9e 之前，`--round-trip` 即便报告 "verified: 0 diffs" 也可能遗失非 root storage CLSIDs —— 因为旧版的 `diff_packages` 只看 stream 字节 + root CLSID，不看非 root storage CLSID。Phase 9e 同时修复能力（写回）和观察（diff），让"0 diffs" 真正意味着 "容器级元数据完全一致"。

### 测试

- 模块内单元测试 +3：`package::with_storage_clsids_round_trips_map` / `package::diff_flags_non_root_storage_clsid_mismatch` / `package::diff_reports_missing_non_root_clsid_on_one_side`
- `tests/writer_roundtrip.rs` +1：`non_root_storage_clsid_round_trips`（内存 fixture 给 `/UnknownStorage` 烧一个 `F29F85E0-4FF9-1068-AB91-08002B27B3D9`，round-trip 保持）
- `tests/writer_real_files.rs` +1：`real_file_reports_non_root_storage_clsids_deterministically`（passthrough 保持条数与值、非 nil 约束）
- **总计 166 个测试通过**（从 161 → 166，新增 5 个）

### 文档

- `ARCHITECTURE.md`：v0.3.7 + Phase 9e
- `docs/writer-clsid-and-timestamps.md`：能力矩阵更新 —— 非 root storage CLSID 从 "第一版未保留" 升级为 "保留"

## [0.3.6] - 2026-04-19

### Phase 9d: 卫生 pass + 文档刷新

连续 4 轮（Phase 8/9a/9b/9c）新功能迭代后的巩固回合：清零所有 clippy warnings、刷新 ARCHITECTURE mermaid 图、补 writer quickstart 文档，让代码库和文档回到对齐状态。

- **Clippy 清零**：从 13 个 pre-existing warnings（9 lib + 2 example + 1 unit_parsers + 1 parse_real_files）→ **0 warnings**
  - `src/cfb/reader.rs`：`ObjectGraph` struct literal 替代 field assignment；折叠内嵌 `if` 到外层 `match` arm（ProjectNumber / DrawingNo）
  - `src/inspect/mermaid.rs`：`path.rsplit(|c| c == '/' || c == '\\')` → `path.rsplit(['/', '\\'])`；crossref 测试 struct literal 化
  - `src/parsers/string_scan.rs`：移除 `if x { a } else { a }` 同值分支；清理 unused `start` 变量
  - `src/streams/cluster.rs`：`blen >= 4 && blen < 512` → `(4..512).contains(&blen)`；`blen % 2 == 0` → `blen.is_multiple_of(2)`
  - `src/streams/jsite.rs`：`.map_or(false, …)` → `.is_some_and(…)`；`.filter(…).next()` → `.find(…)`
  - `examples/mermaid_demo.rs`：`ObjectGraph` / `CrossReferenceGraph` 测试数据 struct literal 化
  - `tests/unit_parsers.rs`：`tags.get(k).is_none()` → `!tags.contains_key(k)`
  - `tests/parse_real_files.rs`：嵌套 `if let Some` + iter → `.into_iter().flatten()`
  - `src/crossref.rs` 测试：单点用 `#[allow(clippy::field_reassign_with_default)]` + block 隔离
- **`cargo clippy --all-targets -- -D warnings` 退出 0**（Phase 8 起第一次）
- **ARCHITECTURE mermaid 更新**：
  - 分层架构图的 L7 Report/CLI 加入 `inspect/diff.rs` + `schema.rs`；CLI 节点展开 5 个 writer 命令
  - L8 Package/Writer 重命名为 "Package / Writer / Diff" 并加入 `xml_edit.rs` + `PackageDiff` / `diff_packages`
  - 新增 "Writer 数据流" 专用 mermaid flowchart：parse_package → WritePlan → PidWriter::write_to → cfb::create → set_storage_clsid → 写出 → parse → diff_packages → inspect::diff::render 全链路可视化
  - 类型表新增 `PidPackage` / `PidWriter` / `WritePlan` / `PackageDiff` 对应描述
- **`docs/writer-quickstart.md`**（新文档）：从 parse → edit → write → verify 完整入门，6 个场景（passthrough / metadata 更新 / WritePlan 混用 / diff / SheetPatch / 能力矩阵）+ 完整可粘贴代码片段

### 测试

- **仍然 161 个测试通过**（无回归，本轮无功能新增）

## [0.3.5] - 2026-04-19

### Phase 9c: Package diff + round-trip verify

给 Writer 能力补上"**对比验证**"这一关键闭环。Phase 8/9a/9b 解决了"能写"，本轮解决"能验"。

- **`PackageDiff` / `StreamDiff`**（`src/package.rs`）：stream-level 差异模型
  - `only_in_a` / `only_in_b`：单侧独有路径
  - `modified: Vec<StreamDiff>`：双方都有但字节不等；携带 `len_a / len_b / first_mismatch_offset` + 16 字节 hex 前后文
  - `root_clsid_match`：root CLSID 等价检查
  - `is_empty()` / `diff_count()` 便捷方法
- **`diff_packages(&PidPackage, &PidPackage) -> PackageDiff`** 纯函数
- **`inspect::diff::render(&PackageDiff) -> String`** 人类可读渲染
  - `(no differences)` 快捷路径（空 diff 一行）
  - `--- Only in A ---` / `--- Only in B ---` / `--- Modified Streams ---` 三段报告
  - 每个 modified stream：`path  len=a vs b  first_diff@0xNN` + 两行 hex context
- **CLI 扩展**（`pid_inspect`）：
  - `--diff <other.pid>`：对比两个 `.pid` 包；空 diff 退出码 0，非空退出码 1（CI 友好）
  - `--round-trip <out> --verify`：写完后自动 diff，`verified: 0 diffs` 或打印具体差异并以 1 退出
- **re-export**：`pid_parse::{diff_packages, PackageDiff, StreamDiff}`

### 典型输出（真实样本）

```
$ pid_inspect a.pid --round-trip out.pid --verify
round-trip ok: a.pid -> out.pid
  streams written: 69
  root CLSID preserved: {16ce6023-5f5b-11d1-9777-08003655f302}
  verified: 0 diffs

$ pid_inspect a.pid --set-drawing-number NEW --output out.pid
$ pid_inspect a.pid --diff out.pid
summary:     1 diff(s) — 0 only-in-a / 0 only-in-b / 1 modified
--- Modified Streams ---
  /TaggedTxtData/Drawing  len=3619 vs 3611  first_diff@0x4D0
    a: 44 57 47 2d 30 32 30 31 47 50 30 36 2d 30 31 3c    # "DWG-0201GP06-01<"
    b: 4e 55 4d 2d 4e 45 57 3c 2f 44 72 61 77 69 6e 67    # "NUM-NEW</Drawing"
```

### 测试

- 模块内单元测试 +8：`package::diff_*` 5 个（空 / only-in / byte-level / CLSID / 长度不等时 first_mismatch 位置）+ `inspect::diff::render_*` 3 个
- `tests/writer_real_files.rs` +2：`real_file_passthrough_produces_empty_diff` / `real_file_set_drawing_number_diff_is_localized_to_one_stream`（真实样本上断言编辑后只有 1 处 modified）
- **总计 161 个测试通过**（从 151 → 161，新增 10 个）

### 文档

- `ARCHITECTURE.md`：v0.3.5 + Phase 9c
- `README.md`：新增 `--diff` / `--round-trip --verify` 示例

## [0.3.4] - 2026-04-19

### Phase 9b: 通用 XML metadata editor

把 v0.3.3 的 `--set-drawing-number` 泛化：用户可编辑 `/TaggedTxtData/*` 任意 stream 里任意简单 tag，不再限定到图号。

- **`PidPackage::set_xml_tag(stream_path, tag, new_value)`**：通用 API。返回替换前的旧值；`MissingStream` / `ParseFailure` 覆盖所有错误路径；内部复用 `writer::xml_edit::replace_simple_tag_text`
- **便捷方法**：`set_drawing_xml_tag(tag, value)` / `set_general_xml_tag(tag, value)`（分别定向到 `/TaggedTxtData/Drawing` / `/TaggedTxtData/General`）
- **`extract_simple_tag_text`** 私有辅助：读取 `<tag>...</tag>` 内容，用于返回旧值以便调用方 diff / 日志
- **CLI 扩展**：
  - `--set-xml-tag <stream> <tag> <value> --output <pid>`：通用编辑，例 `--set-xml-tag /TaggedTxtData/Drawing Template NEW.pid`
  - `--set-drawing-number` 重构走 `PidPackage::set_xml_tag`（DRY），保留独立 log 消息 `set-drawing-number ok:`
  - `flag_triple` helper 解析 3 个连续位置参数

### 测试

- 模块内单元测试 +4：`package::set_xml_tag_returns_old_value_and_updates_bytes` / `package::set_xml_tag_missing_stream_returns_missing_stream` / `package::set_xml_tag_rejects_non_utf8_stream` / `package::set_drawing_xml_tag_shortcut_delegates_to_set_xml_tag`
- `tests/writer_real_files.rs` +1：`real_file_set_xml_tag_edits_template_only` — 真实样本上把 `<Template>XIONGANA2.pid</Template>` 改为 `<Template>REPLACED.pid</Template>`，断言 drawing_meta 解析结果同步、其他标签不变
- **总计 151 个测试通过**（从 146 → 151，新增 5 个）

### 文档

- `ARCHITECTURE.md`：v0.3.4 + Phase 9b 完成
- `README.md`：新增 `--set-xml-tag` 使用示例

## [0.3.3] - 2026-04-19

### Phase 9a: Writer CLI 接入 + Root CLSID 保留

把 Phase 8 的 writer API 下沉到 `pid_inspect` CLI；在 `cfb = "0.10"` 能力范围内尽可能多保留容器级身份信息。

- **Root CLSID 保留**：
  - `PidPackage.root_clsid: Option<Uuid>`（新字段，`serde` skip-if-none）
  - `parse_pid_package` 在 CFB 打开后立即 `cfb.root_entry().clsid()` 读取；nil UUID 归一化为 `None`
  - `writer/cfb_write::write_package` 在写完流后 `cfb.set_storage_clsid("/", clsid)` 还原
  - 真实 `.pid` 样本 `DWG-0201GP06-01.pid` 的 CLSID `{16ce6023-5f5b-11d1-9777-08003655f302}` 经 passthrough 保留
- **`src/writer/xml_edit.rs` 新模块**：
  - `replace_simple_tag_text(xml, tag, new_value)`：文本级 `<tag>old</tag> → <tag>new</tag>` 精确替换；nested 同名 tag / 缺失 open/close / 自闭合拒收；`&`/`<`/`>` 自动 XML 转义
  - 6 个单元测试覆盖正常替换 / 转义 / 缺 tag / 缺 close / nested 拒收 / 保留周围空白
- **CLI 扩展**（`src/bin/pid_inspect.rs`）：
  - `--round-trip <output.pid>`：passthrough 回写；打印 streams 写入数、CLSID 保留状态
  - `--set-drawing-number <NEW> --output <output.pid>`：改 `<DrawingNumber>` + 其他所有流字节保持；真实样本验证 `<DrawingSite.DrawingNumber>` / `<DrawingSite.Name>` 等相似 tag 不受影响
  - `--schema`：JSON Schema 导出（v0.3.1 CHANGELOG 提到但漏接，本次补上）
- **依赖**：新增 `uuid = "1"`（与 `cfb` 0.10 已传递依赖的同一版本）；`pid_parse::Uuid` re-export 供集成测试 / 下游直接使用

### 范围边界（与 writer-clsid-and-timestamps.md 对齐）

`cfb` 0.10 无公开 API 写 stream CLSID / state_bits，也不能写任意时间戳（只有 `touch(path)` = now）。第一版：
- ✅ 保留 Root CLSID
- ❌ 不保留非 root storage / stream CLSID（真实样本几乎都是 nil，投入产出低）
- ❌ 不保留时间戳（新容器刷新）
- ❌ 不保留 stream state_bits / CFB 目录物理顺序（BTreeMap 字典序）

### 测试

- 模块内单元测试 +6：`writer::xml_edit` 6 个
- `tests/writer_roundtrip.rs` +2：`root_clsid_round_trips_when_source_has_one` / `fixture_without_clsid_reports_none`
- `tests/writer_real_files.rs` +2：`real_file_passthrough_preserves_root_clsid` / `real_file_set_drawing_number_rewrites_only_the_target_tag`（+ 字节级非改动流保持断言）
- **总计 146 个测试通过**（从 135 → 146，新增 11 个）

### 文档

- `docs/writer-clsid-and-timestamps.md`（新）：完整能力矩阵 + 选择理由 + 下一步候选
- `ARCHITECTURE.md`：v0.3.3 能力边界 + Phase 9a 完成
- `README.md`：新增 CLI `--round-trip` / `--set-drawing-number` / `--schema` 使用示例

## [0.3.2] - 2026-04-19

### Phase 8: Writer 层（passthrough round-trip + metadata 回写）

从 parser-only 升级为 parser + writer。第一版交付目标：真实 `.pid` 文件 passthrough 零字节差、元数据流可声明式回写。

- **`src/package.rs` 新模块**：
  - `PidPackage { source_path, streams, parsed }`：把每个 CFB 流的**原始字节**（`BTreeMap<path, RawStream>`）与解析结果 `PidDocument` 打包在一起
  - `RawStream { path, data, modified }`：路径已规范化（`/` 分隔、前导 `/`），`modified` 标识 writer 是否改动过
  - 方法：`new` / `get_stream` / `get_stream_mut` / `replace_stream`（自动规范化路径 + 标脏）/ `mark_unmodified`
- **`PidParser::parse_package`**：与 `parse_file` 平行的新入口，返回 `PidPackage`。内部 `cfb::reader` 重构：`parse_pid_package` 一次性产出解析结果 + 原始字节 map，`parse_pid_file` 降级为薄包装（公开行为不变）
- **`src/writer/*` 5 个新模块**：
  - `writer/plan.rs`：`WritePlan { metadata_updates, stream_replacements, sheet_patches }`、`MetadataUpdates { drawing_xml, general_xml, summary_updates }`、`StreamReplacement { path, new_data }`、`SheetPatch { sheet_path, chunk_patches, experimental }`、`SheetChunkPatch { start, end, replacement }`；便捷构造 `WritePlan::metadata_only` / `is_passthrough`
  - `writer/metadata_write.rs`：`apply_metadata_updates` 把 `drawing_xml` / `general_xml` 写回 `/TaggedTxtData/Drawing` / `/TaggedTxtData/General`；空 XML 拒收（`PidError::ParseFailure`）；`summary_updates` 暂为 no-op 占位
  - `writer/sheet_patch.rs`：`apply_sheet_patch` 字节级 `[start, end)` splice，多补丁按 `start` 倒序执行保持偏移稳定；越界返回 `PidError::ParseFailure`；`apply_sheet_patch_to_package` 高层 API，缺失流返回 `PidError::MissingStream`
  - `writer/cfb_write.rs`：`write_package` 用 `::cfb::CompoundFile::create` 新建容器，`collect_storage_paths` 推导所有父 storage，按 `BTreeMap` 顺序写入每个 stream
  - `writer/mod.rs`：`PidWriter::write_to(&package, &plan, output)`，在 `package.clone()` 上依次应用 metadata / replacements / patches 后落盘；不改动调用方的 package

### 公开 API 面

- `pid_parse::{PidPackage, RawStream, PidWriter, WritePlan, MetadataUpdates, StreamReplacement, SheetPatch, SheetChunkPatch}` 全部在 crate 根重新导出
- `PidParser::parse_package(path) -> Result<PidPackage, PidError>` 新增
- `PidDocument` / `PidParser::parse_file` 行为与字段不变

### 范围边界（第一版明确不做）

- `SummaryInformation` property set 不写（`summary_updates` 字段保留为 API 占位）
- 不保留原容器的 CLSID / 创建 + 修改时间戳（`cfb::create` 新容器）
- `/TaggedTxtData/*` 字节替换，不做 BOM / UTF-16 编码嗅探，调用方自备字节
- SheetPatch 仅 API 层开放，CLI 暂不接（仍依赖后续 Sheet 几何解码）

### 测试

- 模块内单元测试 +15：`package` 3 / `writer::plan` 2 / `writer::metadata_write` 4 / `writer::sheet_patch` 6 / `writer::cfb_write` 2 / `writer::mod` 1（端到端 passthrough）
- 集成测试 `tests/writer_roundtrip.rs` +6：内存 CFB fixture（`::cfb::CompoundFile::create` + `create_storage_all` + `create_stream`）
  - `passthrough_roundtrip_preserves_every_stream`：全部 stream key & 字节一一相等
  - `metadata_only_update_replaces_tagged_streams_and_keeps_others`：只改 Drawing XML，其它不变
  - `unknown_streams_are_preserved_through_passthrough_with_metadata`：未知 blob 字节保持
  - `sheet_patch_replaces_byte_range_and_preserves_length` / `sheet_patch_out_of_range_is_rejected`
  - `original_package_is_not_mutated_by_write`：writer 在 clone 上工作
- 集成测试 `tests/writer_real_files.rs` +2（条件性）：真实 `DWG-0201GP06-01.pid` passthrough → 每个 stream 字节相等 + drawing_meta 复现
- **总计 135 个测试通过**（从 106 → 135，新增 29 个）

### 文档

- `ARCHITECTURE.md`：分层架构图新增 `Package` (L2.5) / `Writer` (L8) 层，演进路线更新 Phase 8 完成
- `README.md`：新增"写回"使用示例与 API 介绍

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
