# pid-parse 解析/写出进度盘点 + 下一步开发路线（v0.7.1 →）

> 起稿：2026-04-21
> 对应状态：HEAD = v0.7.0（Phase 10g, VT_LPSTR UTF-8）+ 未提交的 v0.7.1 session archive 产物
> 适用范围：作为 `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` 的**战术实施向导**，把 roadmap Phase 2-5 落到具体 Phase 序列。

## 1. TL;DR

- **解析层**：13 个 known 顶层流 → 7 FullyDecoded / 5 PartiallyDecoded / 3 存储前缀 IdentifiedOnly。派生层（ObjectGraph / CrossReferenceGraph / PidLayoutModel）齐备。
- **写出层**：Writer CRUD 完整（drawing/general XML + Summary section 1 全 CRUD + stream replace + sheet byte-patch + root / non-root CLSID 保留 + round-trip verify）。
- **测试与 CI**：332 tests 全绿，clippy/fmt 双零，CI 绿。
- **现场 WIP**：Phase 10h 已完成 3/4（Cargo.toml bump + `phase10-coverage-series-summary.md` + `sppid/v0.7.x-status.md`），仅差 CHANGELOG + commit + tag。

下一步按 5 个优先级推进：

| 优先级 | Phase | 内容 | 估时 |
|---|---|---|---|
| P0 | 10h ship | CHANGELOG [0.7.1] + commit + tag v0.7.1 | 30min |
| P1 | 10i | VT_LPSTR CP1252/GBK/Shift-JIS code-page fallback | 3-5hr |
| P1 | 10j | DocumentSummaryInformation section 2（user dict）编辑 | 4-6hr |
| P2 | 11a | PSMclustertable per-record 结构化（roadmap 2.2）| 6-8hr |
| P2 | 11b | PSMsegmenttable 结构化（roadmap 2.3）| 4-6hr |
| P2 | 11c | Sheet 深层几何/图元解码（roadmap 2.5）| 10-14hr |
| P3 | 12a | 规范化语义图层（roadmap 3，大 Phase）| 20-30hr |
| P3 | 12b | consumed/leftover 字节验证框架（roadmap 4）| 12-18hr |
| — | 验收 | roadmap Phase 5：leftover < 5% on 2-3 代表性样本 | — |

## 2. 现状详细快照

### 2.1 解析层 coverage 分类（v0.7.0 静态+动态双探针）

**FullyDecoded (7)**

| 流名 | 奠定 Phase | 动态探针 |
|---|---|---|
| `\x05SummaryInformation` | 9l / 9n / 10g | `doc.summary.is_some()` |
| `\x05DocumentSummaryInformation` | 同上 | `doc.summary.is_some()`（shared surface）|
| `PSMroots` | v0.2.3 | `doc.psm_roots.entries.len() > 0` |
| `DocVersion2` | v0.3.8（逆向 12B+N×9B）| `doc.doc_version2_decoded.is_some()` |
| `DocVersion3` | v0.2.4 + 10d helpers | `doc.version_history.records.len() > 0` |
| `AppObject`（COM 注册表）| v0.2.4 | `doc.app_object_registry.is_some()` |
| `JTaggedTxtStgList` | v0.2.4 | `doc.tagged_storages.is_some()` |

**PartiallyDecoded (5)**

| 流名 | 当前深度 | 缺口 |
|---|---|---|
| `PSMclustertable` | header + name list | per-record 字段（cluster_id / flags / type）raw |
| `PSMsegmenttable` | flags 数组 | segment record 语义（geometric/connection/reference）未名 |
| `PSMcluster0` | 记录边界已知 | 内部记录内容未解 |
| `StyleCluster` | 同上 | 同上 |
| `Dynamic Attributes Metadata` + `Unclustered Dynamic Attributes` | 类/属性表 + 关系端点（v0.3.0 31B trailer 逆向）| 每类 record 内部字段仅部分命名 |

**IdentifiedOnly 存储前缀 (3)**

| 前缀 | 已解能力 | 缺口 |
|---|---|---|
| `Sheet*` | cluster header + 0x89 marker + 端点对记录（Phase 6）| 页面图元、坐标、标签文本、符号引用、标注 |
| `TaggedTxtData` | XML 可读可写（v0.3.2+）| storage 无缺口；内容层可编辑 |
| `JSite*` | 符号路径 + GUID + JProperties | 未对接到对象级（symbol_path 已经下沉到 PidLayoutItem via v0.4.1 patch）|

### 2.2 写出层能力矩阵

| 操作 | API | CLI | 保真 |
|---|---|---|---|
| Drawing/General XML 整体替换 | `MetadataUpdates.{drawing_xml, general_xml}` | `--edit ATTR=V` / `--general-edit ELEM=V` / `--apply-plan` | 字节级 |
| XML 单 tag/attr 定点编辑 | `set_drawing_attribute` / `set_element_text` / `set_xml_tag` | `--set-xml-tag STREAM TAG VALUE` | 兄弟字节 byte-identical |
| Summary prop CREATE / UPDATE | `summary_updates: BTreeMap<String, String>` | `--set-summary KEY=V` | 仅碰 VT_LPSTR / VT_LPWSTR；FILETIME / I4 byte-for-byte 保留 |
| Summary prop DELETE | `summary_deletions: Vec<String>` | `--delete-summary KEY` | 同上；unknown key 报错 / missing key silent no-op |
| 任意流整体替换 | `stream_replacements`（base64 JSON）| `--apply-plan` | 无语义保真 |
| Sheet byte-range patch | `sheet_patches`（倒序 splice）| `--apply-plan` | experimental |
| Round-trip verify | `diff_packages` → `PackageDiff` | `--round-trip --verify` / `pid_writer_validate` | 0 diffs PASS |
| CLSID 保留 | 自动（`PidPackage.root_clsid` + `storage_clsids`）| 自动 | 完整（root + 非 root）|

**已知保真缺口**：

- 时间戳（cfb 0.10 上游只能 `touch(path) = now`）
- 目录物理顺序（重写按 lexicographic path，非源顺序）
- DocumentSummaryInformation **section 2**（user-defined dictionary）不编辑，但 section 2..N 字节 verbatim 透传
- VT_LPSTR 非 UTF-8 code page 未处理（仅 Phase 10g 解了 UTF-8 路径）

### 2.3 测试与 CI

```
lib                  238
inspect_cli            4
parse_real_files      28   (skip-when-missing fixture)
unit_parsers          18
writer_real_files     10   (skip-when-missing fixture)
writer_roundtrip      13
writer_validate_cli   21
---
TOTAL                332   tests, green
```

- `cargo fmt --check` / `cargo clippy --all-targets -D warnings` 双零
- GitHub Actions CI 绿（build + test + clippy + fmt hard-fail）
- fixture：`test-file/DWG-0201GP06-01.pid`（单真实样本 — 多样本是 Phase 11 起的硬约束）

## 3. 下一步开发计划（分阶段）

### 阶段 A — 立即收尾（P0，~30min）

#### Phase 10h ship v0.7.1

1. `CHANGELOG.md` 加 `[0.7.1]` 段，风格 "docs archive; no behavior change"（行 ~25）
2. 验证：`cargo fmt --check && cargo clippy --all-targets -D warnings && cargo test --all-targets` → 332 tests
3. 可选：扫 `grep -rn "TODO\|FIXME\|parked" src/ docs/` 清理明显过时的注释
4. 提交：`git add -A && git commit -m "docs(release): v0.7.1 — session archive (Phase 10h)"`
5. 打 tag：`git tag v0.7.1`

### 阶段 B — Writer 层边界打磨（P1，7-11hr）

两个 Phase 都是 **minor bump**，都是 additive + 行为放宽（原 `Err` 现 `Ok`）。

#### Phase 10i — VT_LPSTR 多 code page 回退（minor bump 0.7 → 0.8.0）

- **动机**：SmartPlant 2019- 旧文件实测为 CP1252（西欧）/ GBK（简中）/ Shift-JIS（日文）；Phase 10g 只通 UTF-8 一条路
- **方案**：
  - Reader 端：探测源 fixture code page。启发式顺序 = 已知 fixture 白名单 → 首 64B 非 ASCII 字节频次 → BOM → fallback UTF-8
  - Writer 端：`MetadataUpdates.summary_updates` 语法扩展为 `BTreeMap<String, StringOrEncoded>`，其中
    ```rust
    enum StringOrEncoded {
        Plain(String),
        Encoded { value: String, encoding: &'static str },
    }
    ```
  - JSON 形态：`{"title": "Q4 Report"}` 或 `{"title": {"value": "Ø Pipe", "encoding": "CP1252"}}`
- **依赖**：`encoding_rs`（已在 Cargo.toml 里）
- **测试**：
  - 3-4 个真实 CP1252/GBK fixture 的 round-trip
  - lossy 编码 fail-fast（比如给 CP1252 塞 "中文" 要 fail 而不是 silent mojibake）
  - UTF-8 plain path 100% 向后兼容
- **SemVer**：minor（新字段 + 行为放宽）
- **Plan doc**：`docs/plans/2026-04-21-phase-10i-vtlpstr-codepage.md`（待写）

#### Phase 10j — DocumentSummaryInformation section 2 编辑（minor bump）

- **动机**：section 2（user-defined property dictionary）是 SmartPlant 堆自定义属性的地方，当前 Writer 仅接 section 1 的 11 个 key；section 2 verbatim 透传不可编辑
- **方案**：
  - `SummaryPropertySet` 扩展到多 section 读写
  - 新字段 `summary_user_updates: BTreeMap<String, String>` / `summary_user_deletions: Vec<String>`（key 是 user dict 里的 property 名字符串）
  - CLI `--set-user-summary KEY=VALUE` / `--delete-user-summary KEY`
- **拦截器**：DocumentSummaryInformation section 1 的 12 个 PROPID 属于预定义 FMTID，section 2 的 property 名按字符串查；这两个不能混淆——在入口做预检查
- **测试**：
  - user dict 新增 / 覆盖 / 删除
  - 非 user-dict prop 字节透传 guard
  - section 1 + section 2 混合编辑同一次 WritePlan
- **SemVer**：minor（新 API + 新 CLI + 行为扩展）
- **Plan doc**：`docs/plans/2026-04-21-phase-10j-docsummary-section2.md`（待写）

### 阶段 C — Parser 结构化加深（P2，20-28hr，3 轮）

覆盖 `2026-04-21-sppid-full-parse-roadmap.md` 的 Phase 2 全部剩余任务。**执行原则**（roadmap 明确要求）：每轮产出 raw + decoded + audit/probe 三层。

#### Phase 11a — PSMclustertable per-record 结构化（roadmap 2.2）

- **当前**：header 已知、name list 已解出；per-record 字段仍以 raw hex / offset 暴露
- **目标字段**（基于 SmartPlant 行为推测，需逆向确认）：
  ```rust
  pub struct PsmClusterRecordDecoded {
      pub cluster_id: u32,
      pub index: u16,
      pub flags: u16,
      pub type_tag: u16,
      pub declared_segment_count: u32,
      pub raw_trailer: Vec<u8>,  // 未解字段保留
  }
  ```
- **交叉验证锚点**：
  - PSMcluster record 总数 = PSMsegment `flags.len()` / PSMclustertable `declared count`
  - 每条 record 的 `cluster name` vs `doc.clusters[i].path`
- **coverage 升级**：`PartiallyDecoded` → `FullyDecoded`（全字段命中时）
- **风险**：单样本过拟合。硬约束 = **至少 2 个真实 fixture 回归**；缺 fixture 则只升级到"decoded + confidence=medium"，不 claim FullyDecoded
- **Plan doc**：`docs/plans/2026-04-XX-phase-11a-psmclustertable-records.md`

#### Phase 11b — PSMsegmenttable 结构化（roadmap 2.3）

- **当前**：12B header + N×1B flags 数组解出；无语义
- **目标**：
  - segment record 字段命名（推测：segment_id / source_cluster / target_record_id / kind）
  - 建立 segment ↔ layout.segments / relationship / sheet endpoint 的来源链
  - 区分 kind = Geometric（几何实体）/ Connection（连接实体）/ Reference（引用实体）
- **交叉验证锚点**：
  - Sheet `endpoint_records.rel_field_x` vs PSMsegment `segment_id`
  - layout.segments `role` vs PSMsegment `kind`
- **Plan doc**：`docs/plans/2026-04-XX-phase-11b-psmsegmenttable.md`

#### Phase 11c — Sheet 深层几何/图元解码（roadmap 2.5）

- **当前**：cluster header（v0.2.2）+ 0x89 marker probe + 端点对记录（Phase 6, v0.3.0）；type=0x00CE 其他结构未解
- **目标**：
  - 图元坐标（boundary / center / rotation）
  - 标签文本（位置 + 内容 + 字体提示）
  - 符号引用（指向 JSite 或 cluster）
  - 标注（箭头、线型、颜色）
- **策略**（循序渐进，避免一次深挖失控）：
  1. **先投 2hr 做 probe 增强**：`--probe-sheet-chunks` 按记录边界切片 + record type 频次统计 + hex context
  2. **按频次排序**：高频（> 20% of records）的 record type 优先逆向，低频作为 experimental
  3. **先解坐标字段**：双字节 / 四字节小整数 pattern 容易识别
  4. **再解文本字段**：UTF-16LE runs 在 SmartPlant 里很一致
  5. **最后解引用关系**：依赖 Phase 11a/11b 的 cluster/segment record_id 锚点
- **coverage 升级**：`IdentifiedOnly` → `PartiallyDecoded`（若坐标 + 文本字段稳定）
- **风险**：SmartPlant 图元格式封闭，需多样本。**建议单独 session 设计 + 至少 3 个 fixture**
- **Plan doc**：`docs/plans/2026-04-XX-phase-11c-sheet-geometry.md`

### 阶段 D — 大架构升级（P3，32-48hr，2 轮）

#### Phase 12a — 规范化语义图层（roadmap Phase 3，大 Phase）

- **合并来源**：`Dynamic Attributes Metadata` / `Unclustered Dynamic Attributes` / `Sheet*` / `PSMroots` / `PSMclustertable` / `JSite*`
- **统一模型**：
  ```rust
  pub struct NormalizedObject {
      pub id: NormalizedId,
      pub kind: NormalizedObjectKind,
      pub provenance: Provenance,
      // ...
  }
  pub struct NormalizedRelationship { .. }
  pub struct NormalizedEndpoint { .. }
  pub struct NormalizedSymbolRef { .. }
  pub struct NormalizedClusterRef { .. }

  pub struct Provenance {
      pub stream_path: String,
      pub record_id: Option<u32>,
      pub field_x: Option<u32>,
      pub cluster_index: Option<u32>,
      pub original_drawing_id: Option<String>,
      pub original_model_id: Option<String>,
      pub guid: Option<String>,
      pub source_layer: SourceLayer,  // Raw | Decoded | Inferred
  }
  ```
- **下游重构**：`inspect` / `report` / `import_view` / `layout` 全部迁移到新图层；旧视角可暂留 backward-compat 挂载
- **风险**：`PidDocument` 可能要做 modular 拆分；需单独 session 设计评审 + 至少一份 public API 审查文档
- **SemVer**：major（0.x → 1.0.0 候选点；或 minor 若保留旧 API 兼容）
- **Plan doc**：`docs/plans/2026-04-XX-phase-12a-normalization-design.md`（先发设计稿征求反馈）

#### Phase 12b — consumed/leftover 字节验证框架（roadmap Phase 4）

- **核心**：每个 parser 报告 `Vec<ByteRange>` 已消费 + 未消费
- **建模**：
  ```rust
  pub struct ParserTrace {
      pub stream_path: String,
      pub total_bytes: u64,
      pub consumed_ranges: Vec<ByteRange>,
      pub leftover_ranges: Vec<ByteRange>,
      pub confidence: Confidence,
  }
  ```
- **断言集**：
  - record count 与遍历数一致
  - 交叉引用无悬空
  - `stream_size == consumed + leftover`（互斥 + 穷尽）
  - leftover 比例回归守：CI 阈值，>= 前一版本严格更差则 fail
- **CLI**：`pid_inspect --byte-audit` 打印 stream-level coverage heat map（绿 = consumed / 黄 = partial / 红 = leftover）
- **Plan doc**：`docs/plans/2026-04-XX-phase-12b-byte-audit-framework.md`

### 阶段 E — 验收（roadmap Phase 5）

- 所有顶层流 FullyDecoded
- `unknown_streams` 仅剩样本特异内容
- 2-3 个代表性样本 leftover 字节比例 < 5%
- `inspect` 输出以结构化为主，不再依赖 raw/probe 描述
- `object_graph` / `cross_reference` / `layout` 全部基于统一规范化图层

## 4. 起步节奏建议

| 何时 | 做什么 | 工时 |
|---|---|---|
| 本 session | Phase 10h ship v0.7.1 | 30min |
| 下个 session | Phase 10i（无破坏，writer 边界打磨）| 3-5hr |
| 再下个 | Phase 11a（PSMclustertable，能快速把 coverage 一块变绿的 parser 工作）| 6-8hr |
| 之后 | 11b → 11c → 12a → 12b 顺序 | — |
| 按需 | 每 5-7 个新功能 Phase 之后插入卫生 pass（Phase 9d 节奏）| — |

## 5. 方法论沿用

从 Phase 9k–10g 的 12 轮收官纪念（`phase10-coverage-series-summary.md`）提炼：

1. **每个 Phase 先写 plan** — `docs/plans/YYYY-MM-DD-*.md`
2. **coverage 优先于新 parser** — 让现状可视化驱动优先级，而不是盲目深挖
3. **SemVer 以"错误路径变化"判定** — 原 `Err` 现 `Ok` 一定 minor bump
4. **raw + decoded + audit/probe 三层** — roadmap 明确要求；不确定语义放 audit 层不强行命名
5. **交叉验证锚点** — 每个新 decoder 必须找一个已稳定的锚点（如 DV2 ↔ DV3、PSMsegment ↔ Sheet endpoint）做 drift guard
6. **5-7 个 feature 后扫地一次** — Phase 9d / 9i / 9k / 10h 的节奏

## 6. 风险登记

| 风险 | 缓解 |
|---|---|
| 逆向范围膨胀 | coverage 驱动 + 每 Phase 量化验收 |
| PidDocument 膨胀 | Phase 12a 前确定 modular 拆分；新模型按模块分组 |
| 单 fixture 过拟合 | Phase 11 起硬约束 ≥ 2 fixture；无 fixture 则限 decoded + confidence=medium |
| 大 Phase 失控（Phase 11c / 12a）| 单独 session 设计稿 + 公开 API 评审 + 可回退 feature flag |
| Writer / parser 语义漂移 | 每次升级用 DV2↔DV3 风格的 drift guard 测试 |

---

> 交付动作：Phase 10h ship 之后本文件成为 **v0.7.1 及之后的"导航图"**，后续每个 Phase 的 plan doc 可以 cross-reference 本文件的 Phase 编号与估时。
