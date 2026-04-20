# Phase 9k–10g: Writer 收尾 + SPPID coverage 系列归档

本文是 `pid-parse` 从 `v0.4.2`（Writer 建设期收官）到 `v0.7.0`（VT_LPSTR
UTF-8 支持）12 轮迭代的**收官纪念**。风格对齐
[`phase8-9h-summary.md`](./phase8-9h-summary.md)，补全 v0.3.10 之后的
全部 session 轨迹。

## 起点与终点

| 维度 | v0.4.1（Phase 8c 结束） | v0.7.0（Phase 10g 结束）| Δ |
|---|---|---|---|
| 能力定位 | Writer 层全功能 + layout-first 模型 | Writer CRUD 完整 + SPPID coverage 体系 | 两条大周期闭环 |
| 单元 / 集成测试 | 260 | 332 | **+72** |
| 新增 Phase | — | 12 (9k → 10g) | 每 Phase 均 plan 归档 |
| 新增 CLI flag | — | 4 (`--apply-plan`、`--set-summary`、`--delete-summary`、`--coverage`) | 3 + 1 selector |
| 新增 public API 模块 | — | 2 (`writer::summary_write`、`inspect::coverage`) | + Phase 9l/10a 开启 |
| CHANGELOG 行数 | ~100 | ~550 | 保留每 Phase 完整可追溯 |
| Plan 文件 | 2 | 14 | 每 Phase 一份 dev plan |
| Clippy warnings | 0 | 0（Phase 9k 扫过一次） | 保持零容忍 |
| serde/JsonSchema 向后兼容 | — | ✓（0.5.x/0.6.x/0.7.x 全段无 field-rename）| 零破坏 |

## 阶段轨迹（12 轮）

### Phase 9k — v0.4.2：Writer 体系 ship + P3 cleanups

- **动机**：Phase 8-9h 把 Writer 层做到 "能 round-trip"，但 `--apply-plan`
  commit 还没 push、P3 cleanups 积压（file_stem / hints / diff.rs
  unwrap / tests use 散落）。
- **核心产出**：Ship `--apply-plan` CLI + P3-1/3/4 清理 + 7 条 clippy
  清零 + 10 文件 `cargo fmt` 漂移修复。
- **方法论**：Phase 9d "新功能累计后强制扫地"的再一次应用。

### Phase 9l — v0.5.0：SummaryInformation property-set writer

- **动机**：`MetadataUpdates.summary_updates` 从 Phase 8 起就是 parked
  placeholder（silently ignored）——产品义务欠款。
- **核心产出**：
  - `src/writer/summary_write.rs` 新模块（~620 行），OLE [MS-OLEPS] property-set
    parse + serialize，byte-level round-trip 保真
  - 11 个符号 key 映射（`title` / `author` / `subject` / …）到
    SummaryInformation + DocumentSummaryInformation 的 PROPID
  - 编码策略：仅编辑 `VT_LPSTR` / `VT_LPWSTR` 字符串型 property，
    其他类型（FILETIME / I4 / unknown）byte-for-byte 透传
- **交叉验证锚点**：Phase 9f 的 DocVersion2 逆向对齐（op_type 0x82
  ↔ "SaveAs"）让 DocVersion3 `operation_label` 有了跨流验证的参照
- **SemVer**：minor bump (0.4.x → 0.5.0)，因 `summary_updates`
  从 no-op → active 是 consumer-visible 行为变化。

### Phase 9m — v0.5.1：`--set-summary` CLI flag + real-file integration

- **动机**：Phase 9l 的 lib API + apply-plan JSON 已齐全，但 CLI 缺
  `--set-summary KEY=VALUE` 对称便利 flag，用户想改单个 title 还得
  写 plan.json。
- **核心产出**：
  - CLI 特化 flag；与 `--apply-plan` 互斥，与 `--edit` / `--general-edit`
    可共存（不同流）
  - `SUMMARY_INFO_PATH` / `DOC_SUMMARY_PATH` 从 `pub(crate)` 提升为
    `pub`
  - 条件性 real-file 测试 `real_file_set_summary_title_preserves_other_streams`

### Phase 9n — v0.5.2：`summary_deletions` — CRUD 收尾

- **动机**：Writer 层 SummaryInformation 只有 CREATE + UPDATE，缺 DELETE。
- **核心产出**：
  - `WritePlan.MetadataUpdates.summary_deletions: Vec<String>`
  - `apply_summary_deletions` 公共入口，silent no-op 对未存在 key；
    unknown key 清晰错误
  - 顺序定义：xml → deletions → updates（冲突 pre-check 保证同 key
    在两个字段同时出现会 fail 而不产生 inconsistent state）
  - CLI `--delete-summary KEY` 对称 flag
- **方法论**：CRUD "完形补图" 原则——缺 D 的 API 不完整。

### Phase 9o — v0.5.3：Writer API ergonomics patches

- **动机**：4 轮 Writer 内部扩展后，consumer 侧的"入门样板"仍很重
  （`PidParser::new().parse_package(...)` 两步 / `Vec<u8>` 不能直接
  parse / `WritePlan` JSON 要自己 handle serde_json::Error）。
- **核心产出（纯 additive）**：
  - `PidPackage::from_path` / `from_bytes`（后者当前 tempfile 兜底，
    Phase 10i+ 改 pure-memory）
  - `PidWriter::write_to_bytes` 镜像 `write_to`，返回 `Vec<u8>`
  - `WritePlan::from_json` / `to_json` / `to_json_pretty`
  - `cfb_write::write_package_to_writer<F: Read+Write+Seek>` 泛型
    backend（Phase 10a 测试复用）

### Phase 10a — v0.6.0：SPPID 解析覆盖清单

- **动机**：v0.5.x Writer 收官之后，真正的瓶颈从"能不能写"换成
  "到底解到多少"。用户同一天起草的
  `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` 策略明确指出：
  "当前项目距离'完全解析 SPPID'最缺的不是再写 parser，而是把整体
  覆盖面、结构边界、验证边界先显式化。"
- **核心产出**：
  - `model::ParseCoverageStatus` / `CoverageNodeKind` / `CoverageEntry` /
    `CoverageReport` 类型
  - `inspect::coverage` 模块（静态映射，11 个 known stream 每个硬
    编码到 4 个状态之一）
  - `pid_inspect --coverage` CLI flag
  - `--- Coverage ---` section 嵌入 `generate_report`
- **SemVer**：minor bump (0.5.x → 0.6.0)，因新 public API surface +
  开启新大周期。

### Phase 10b — v0.6.1：动态 coverage 分类

- **动机**：Phase 10a 的静态映射无法区分"流存在 + parser 解出模型"
  vs "流存在但 parser silent-failure"。
- **核心产出**：
  - `classify(name, doc)` 新签名，咨询 `&PidDocument` 字段
  - `apply_dynamic_downgrade` + `stream_is_populated` helper
  - 降级时 note 指名缺失字段，如 "stream present but parser did
    not populate the expected 'version_history' field"
  - 4 个 cluster / dynamic-attrs 名字留给 Phase 10c

### Phase 10c — v0.6.2：cluster & dynamic-attrs 动态 probe

- **动机**：清 Phase 10b 的 parking list。
- **核心产出**：
  - 4 个 cluster / dynamic-attrs 流的动态 probe（按 `ClusterKind` 过滤）
  - `Unclustered Dynamic Attributes` 两条 populate surface（blob
    或 cluster kind）— OR 聚合
- **里程碑**：`KNOWN_TOP_LEVEL_STREAM_NAMES` 里 12 个条目全部有
  动态 probe，无 parking。

### Phase 10d — v0.6.3：DocVersion3 operation 语义化 + report 渲染

- **动机**：`VersionRecord.operation` 长期是 raw 2-char 字符串，与
  Phase 9f 的 DocVersion2 `op_type_label(0x82) = "SaveAs"` 不对称。
- **核心产出**：
  - `VersionRecord::is_save_as` / `is_save` / `operation_label` /
    `parsed_timestamp` 4 个 helper，**不改** serde/JsonSchema
    wire format
  - `generate_report` Version History section 用 `[SaveAs …]` 替
    `[SA …]`；unknown op code 补 `(XY)` 后缀
  - `tests/parse_real_files.rs::doc_version2_decoded_matches_version_history`
    用 `operation_label()` 替代内联 match，形成 DV2↔DV3 drift
    guard

### Phase 10e — v0.6.4：coverage JSON 导出

- **动机**：CI / dashboard / 未来 Phase 10f 需要机器可读 coverage。
- **核心产出**：
  - `CoverageReport::to_json` / `to_json_pretty` / `from_json`
    helpers（错误统一包装为 `PidError::ParseFailure`）
  - `pid_inspect --coverage --json` 输出 coverage-only JSON，与
    `--json` 单独 dump 全 doc 区分

### Phase 10f — v0.6.5：coverage byte dimension

- **动机**：roadmap Phase 4 "暴露未解释字节集中区"需要 byte 维度；
  本轮先接 `StreamEntry.size` 免费信号。
- **核心产出**：
  - `CoverageEntry.stream_size: Option<u64>`
  - `CoverageReport::total_bytes_by_status`
  - report coverage section 显示 "(42 B)" / "(1.2 KB)" / "(3.1 MB)"
    / "(2.5 GB)" 后缀（1024-binary，1 位小数）

### Phase 10g — v0.7.0：VT_LPSTR UTF-8 支持

- **动机**：Phase 9l 设了 ASCII-only 门，明确 flag 为 "tracked for
  Phase 9m UTF-8 support"。9m/9n/9o 都没动到它。Phase 10g 兑现。
- **核心产出**：
  - `encode_string` 的 VT_LPSTR 分支去除 `is_ascii()` 检查，接受
    任意 UTF-8
  - Round-trip 测试：构造 LPSTR fixture → 写 "公司 Co. 中文" →
    parse 回来仍是 VT_LPSTR 编码、bytes 解为原字符串
- **SemVer**：minor bump (0.6.x → 0.7.0)，因错误路径变成功路径是
  行为变化。

## 方法论沉淀

### 1. 连续 feature + 卫生 pass 的交替节奏

本周期再次证实 Phase 9d 开出的节奏：

- Phase 9k-9o = 5 轮 Writer 新功能（累积大量 changelog）
- Phase 10a-10g = 7 轮 SPPID coverage 系列（内容更紧凑，每轮 scope
  更小）
- Phase 10h = 卫生 pass / docs 归档（本文件！）

与 Phase 9d→9h、9i→9k 的节奏一致。

### 2. coverage 是 parser 工作的对偶面

Phase 10a 的洞察：与其直接写 parser，不如先让"当前 parser 的状态
vs 真实流的状态"可视化。coverage 本身不解析任何新字节，但通过
降级机制（Phase 10b）立刻揭示 silent failures，反向驱动 parser
改进优先级。

这对应 Phase 9e 写的 "隐藏 bug 靠观察维度的加法来发现"。

### 3. SemVer minor bump 的判定

本周期有 4 次 minor bump（0.5.0, 0.6.0, 0.7.0）和 8 次 patch bump。
判定规则：

| 场景 | bump |
|---|---|
| 新增 public API 类型 | minor |
| 新增 pub fn 方法（没破坏既有签名）| 可以 patch（保持 consumer 能 "patch-level recompile no-op"）|
| **原错误路径变成功路径（语义放宽）**| minor |
| 错误消息文案变化 | patch |
| bug fix / fmt 清理 / docs | patch |

Phase 10g 的 "ASCII 限制解除" 是最明显的第 3 类，不 minor bump 会
欺骗 consumer。

### 4. doc-first plan 文化

本周期每轮都先写 `docs/plans/2026-04-21-phase-*.md`，再动代码。
好处是 commit message 可以回指 plan 做扩展说明，CHANGELOG 可以
引用具体 scope 决策。**12 个 plan 文件** 成为后来者追溯单个 Phase
时的最小单位。

### 5. 交叉验证锚点的累积

Phase 9f 给 DocVersion2 建立的 op_type ↔ version number ↔ save 序
列三重锚点，在 Phase 10d 的 DV3 operation helper 升级时**直接被
复用**：DV2/DV3 cross-validate 测试只改了两行就把 Phase 10d 的 new
API（`operation_label`）拉进 drift guard 网。

"早期逆向投资的锚点" 是长期 payoff。

## 版本 ↔ Phase ↔ commit 索引

| Version | Phase | Commit | Main deliverable |
|---|---|---|---|
| 0.4.2 | 9k | `d77393f` | apply-plan ship + P3 cleanups + lint restore |
| 0.5.0 | 9l | `8643e57` | SummaryInformation property-set writer |
| 0.5.1 | 9m | `1bf2de5` | `--set-summary` CLI + real-file integration |
| 0.5.2 | 9n | `c4262b2` | `summary_deletions` — CRUD closure |
| 0.5.3 | 9o | `7703720` | Writer API ergonomics patches |
| 0.6.0 | 10a | `58b1b2e` | SPPID coverage inventory (new era) |
| 0.6.1 | 10b | `188ef71` | dynamic coverage classification |
| 0.6.2 | 10c | `d6da369` | cluster & dynamic-attrs probes |
| 0.6.3 | 10d | `abfec0a` | DocVersion3 operation helpers + report label |
| 0.6.4 | 10e | `83668d4` | coverage JSON export |
| 0.6.5 | 10f | `5b69a22` | coverage byte dimension |
| 0.7.0 | 10g | `58a3b9f` | VT_LPSTR UTF-8 support |

## 给未来贡献者的三条建议

1. **每个 Phase 先写 plan**：`docs/plans/YYYY-MM-DD-phase-*.md` 是
   本周期可追溯性的核心；即便是 30-min 小 Phase 也值得一份 plan。
2. **coverage 优先于新 parser**：现在 `pid_inspect --coverage` 能
   直接指出 "哪些流被动态降级了" — 先按降级列表找真正缺的 parser
   升级点，比盲目深挖单个流高效。
3. **SemVer 以"错误路径变化"为判定边界**：如果你的改动让之前
   返回 `Err` 的调用现在返回 `Ok`，一定 minor bump；哪怕是文档里
   的小字限制解除。
