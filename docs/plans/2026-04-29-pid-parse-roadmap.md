# pid-parse 12-Week Roadmap (v0.11.7 → v1.0.0 Candidate)

> 起稿：2026-04-29
> 上游对应文档：
> - `docs/prd-pid-parse-current-state.md` — 产品现状 PRD
> - `docs/sppid/2026-04-21-sppid-full-parse-roadmap.md` — SPPID 完全解析战略路线图
> - `docs/sppid/v0.10.x-status.md` — v0.10.x 解析能力快照
> - `docs/plans/2026-04-21-next-steps-roadmap-v0.7.1-onward.md` — v0.7.1 战术导航图（已部分过时）
>
> 本文是 v0.11.7 之后下一阶段的**战术实施总图**，把 PRD 的 P0–P5 与 sppid roadmap 的 Phase 2–5 落到 12 周可执行 Phase 序列；每个 Phase 单独有 plan doc 承接。

---

## 1. TL;DR

| 优先级 | Phase | 内容 | 估时 | 周次 |
|---|---|---|---|---|
| P0 | 12c | 真实 fixture byte-audit baseline 落盘 + CI 接通 | 4h | W1 |
| P0 | 12d | coverage / byte-audit JSON schema 锁定 | 2h | W1 |
| P1 | 11a | `PSMclustertable` per-record 字段命名 | 8h | W2 |
| P1 | 11b | `PSMsegmenttable` 结构化 | 4h | W3 |
| P1 | 11d | `PSMspacemap` 子流（`tseg` magic）解码 | 4h | W3 |
| P2 | 11c-1 | `--probe-sheet-chunks` 增强 | 4h | W4 |
| P2 | 11c-2 | Sheet 高频 record type 字段命名（典型 typed model） | 12h | W4–5 |
| P2 | 11c-3 | Sheet stream coverage 升 PartiallyDecoded | 4h | W5 |
| P3 | A40 | DWG MDF fixture 落位 + DWG-gated 测试转 hard gate | 4h | W6 |
| P3 | A41 | DWG canonical field enrichment（A24/A27b ≥80% 收敛） | 8h | W6 |
| P3 | A42 | A01 raw byte 3 类 synthetic slot 收敛或形式化豁免 | 4h | W6 |
| P4 | 12a-design | Normalized graph 设计稿 + 公开 API 评审 | 8h | W7 |
| P4 | 12a-1 | additive normalized graph 加入 `PidDocument` | 10h | W8 |
| P4 | 12a-2 | `inspect` / `import_view` / `layout` 迁移消费 | 14h | W9–10 |
| P4 | 12a-3 | 旧 API 兼容窗口 + 删除 | 4h | W11 |
| P5 | release | 验收 + v1.0.0 candidate release | 6h | W12 |
| 总计 | — | — | **~100h** | **12 周** |

> 任何 Phase 都遵循项目既有节奏：一份 plan doc → 5 道 pre-commit gate → raw/decoded/audit 三层 → ≥2 fixture 才升 coverage 等级。

---

## 2. 现状速览（截至 v0.11.7 unreleased）

### 2.1 三条能力线成熟度

| 线 | 现成熟度 | 主要剩余缺口 |
|---|---|---|
| `.pid` 只读解析 | 中高 | PSM record 深层 / Sheet 几何 / Normalized graph 未实施 |
| `.pid` round-trip writer | 高 | 时间戳依 cfb upstream；语义级图元编辑未做 |
| Backup/MDF → publish XML | A01 高 / DWG 中 | DWG fixture / branch-point loader enrichment / 3 类 synthetic slot |

### 2.2 顶层流 coverage（17 个流已注册 byte-audit trace）

```
FullyDecoded (7):
  SummaryInformation / DocumentSummaryInformation / PSMroots /
  DocVersion2 / DocVersion3 / AppObject / JTaggedTxtStgList

PartiallyDecoded (5):
  PSMclustertable / PSMsegmenttable / PSMcluster0 /
  StyleCluster / Dynamic Attributes Metadata / Unclustered DA

IdentifiedOnly (3+):
  Sheet* / TaggedTxtData / JSite* / PSMspacemap
```

### 2.3 已就位的工程化能力

- `byte_audit` framework（`src/byte_audit/`）+ `--byte-audit-baseline` CLI 比较器
- `ParseProfile::Light` light parse（v0.11.7）
- 13-lint quality gate（10 clippy pedantic deny + 3 rustdoc deny + missing-docs ratchet）
- `PidPackage::from_bytes` 纯内存解析
- Sheet endpoint 结构化诊断字段（不再静默吞 error）
- writer pipeline 复用 helper（validator / writer 同一应用顺序）
- `.github/scripts/check-byte-audit-baselines.sh` 已就位
- 真实 `.pid` fixture 已就位（`test-file/DWG-0201GP06-01.pid` / `DWG-0202GP06-01.pid` / `工艺管道及仪表流程-1.pid`）

### 2.4 待解决的关键阻塞

| # | 阻塞 | 影响 |
|---|---|---|
| 1 | `docs/baselines/*.byte-audit.json` 未提交 | byte-audit 只能 soft-skip，无法守 coverage 回归 |
| 2 | `PSMclustertable`/`PSMsegmenttable` per-record 字段未命名 | partial → fully decoded 需 ≥2 fixture |
| 3 | Sheet 几何 / 文本 / 符号引用未深层解码 | `IdentifiedOnly`，layout/import 仍混合 probe |
| 4 | Normalized graph 未建 | object/relationship/endpoint/symbol/cluster 仍分散 |
| 5 | DWG MDF fixture / loader enrichment | `tests/publish_dwg_mirror.rs` soft-skip 关键 gate |
| 6 | A01 raw byte 3 类 synthetic slot | `PIDPipingConnector UID` / `Rel IObject UID` / `PIDRepresentation GraphicOID` |

---

## 3. 阶段 A — 固化 baseline（P0，W1，~6h，2 个 PR）

### Phase 12c — 真实 fixture byte-audit baseline 落盘

- 详细执行步骤见 `docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md`
- 目标：把 `--byte-audit-baseline` 从工具变 CI 回归门
- 验收：
  - `overall_coverage_ratio` ↓ → CI fail
  - traced stream `consumed_bytes` ↓ → CI fail
  - traced 翻回 unregistered → CI fail
  - 公开 CI 无 fixture 时 soft-skip 退出 0

### Phase 12d — coverage / byte-audit JSON schema 锁定

- 把 `ByteAuditReport` / `CoverageReport` 加到 `--schema` JSON schema 输出
- 加 schema snapshot 测试（防止字段重命名 / 删除导致下游崩）
- 估时：2h
- Plan doc：`docs/plans/2026-04-XX-phase-12d-audit-schema-lock.md`（待写）

---

## 4. 阶段 B — PSM record 结构化加深（P1，W2–3，~16h，3 个 PR）

> 严格遵循"≥2 fixture 才升 coverage"硬约束；缺 fixture 只允许 `decoded + confidence=medium`。

### Phase 11a — `PSMclustertable` per-record 字段命名

- 目标字段（基于 SmartPlant 行为推测，需逆向确认）：
  ```rust
  pub struct PsmClusterRecordDecoded {
      pub cluster_id: u32,
      pub index: u16,
      pub flags: u16,
      pub type_tag: u16,
      pub declared_segment_count: u32,
      pub raw_trailer: Vec<u8>,
  }
  ```
- 交叉验证锚点：
  - `record total == doc.psm_segment_table.flags.len()`
  - record `cluster_name == doc.clusters[i].path`
- 验收：
  - parser 输出 raw + decoded + audit 三层
  - drift guard 测试（DV2↔DV3 风格交叉断言）
  - byte-audit `consumed_bytes` 显著上升
  - 单 fixture 时升到 `Decoded(confidence=medium)`，2+ fixture 后升 `FullyDecoded`
- 估时：6–8h
- Plan doc：`docs/plans/2026-04-XX-phase-11a-psmclustertable-records.md`

### Phase 11b — `PSMsegmenttable` 结构化

- 目标：
  - segment record 字段（推测 `segment_id` / `source_cluster` / `target_record_id` / `kind`）
  - 建立 segment ↔ `layout.segments` / relationship / Sheet endpoint 的 provenance 链
  - `kind` 区分 Geometric / Connection / Reference
- 锚点：Sheet `endpoint_records.rel_field_x` ↔ PSMsegment `segment_id`
- 估时：4–6h
- Plan doc：`docs/plans/2026-04-XX-phase-11b-psmsegmenttable.md`

### Phase 11d — `PSMspacemap` 子流（`tseg` magic）解码

- 目标：`IdentifiedOnly` → `PartiallyDecoded`
- 锚点：tseg 子流与 PSMsegmenttable cross-reference
- 估时：4h
- Plan doc：`docs/plans/2026-04-XX-phase-11d-psmspacemap.md`

---

## 5. 阶段 C — Sheet 几何深层解码（P2，W4–5，~20h，单独 session）

> 方法论：先增强 probe，再按 record type 频次 ranked 解码，避免一次深挖失控。
> 硬约束：≥3 个 Sheet fixture（含 A01 + DWG 各一）。

### Phase 11c-1 — `--probe-sheet-chunks` 增强

- record type 频次 / chunk boundary / coordinate candidate / text run offset / symbol ref candidate
- 估时：4h

### Phase 11c-2 — 高频 record type 字段命名

- 按频次 > 20% 的 record type 优先逆向
- 顺序：先解坐标（双/四字节 small int pattern）→ UTF-16LE text run → cluster/segment 引用
- 输出 typed sheet model：
  ```rust
  pub enum SheetGeometry {
      Primitive { kind, coords, .. },
      TextAnnotation { offset, content, font_hint },
      Connector { source_endpoint, target_endpoint },
      SymbolPlacement { symbol_ref, position, rotation },
  }
  ```
- 每个字段携带 source byte ranges
- 估时：12h

### Phase 11c-3 — Sheet stream coverage 升级

- `IdentifiedOnly` → `PartiallyDecoded`
- byte-audit Sheet leftover ratio 显著下降
- 估时：4h

---

## 6. 阶段 D — DWG publish 闭环（P3，W6，~16h，3 个 PR）

### Phase A40 — DWG MDF fixture 落位

- 落 `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf` 或脱敏 mirror
- `tests/publish_dwg_mirror.rs` 与 `publish_meta_parity.rs` 的 DWG-gated 测试转 hard gate
- 估时：4h

### Phase A41 — DWG canonical field enrichment

- BranchPoint / PipingBranchPoint loader item type mapping + subtable chain
- DWG-only canonical fields：EqType / ProcessEqCompType / ConnectionFlowDirection / insulation / slope
- A24 单接口白名单 + A27b 15 条 style 差异：≥80% 收敛
- 验收：DWG branch-point count / interface / attr parity 全绿
- 估时：8h

### Phase A42 — A01 raw synthetic slot 收敛或形式化豁免

- 三类 slot：`PIDPipingConnector UID` / `Rel IObject UID` / `PIDRepresentation GraphicOID`
- 若可逆向：补 writer arm + parity gate
- 若 SmartPlant 无规律：写入 PRD 永久豁免 + 对外说明
- 估时：4h

---

## 7. 阶段 E — Normalized 语义图层（P4，W7–11，~36h，单独设计 session）

> 大 Phase，先发设计稿评审，再 incremental 切换；旧 API 保留兼容窗口。
> 候选 SemVer：minor（保留旧视角）或 major（v1.0.0 触发点）。

### Phase 12a-design — 设计稿 + 公开 API 评审

- 模型：
  ```rust
  pub struct NormalizedObject { id, kind, provenance, .. }
  pub struct NormalizedRelationship { source, target, kind, provenance, .. }
  pub struct NormalizedEndpoint { object_ref, port_index, provenance, .. }
  pub struct NormalizedSymbolRef { symbol_path, jsite_index, provenance, .. }
  pub struct NormalizedClusterRef { cluster_index, kind, provenance, .. }
  pub struct NormalizedGeometry { primitive, coords, provenance, .. }

  pub struct Provenance {
      pub stream_path: String,
      pub byte_range: Option<ByteRange>,
      pub record_id: Option<u32>,
      pub field_x: Option<u32>,
      pub cluster_index: Option<u32>,
      pub original_drawing_id: Option<String>,
      pub original_model_id: Option<String>,
      pub guid: Option<String>,
      pub source_layer: SourceLayer, // Raw | Decoded | Probed | Inferred
  }
  ```
- 公开 API surface 评审：哪些字段是 stable / experimental / hidden
- 估时：8h

### Phase 12a-1 — additive normalized graph 加入 `PidDocument`

- 不动旧字段，纯新增
- 估时：10h

### Phase 12a-2 — `inspect` / `import_view` / `layout` 迁移消费

- `crossref.rs` 与 `layout.rs` 内部改为基于 normalized graph 派生
- 旧 API 标 `#[deprecated]`
- JSON schema + fixture snapshot 锁定
- 估时：14h

### Phase 12a-3 — 旧 API 兼容窗口 + 删除

- 一个 minor cycle 后移除，发 v1.0.0
- 估时：4h

---

## 8. 阶段 F — 验收与 v1.0 候选（W12）

| 验收项 | 通过条件 |
|---|---|
| coverage | 所有顶层 stream 有明确 parser；`Unknown` 仅样本特异 |
| byte-audit | 2–3 个代表样本 leftover ratio < 5% |
| inspect | 输出以结构化为主，不再依赖 raw/probe 描述 |
| `object_graph` / `cross_reference` / `layout` | 全部基于 normalized graph |
| publish A01 | semantic / Meta / Rel DefUID/UID gates 持续 clean |
| publish DWG | branch-point / canonical / interface / attr parity 全绿 |
| writer | metadata-only round-trip diff 为 0 或可解释 |
| CI | byte-audit baseline runner hard-fail（私有 fixture 路径） |
| SemVer | 旧 API 兼容窗口结束，v1.0.0 candidate |

---

## 9. 风险登记

| 风险 | 影响 | 缓解 |
|---|---|---|
| 单 fixture 过拟合 | parser 在新 plant 上误解字段 | 11a 起硬约束 ≥2 fixture，否则限 `confidence=medium` |
| Sheet 几何逆向失控 | 11c 拖延数月 | 单独 session + probe-first + 频次 ranked + feature flag |
| Normalized graph API churn | 下游崩 | 12a 设计稿先发评审 + additive + 兼容窗口 |
| DWG MDF fixture 缺 | DWG gate 一直 soft-skip | 与产品 owner 协商提供脱敏 mirror |
| GPL vendored MDF reader | 对外分发受限 | README license 节已就位；分发时按 GPL-3.0 提供 source |
| byte-audit baseline 私有 | 公开 CI 无法守 | 公开 soft-skip + 私有 CI 跑 baseline artifact |
| `PidDocument` 持续膨胀 | 字段失控 | 12a 设计阶段先做 modular 拆分评审 |
| 中文文件名 fixture 跨平台 | CI / shell escaping 易出错 | baseline 命名 ASCII slug 化（见 12c plan） |

---

## 10. 起步节奏建议

按时间倒推首个可执行任务：

1. **本 session**：`docs/plans/2026-04-29-phase-12c-byte-audit-baseline.md` 已写好，可立即执行 Task 1–5
2. **下个 session**：执行 Phase 12d schema lock
3. **W2 起**：每周开一个 Phase 11x，遵循"plan doc → red test → minimum patch → 5 道 gate → squash merge"
4. **每 5–7 个 Phase 之后**：插入卫生 pass（Phase 9d / 9i / 10h 风格）
5. **W7 起**：Normalized graph 设计稿先发，再写代码

---

## 11. 方法论沿用

从 Phase 9k–10g 与 12b 系列收官提炼：

1. **每个 Phase 先写 plan** — `docs/plans/YYYY-MM-DD-*.md`
2. **coverage 优先于新 parser** — 让现状可视化驱动优先级
3. **SemVer 以"错误路径变化"判定** — 原 `Err` 现 `Ok` 一定 minor bump
4. **raw + decoded + audit/probe 三层** — roadmap 明确要求
5. **交叉验证锚点** — 每个新 decoder 必须找一个已稳定的锚点做 drift guard
6. **5–7 个 feature 后扫地一次** — Phase 9d / 9i / 9k / 10h 节奏
7. **不主动扩展 scope** — 完成即止，按 plan doc 走
