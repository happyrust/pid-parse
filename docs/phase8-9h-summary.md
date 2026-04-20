# Phase 8 → 9h：Writer 建设周期总结

本文是 `pid-parse` 从 `v0.3.1`（parser-only）到 `v0.3.10`（parser + writer + verify + CI）9 轮迭代的**收官纪念**。记录动机、决策、方法论与统计，用作后来者查阅以及未来大 Phase 的借鉴。

## 起点与终点

| 维度 | v0.3.1（Phase 7b 结束） | v0.3.10（Phase 9h 结束）| Δ |
|---|---|---|---|
| 能力定位 | 只读 P&ID 解析器 | 读 + 写 + 验证 + CI | 产品级别跃迁 |
| 单元/集成测试 | 106 | 172 | **+66** |
| 新增模块 | — | 9 | `package` / `writer/{mod, plan, metadata_write, sheet_patch, cfb_write, xml_edit}` / `inspect/diff` / `parsers/doc_version2` |
| 新增 CLI 命令 | — | 5 | `--round-trip [--verify]` / `--set-drawing-number` / `--set-xml-tag` / `--diff` / `--schema` |
| Clippy warnings | 9（pre-existing） | **0**（`--all-targets -D warnings`）| 清零 |
| 容器保真度 | 仅 stream 字节 | stream + root CLSID + 非 root CLSID | 3 层 |
| 已识别顶层结构化流 | 全部已识别，除 `/DocVersion2` 黑盒 | **全部识别** | 无残留 |
| CI | — | GitHub Actions 全绿 | ✅ |
| 公开文档 | `README.md` / `ARCHITECTURE.md` | +`writer-quickstart.md` / `writer-clsid-and-timestamps.md` / `phase8-9h-summary.md` | 3 份新文档 |

## 阶段轨迹（9 轮）

### Phase 8 — v0.3.2：Writer 层骨架

- **动机**：之前只能读，不能改，产品价值受限
- **核心产出**：
  - `PidPackage { streams, parsed }` — 原始字节 + 解析结果共存
  - `PidWriter::write_to(&pkg, &plan, out)` — 声明式写出
  - `WritePlan { metadata_updates, stream_replacements, sheet_patches }`
- **验证**：真实 `.pid` passthrough 所有流字节零差异
- **测试**：+29（135 → 本轮起点 106 的实际情况）

### Phase 9a — v0.3.3：CLI 接入 + Root CLSID 保留

- **动机**：让 Writer 能力从 API 下沉到用户可见
- **核心产出**：`--round-trip` / `--set-drawing-number` / `--schema`（补上 v0.3.1 漏接的）
- **关键修复**：`cfb::create` 默认把 root CLSID 置为 nil UUID，会丢掉 SmartPlant P&ID 的身份标识 `{16ce6023-5f5b-11d1-9777-08003655f302}`；通过 `cfb.set_storage_clsid("/", source_clsid)` 回写
- **辅助**：`writer/xml_edit::replace_simple_tag_text`（精确 `<tag>...</tag>` 替换 + XML 字符转义）
- **新文档**：`docs/writer-clsid-and-timestamps.md`（cfb 0.10 能力矩阵）

### Phase 9b — v0.3.4：通用 XML metadata editor

- **动机**：`--set-drawing-number` 是特化命令，用户还想改 `Template`、`DocumentCategory` 等
- **核心产出**：`PidPackage::set_xml_tag(stream, tag, value)` 泛化 + 便捷方法 `set_drawing_xml_tag` / `set_general_xml_tag`
- **CLI**：`--set-xml-tag <stream> <tag> <value> --output <pid>` 通用版；`--set-drawing-number` 走新 API（DRY）但保留独立 log

### Phase 9c — v0.3.5：Package diff + round-trip verify

- **动机**：用户改完后怎么知道"只动了想动的"？
- **核心产出**：`diff_packages` 纯函数 + `PackageDiff { only_in_a, only_in_b, modified, root_clsid_match }`
- **渲染**：`inspect::diff::render` 人类可读报告（含 first_mismatch_offset + 16 字节 hex context）
- **CLI**：`--diff <other.pid>`（CI 友好 exit code）、`--round-trip --verify`

### Phase 9d — v0.3.6：卫生 pass

- **动机**：连做 4 轮新功能后需要停下来扫地
- **核心产出**：
  - Clippy **13 → 0**（`cargo clippy --all-targets -- -D warnings` 首次通过）
  - ARCHITECTURE mermaid 图刷新 + 新增"Writer 数据流"专用 flowchart
  - `docs/writer-quickstart.md` 新文档（parse → edit → write → verify 6 场景）
- **测试**：无新增，零回归

### Phase 9e — v0.3.7：非 root storage CLSID 保留 + 隐藏 bug 修复

- **动机**：Phase 9a 跳过了非 root CLSID，认为"真实样本几乎都是 nil"
- **真实样本发现**：`DWG-0201GP06-01.pid` 实际有 **3 个**非 root CLSID（`/JSite329` / `/JSite396` 指向同一 SmartPlant COM `{0a1cf23d-…}`；`/JSite948` 指向 `{7effbe60-…}`）
- **核心产出**：
  - `PidPackage.storage_clsids: BTreeMap<String, Uuid>` + `with_storage_clsids` builder
  - `parse_pid_package` walk 采集、`cfb_write::write_package` step 4 回写
  - `PackageDiff.storage_clsid_diffs` 观察维度，diff report 段
- **关键修复**：Phase 9a 以来的 `--round-trip --verify "0 diffs"` **实际上是不完整的** —— 旧 diff 只看 stream 字节 + root CLSID，不看非 root storage。Phase 9e 同时修复能力（写回）+ 观察（diff）+ 真实样本验证

### Phase 9f — v0.3.8：DocVersion2 逆向解码

- **动机**：`/DocVersion2` 是项目里最后一个 "48B 黑盒"，从 v0.2.1 起就只保留 raw
- **方法论**（教科书式的逆向）：
  1. Dump 两个样本（48B / 39B），差分对比找出共享前缀
  2. 9B 分组试错，发现 records 完美切齐
  3. 记录数值 `0x90` (144) / `0x4D` (77) 与 `DocVersion3` 的 `"0144"` / `"0077"` 交叉验证
  4. Type 字节语义：`0x82` ↔ `"SA"`(SaveAs)、`0x81` ↔ `"SV"`(Save)
- **核心产出**：
  - `parsers/doc_version2.rs`：5 单元测试（含 fuzzing 边界 / misaligned / wrong magic / label 映射）
  - `model.rs`：`DocVersion2 { magic, reserved_all_zero, records }` / `DocVersion2Record`
  - 集成测试 `doc_version2_decoded_matches_version_history`：两条冗余路径逐条对齐
- **影响**：消除**项目里最后一个未识别的顶层结构化流**

### Phase 9g — v0.3.9：Report 展示 + CLI 默认路径

- **动机**：Phase 9e/9f 把新字段加到模型里，但 `inspect/report.rs` 自 v0.2.4 起没更新，用户跑 `pid_inspect drawing.pid` 看不到
- **核心产出**：
  - `generate_package_report(&PidPackage)` 新函数：调 `generate_report(&pkg.parsed)` 后追加 `--- Container CLSIDs ---` 段
  - `generate_report` 扩展：`--- DocVersion2 (decoded, magic, records) ---` 段
  - CLI 默认走 `parse_package`，零破坏 `generate_report` 公共 API

### Phase 9h — v0.3.10：CI 工作流 + CI 首战绿化

- **动机**：172 tests / 0 clippy warnings 的质量需要自动守护
- **核心产出**：`.github/workflows/ci.yml`（cargo build/test/clippy `--all-targets -Dwarnings`，Swatinem/rust-cache 缓存）+ README CI badge
- **CI 首战揭示 27+1 处隐藏 bug**（`parse_real_files.rs` 和 `unit_parsers.rs::sheet_stream_reuses_cluster_header`）：fixture 缺失时 panic 而非优雅跳过。两轮 fix 全部对齐到 `fixture.exists() + return` 模式
- **最终**：commit `c136494` CI 38s 全绿；badge 实时生效

## 方法论沉淀

### 1. 大块工作中的节奏设计

连续做功能容易陷入 scope creep。Phase 8-9c 4 轮新功能后强制插入 Phase 9d **卫生 pass**（不做新功能，只清 lint / 刷文档），把质量水位重新抬到清零，再继续 Phase 9e 起的新内容。这避免了"技术债累计到不可维护"。

### 2. 隐藏 bug 靠观察维度的加法来发现

- Phase 9c 的 `diff_packages` 只看 stream 字节 + root CLSID → 无法发现非 root CLSID 丢失
- Phase 9e 加上 `storage_clsid_diffs` → 立刻发现真实样本的 3 个 CLSID 被 Phase 9a 悄悄丢掉
- Phase 9h 的 CI 加 `cargo test --all-targets` → 立刻发现 27+1 处 fixture panic

**结论**：每引入一个新维度（diff 字段、CI job），都可能揭示 0 个或多个此前"看不见的 bug"。这比"先写测试再写代码"更强，因为维度增加的测试是整体性的。

### 3. 逆向工程的"交叉验证"方法

DocVersion2 的 48B 逆向能成功的核心原因：**有已知锚点可以 cross-check**。
- `DocVersion3` 已解（文本形式）
- 两个样本对照分析出 9B 记录 stride
- 记录数值与 `DocVersion3` 版本字符串 decimal 一致
- `op_type` 字节与 `DocVersion3` 操作码一致
- 记录 count 与 `DocVersion3` records 长度相等

四个独立的对齐点全部通过才确信解码正确。没有交叉验证锚点的逆向（如 `PSMcluster0` 的记录内字段）不适合 "快速一轮"，应做独立大 Phase。

### 4. 代码库的 public API 增量式扩展

本周期没有破坏任何 v0.3.1 已存在的公开 API：
- `parse_file` / `PidDocument` 行为严格保持
- 新增 `parse_package` / `PidPackage` 作为 parallel entry（不是替代）
- `generate_report(&PidDocument)` 保留；`generate_package_report(&PidPackage)` 是超集（可选）

这让 9 轮累积改动可以一口气合并到 `main` 而不产生迁移痛。

### 5. CLI 的通用化要走渐进路径

- Phase 9a 的 `--set-drawing-number` 是特化命令
- Phase 9b 的 `--set-xml-tag <stream> <tag> <value>` 是泛化版
- 特化命令保留作为便捷快捷方式，底层都走 `PidPackage::set_xml_tag`（DRY）

直接推出 `--set-xml-tag` 通用版会让用户的学习曲线太陡；先特化再泛化的路径更符合实际使用习惯。

## 未完成与下一步候选

以下候选列入 ARCHITECTURE.md 的 "Phase 9i+" 但本周期未做：

| 候选 | 粒度 | 风险 | 预计工作量 |
|---|---|---|---|
| `cargo fmt --all` 清理 + CI 改 hard-fail | 小 Phase | 低 | < 1 hr |
| `PSMclustertable` per-record 字段精确映射 | 小-中 Phase | 中（逆向） | 1-2 hr |
| `SummaryInformation` property set 回写 | 大 Phase | 中（需处理 unknown-props 透传） | 4-8 hr |
| `--apply-plan <plan.json>` 批处理 | 中 Phase | 低（需选 base64 策略） | 2-4 hr |
| `PSMcluster0` / `StyleCluster` 完整记录解码 | 大 Phase | 高（深度逆向） | 8+ hr |
| 时间戳保真 | 依赖 cfb upstream | 高 | 无法预估 |
| Sheet 流几何解码（画图） | 极大 Phase | 极高 | 需专项项目 |

## 关键文件索引

| 区域 | 文件 |
|---|---|
| 包/写路径 | `src/package.rs` / `src/writer/*` / `src/api.rs::parse_package` |
| 诊断 | `src/package.rs::diff_packages` / `src/inspect/diff.rs` |
| 逆向成果 | `src/parsers/doc_version2.rs` |
| CLI 接入 | `src/bin/pid_inspect.rs` |
| 基础设施 | `.github/workflows/ci.yml` |
| 文档 | `docs/writer-quickstart.md` / `docs/writer-clsid-and-timestamps.md` / `ARCHITECTURE.md` |
| 测试 | `tests/writer_roundtrip.rs`（内存 fixture）/ `tests/writer_real_files.rs`（条件性真实样本）|

## 给未来贡献者的三条建议

1. **优先加维度，而非加测试**：`diff_packages` 这类结构化比较工具能一次性覆盖大量回归。
2. **CI 首战必 fail**：上线 `--all-targets` 前所有本地跑法都只覆盖 "fixture 存在" 的情况，上线后必 fail；预期它发生、快速修一两轮就过。
3. **Clippy 零容忍可延迟但要明确**：Phase 9d 之前项目 9 个 warning 存在了许多版本才清；早期就加 `-D warnings` 会降低迭代速度，但到了稳定期必须清零。
