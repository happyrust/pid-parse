# 任务计划：PID 解析能力开发方案

## 目标
基于当前 `pid-parse` 能力现状，制定下一阶段中文开发方案：优先补齐高价值解析缺口，保持 Probe/Decode 分层、byte-audit 可验证、writer passthrough 安全边界。

## 当前阶段
Phase 30 - radsrvitem.dll IDA 证据刷新（partial）。Phase 29 A..M 的
byte-audit / extraction / registry 清尾已 complete；本轮在可达
`radsrvitem.dll` IDB 中确认多个 `JSite<N>` storage name builder /
open path（`sub_56448A10`、`sub_56448A70`、`sub_5646FF60`），增强
`/JSitesList.entries` ↔ JSite storage id 的证据链；但当前 DLL 仍无
`"OLEM"` / `JSitesList` 字符串，writer 侧和 stale tail 语义未解，
因此 DTO 继续保持 `entries` / `trailing_slots` 保守命名。剩余
multi-fixture 项仍为 IDA（PSMspacemap 页、0x0089/0x0010 语义、
StyleCluster prefix）/ demand（\x01Ole）gated。

## 历史阶段 → goals/ 包托管说明
2026-05-13 起 Phase 13+ 的细节迁移到 `goals/phaseNN-...` 目录（brief / plan /
verification / blockers / goal-prompt / progress.jsonl 五件套+1 模板），
`task_plan.md` 只保留入口与 status。详细路线图见 `docs/plans/`。

## 阶段

### Phase 1：现状基线确认
- [x] 阅读 README、当前状态 PRD、v0.10.x 状态表、架构指南
- [x] 确认公共入口、解析管线、CLI、测试与能力边界
- [x] 记录支持范围与主要缺口
- **Status:** complete

### Phase 2：PSM 结构化补齐
- [x] 收敛 `PSMclustertable` per-record 字段语义
  - [x] `decoded_records.unknown_prefix_bytes` 改为真实记录候选字段之外的前缀字节
- [x] 收敛 `PSMsegmenttable` 记录结构与 segment/cluster 关联
  - [x] 为 `PsmSegmentEntry` 增加保守的 `candidate_owner_cluster_index/name`
- [x] 给 byte-audit 增加 decoded/probed/leftover trace
  - [x] aggregate 测试锁定 `/PSMclustertable` decoded/probed/leftover 分桶
  - [x] 评估 candidate 字段 confidence：保持 prefix `Probed`，不升级为 `Decoded`
- [x] 更新 coverage、报告、JSON schema 与回归测试
  - [x] 文本 report 输出 segment `owner_candidate=index:name`
  - [x] coverage note 说明 segment flags + owner candidate mapping
  - [x] schema 测试锁定 `PsmSegmentEntry` candidate owner 字段
  - [x] 真实 fixture soft-skip 测试校验 candidate owner 与 probe hint 一致
- **Status:** complete

### Phase 3：Sheet 几何与端点深化
- [x] 将现有 Sheet text run / endpoint record / coordinate hint 归一化
  - [x] 将 `sheet_probe` text runs 与 coordinate hints 映射到 `SheetStream.geometry`
  - [x] 将 `SheetEndpointRecord` 同步到 `SheetStream.geometry.endpoints`
- [x] 定义稳定的 `SheetGeometry` / `SheetText` / `SheetEndpoint` DTO
  - [x] 新增 `SheetGeometry` / `SheetText` / `SheetEndpoint` / `SheetCoordinateHintDto` schema DTO
- [x] 保持未命名字节走 probe，不伪装为 decoded
  - [x] `SheetGeometry` 仅承接 probe evidence，不声明完整 CAD geometry decoded
- [x] 用真实 fixture 或 synthetic fixture 建立回归样例
  - [x] synthetic 回归锁定 `SheetGeometry` 同时保留 text、coordinate hint、endpoint
- **Status:** complete

### Phase 4：规范化语义图层
- [ ] 将 object、relationship、endpoint、symbol、cluster、sheet provenance 统一为 canonical graph
  - [x] `PidImportView.relationships` 暴露 Sheet endpoint provenance
- [ ] 明确 `PidDocument` 原始事实层与 `ImportView` 消费视图边界
- [ ] 为 H7CAD / 下游 CAD 导入提供稳定 JSON contract
- **Status:** in_progress

### Phase 5：Publish XML 与 DWG 闭环
- [ ] 继续保持 A01 publish fidelity gates
- [ ] 闭环 DWG fixture、loader enrichment 与 branch-point parity
- [ ] 明确 GPL-3.0 vendored MDF reader 的分发合规说明
- **Status:** pending

### Phase 6：方案交付
- [x] 生成中文开发方案文档
- [x] 生成技术路线图 SVG
- [x] 更新 progress 与 findings
- **Status:** complete

### Phase 7：H7CAD PID 真实几何显示与证据门禁
- [x] 将 Sheet coordinate hints 作为 `Inferred Point` 接入 H7CAD 显示
- [x] 建立 `SheetObjectGeometryHint` 空基线，防止未证明 object-coordinate mapping 被误用
- [x] 建立 field-x window / chunk-shape / marker / coordinate-quality 调查链路
- [x] 建立 `GraphicIdentityNearby` identity index、scanner、scoring report
- [x] 证明 `/Sheet6` 当前仍不可 promotion：`object_geometry_hints=0`、不渲染 line
- [x] 将当前工作拆为 PR1-PR5 边界
- [x] 建立 Text placement Phase A/B：text-window candidates、text-quality filter、scoring report
- [x] 证明 `/Sheet6` 当前仍无可 promotion 的 `Text + Inferred`
- [ ] 执行 hunk staging 或临时分支拆分（需用户明确授权）
- [ ] 获取更多真实 PID fixture，或改善 text extraction 后再继续 Text placement
- **Status:** in_progress

### Phase 8：完整解析推进路线
- [x] 新增多 fixture geometry evidence inventory，确认当前 4 个 fixture 仍无 promotion 证据
- [x] 形成下一阶段中文开发方案：fixture 扩容、Sheet record grammar、object-coordinate promotion gate、Text/Symbol 渲染升级
- [x] 增加 per-fixture / per-sheet 明细输出
- [x] 建立 top identity/text candidate record dump helper
- [x] 建立第一版 Sheet record shape classifier
- [ ] 在 source-proven gate 达标后，再填充 `SheetObjectGeometryHint` 并升级 H7CAD Line/Text/Symbol layer
- **Status:** in_progress

### Phase 9：2026-05-06 下一阶段开发计划
- [x] 复核当前解析能力：`.pid` 容器、metadata、object graph、crossref、layout、writer、publish XML
- [x] 复核当前几何基线：5 fixture inventory 已有 5 个 object-coordinate promotion，Text/Symbol 仍无 promotion
- [x] 新增中文开发计划：`docs/plans/2026-05-06-pid-parse-development-plan-cn.md`
- [x] 将开发计划拆成 6 个执行阶段：
  - [x] Phase 9A fixture baseline hardening
  - [x] Phase 9B Sheet record grammar reverse engineering
  - [x] Phase 9C object-coordinate promotion gate
  - [x] Phase 9D Text/Symbol source-proven rendering
  - [x] Phase 9E canonical graph integration
  - [x] Phase 9F publish XML gate closure
- [ ] 执行 Phase 9A：扩展 fixture registry 与 inventory baseline
- [x] Phase 9A 首个切片：新增显式 geometry fixture registry，并让 inventory 复用 registry
- [x] Phase 9A 第二个切片：新增 fixture availability summary，显式记录 registered/available/missing/target
- [x] Phase 9A 第三个切片：将 availability summary 格式化为 report line 并接入 inventory 输出
- [x] Phase 9C 首个切片：为 promoted `SheetObjectGeometryHint` 增加 provenance-focused regression，并让 note 输出 score/identity/stable_shape
- [x] Phase 9C 第二个切片：为 normalized geometry projection 增加 source note 回归，确认 promoted hint note 进入 `PidGraphicProvenance`
- **Status:** in_progress

### Phase 10：2026-05-09 f64 Record Shape 坐标源与 Endpoint Line 闭环
- [x] Slice 1：f64 pair 坐标候选 DTO 与 extraction helper（已存在，扩展 shape 携带 x/y）
- [x] Slice 2：f64 pair 候选接入 promotion gate fallback（新增 f64 pair gate，promotable 5→20）
- [x] Slice 3：endpoint pair line 产生验证（inferred_points 69→80，但 inferred_lines 仍为 0；endpoint pair 两端不对称）
- [ ] Slice 4：坐标尺度验证与多 fixture 横向确认
- [ ] Slice 5：H7CAD 端到端 line 消费
- [ ] Slice 6：全量回归加固与文档更新
- **Status:** complete
- **Plan:** `docs/plans/2026-05-09-phase-10-f64-coordinate-source-endpoint-line-plan-cn.md`

### Phase 11：2026-05-09 坐标系验证、Text 渲染与几何质量加固
- [ ] Slice 1：f64 坐标值域分析与页面映射研究
- [ ] Slice 2：剩余 endpoint pair 覆盖扩展（34/59 → 40+/59）
- [ ] Slice 3：Text placement gate 重新评估
- [ ] Slice 4：H7CAD 坐标映射集成
- [ ] Slice 5：质量回归与文档
- **Status:** complete
- **Plan:** `docs/plans/2026-05-09-phase-11-coordinate-validation-text-rendering-plan-cn.md`

### Phase 12：2026-05-09 页面坐标变换与 Text 字段提取
- [ ] Slice 1：页面尺寸提取（Template → page_size 映射）
- [ ] Slice 2：NormalizedPidGeometry 坐标变换（f64 × 页面尺寸）
- [ ] Slice 3：H7CAD 坐标空间对齐
- [ ] Slice 4：Sheet Record Text 字段识别（investigation）
- [ ] Slice 5：质量回归与文档
- **Status:** pending（保留待后续重启；Phase 13+ 优先把 Sheet record 反向做透）
- **Plan:** `docs/plans/2026-05-09-phase-12-page-transform-text-extraction-plan-cn.md`

### Phase 13：2026-05-14 Plan B controlled-diff protocol
- [x] 建立 Plan B 控制 diff 协议作为 SPPID Sheet 几何反向工程的安全网
- **Status:** complete
- **Goal package:** `goals/phase14-plan-b-controlled-diff-protocol/`

### Phase 14：2026-05-14 SmartPlant Sheet geometry decoder suite（8 PSM 类型）
- [x] Slice D-E：`GLine2d` (0x3FE6) typed decoder + ratchet
- [x] Slice F-I：`GArc2d` (0x0030) typed decoder + ratchet（Phase 16 后被 retire）
- [x] Slice J：`igLine2d` (0x0018) typed decoder + ratchet（284 records）
- [x] Slice K：`igLineString2d` (0x0084) typed decoder + ratchet（119 records）
- [x] Slice L：`igPoint2d` (0x005E) typed decoder + ratchet（146 records）
- [x] Slice M：`igTextBox` (0x004D) typed decoder + ratchet（142 records）
- [x] Slice N：`igSymbol2d` (0x00CE) typed decoder + ratchet（27 records）
- [x] Slice O：decoder suite final summary
- [x] Slice P：`pid_inspect --geometry-summary` CLI flag
- **Status:** complete
- **Goal package:** `goals/phase14-sppid-sheet-geometry/`
- **Final summary:** `docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md`

### Phase 15：2026-05-14 PSM 0x00FA `GraphicGroup` audit-only decoder
- [x] 跨 4 fixture 352 audit records（header + raw_variable_tail，
      不命名 child OID list）
- [x] 不引入 `PidGraphicKind` variant
- **Status:** complete
- **Goal package:** `goals/phase15-graphic-group-records/`
- **Final summary:** `docs/plans/2026-05-14-phase15-graphic-group-final-summary.md`

### Phase 16：2026-05-16 PSM 0x0030 = `JStyleOverride` 跨 5 IDA 反向
- [x] 跨 5 IDA instance（radsrvitem.dll → J2DSrv.dll → JUTIL.dll →
      style.dll）反向，钉到 RAD CLSID `{47FCC338-...}`
- [x] V3 disk schema 13 个 IOContext::DoIO（64 字节 payload）
- [x] 跨 fixture 98 records（找回 Phase 14 GArc2d 错误丢失的 50 条）
- [x] 新 `decode_jstyle_overrides` + `PidGraphicKind::Annotation`
- [x] 严格 additive，Phase 14 既有 surface 暂留
- **Status:** complete
- **Goal package:** `goals/phase16-j2dsrv-record-decode/`
- **Final summary:** `docs/plans/2026-05-16-phase16-jstyleoverride-final-summary.md`
- **Authoritative fields:** `docs/analysis/2026-05-16-jstyleoverride-v3-fields.md`

### Phase 17：2026-05-17 移除 legacy `PrimitiveArc` 兼容层
- [x] 删除 `decode_primitive_arcs` parser API、`SheetPrimitiveArcDecoded`
      DTO、`SheetGeometry::decoded_primitive_arcs` 字段
- [x] `geometry.rs` 不再为 0x0030 emit `PidGraphicKind::Arc`
- [x] 新 `SheetDecodedGeometryKind::Annotation` + `jstyle_override`
      schema 入口
- [x] `pid_inspect --geometry-summary` 切换为 "Annotations" 计数
- **Status:** complete
- **Goal package:** `goals/phase17-primitive-arc-deprecation/`

### Phase 18：2026-05-17 PSM 0x0010 sub-record family audit-only decoder
- [x] Slice A-H：parser DTO + decoder + model DTO + cluster pipeline +
      cross-fixture ratchet (582 records) + panic-safety + CHANGELOG +
      5 道 gate
- [x] 不命名 sub-kind 字段；不引入 `PidGraphicKind` variant
- [x] commit `81daa20` + push
- **Status:** complete
- **Goal package:** `goals/phase18-psm-0x0010-sub-record/`

### Phase 19：2026-05-17 PSM 0x0010 `leading_word` audit field
- [x] RAD sibling probe 证伪 "CLSID 段 47FCC330..47FCC33E ↔ PSM 0x29..0x35"
      假设（仅 0x0030 有 hits）；evidence `docs/analysis/2026-05-17-phase19-rad-sibling-probe-null-result.md`
- [x] `leading_word: Option<u16>` audit 字段（= `payload[0..2]` LE u16）
- [x] cross-fixture ratchet：0x0002=164 / 0x0003=21 / 0x0001=18 /
      None=0 / total=582
- [x] Phase 18 ratchet 582 不退化
- [x] 字段名描述字节位置不描述语义；不命名 `sub_kind`
- [x] commit `6beb6f1` + push
- **Status:** complete
- **Goal package:** `goals/phase19-psm-0x0010-leading-word-audit/`

### Phase 20：2026-05-17 PSM 0x0010 IDA-confirmed RAD class identity（partial closeout）
- [x] Slice A：`radsrvitem.dll` dispatch table 侦察，定位 `PSMSerializeIn`
      / `PSMSerializeOut` 与 PersistTypeTable 路径
- [x] Slice B：factory / CLSID lookup 追踪到 partial AC：PSM type `0x0010`
      映射 GUID `1D1928C0-0000-0000-C000-000000000046`，parent alias
      `0x0115` 复用同一 GUID
- [ ] Slice C：目标 class Read/IO 函数 + IO sequence（deferred；未恢复）
- [ ] Slice D：sub-kind discriminator 偏移 + 枚举（deferred；禁止命名 `sub_kind`）
- [ ] Slice E：cross-fixture validation（deferred；`leading_word` 仍 audit-only）
- [x] Slice F：`docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
      8 节 authoritative analysis（mirror Phase 16）
- [x] metadata / RTTI / registry / external GUID follow-up：均未恢复 human type name
- [x] readonly Read/DoIO tracing follow-up：恢复 `JStyleBase` control path，但未绑定
      `1D1928C0...`
- [ ] Slice G：full `goal_complete` 未声明；本阶段按 partial AC closeout 收口
- **Status:** partial complete；GUID / type-table identity confirmed，class name /
  Read-DoIO / sub-kind discriminator deferred
- **Goal package:** `goals/phase20-psm-0x0010-ida-class-identity/`
- **详细路线图:** `docs/plans/2026-05-17-phase20-ida-rad-class-roadmap-cn.md`
- **Analysis doc:** `docs/analysis/2026-05-17-phase20-psm-0x0010-rad-class.md`
- **Docs commits:** `b50ca19` / `68d505f` / `d586834`

### Phase 21：2026-05-18 D06 解析覆盖收敛与关系 / Sheet 审计闭环
- [x] Slice A：D06 baseline ratchet，锁定结构计数与 normalized geometry summary
- [x] Slice B：attribute-fallback relationship extraction，D06 现在保留 10 条
      unresolved relationships
- [x] Slice C：D06 `/Sheet6` decoded / audit-only / probe-only evidence inventory
- [x] Slice D：跳过新增 CLI flag；现有 `--geometry-summary` / `--json` 足够
- [x] Slice E：build / test / clippy / fmt / missing-docs gates 通过并更新文档
- **Status:** complete
- **Plan:** `docs/plans/2026-05-18-phase21-d06-parse-coverage-plan-cn.md`
- **Analysis:** `docs/analysis/2026-05-18-d06-relationship-gap.md`,
  `docs/analysis/2026-05-18-d06-sheet6-audit-inventory.md`
- **Commit:** `5255f25`

### Phase 22：2026-05-18 D06 text-placement regression fixture
- [x] 新增 `d06_text_placement_regression_keeps_text_probes_unpromoted`
- [x] 锁定 D06 `/Sheet6` 8 个 raw text probes + 4 个 decoded `igTextBox`
- [x] 确认 text probes 不提升为 inferred `Text` geometry
- [x] focused tests / `parse_real_files` / fmt / lint 通过
- [x] Phase 22 micro：D06 进入 6 个 Phase 14 cross-fixture decoder
      fixture 数组（Slice E/J/K/L/M/N），按 D06 baseline 计数精准
      ratchet 阈值：K +6 / L +10 / M +4 / N +2；E/J 阈值不变
- **Status:** complete
- **Commits:** `9ebdd89` (text-placement regression) /
  `bf4f972` (Phase 22 micro cross-fixture coverage)

### Phase 23：2026-05-18 Coordinate/Page Context 收敛与 transform guardrail
- [x] 分析 Phase 20/21/22 后的真实阻塞：typed `0x0010` DTO 仍缺 Read/DoIO
      与 sub-kind discriminator 证据，D06 text probes 仍 no-promotion
- [x] 选择下一阶段方向：避开 0x0010 blocker，先收敛 page dimensions、
      coordinate metadata investigation 与 page transform guardrail
- [x] 新增中文开发方案：
      `docs/plans/2026-05-18-phase23-coordinate-page-context-plan-cn.md`
- [x] Slice A：coordinate context baseline ratchet
- [x] Slice B：cross-fixture coordinate metadata report 增强
- [x] Slice C：transform promotion gate 合同
- [x] Slice D：文档与下游契约同步
- [x] Slice E：预提交门禁
- **Status:** complete
- **Plan:** `docs/plans/2026-05-18-phase23-coordinate-page-context-plan-cn.md`

### Phase 24：2026-05-18 CoordinatePageMetadata decoder 候选筛选
- [x] 制定 planning-skill 风格中文执行方案：
      `docs/plans/2026-05-18-phase24-coordinate-page-metadata-decoder-plan-cn.md`
- [x] Task 24-01：生成 candidate marker group evidence table
      （`examples/probe_phase24_top_evidence.rs` +
      `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md`）；
      29 top_evidence rows / 25 distinct markers / 0 cross-fixture
      stable marker / 0 page-dim matches
- [x] Task 24-02：stop-and-challenge review；4 条触发 3 条
      （无跨 fixture support、`page_dimension_scalar_matches=0`、
      字段解释需要猜单位/方向/origin）；用户选择 **路径 A negative
      evidence 收口**
- [ ] Task 24-03：跳过 — 不实现 typed candidate DTO；保留
      Phase 23 `probe_only_no_coordinate_page_metadata_promotion`
      guardrail 不变
- [x] Task 24-04：CHANGELOG / findings / progress / task_plan 同步
      Phase 22 micro + Phase 24 Task 24-01 + Task 24-02 review 决策
- **Status:** complete (negative evidence)；Task 24-03 skipped per
  Stop-And-Challenge
- **Plan:** `docs/plans/2026-05-18-phase24-coordinate-page-metadata-decoder-plan-cn.md`
- **Analysis:** `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md`
- **Commits:** `8f3739c` (Task 24-01 probe + analysis) +
  follow-up commit (Task 24-04 docs sync)
- **Re-open trigger:** 新增 PID fixture 在同一 marker 上出现 kind
  一致的 top_evidence 且至少 1 行 `page_dimension_scalar_matches > 0`

### Phase 26：2026-06-03 PID 文件全格式分析说明
- [x] 结合 grill-me 决策树制定格式说明开发计划：
      `docs/plans/2026-06-03-phase26-pid-file-format-analysis-plan-cn.md`
- [x] Phase 26-A：事实源审计
  - [x] 复核 known stream/storage registry、coverage 状态、byte-audit registered parser
  - [x] 标记旧文档中可能偏乐观或过时的格式描述
- [x] Phase 26-B：新增主格式说明
  - [x] 新增 `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`
  - [x] 覆盖 CFB、top-level stream/storage、magic、字节布局、parser、模型字段、coverage、限制
  - [x] 单独区分 `Sheet*` decoded / audit-only / probe-only record families
- [x] Phase 26-C：验证与快照
  - [x] 检查代表性 fixture 可用性；当前工作树未发现 `.pid` 样本
  - [ ] 对代表性 fixture 运行 `pid_inspect --coverage --json`（blocked: fixture 缺失）
  - [ ] 对代表性 fixture 运行 `pid_inspect --byte-audit --json`（blocked: fixture 缺失）
  - [x] 将阻塞原因写入主文档附录
- [x] Phase 26-D：文档交叉链接与收口
  - [x] 更新 README / `docs/format-notes.md` 入口
  - [x] 同步 `task_plan.md` / `progress.md` / `findings.md`
- **Status:** complete (snapshot blocked by missing fixtures)
- **Plan:** `docs/plans/2026-06-03-phase26-pid-file-format-analysis-plan-cn.md`
- **Analysis:** `docs/analysis/2026-06-03-pid-file-format-analysis-cn.md`

### Phase 27：2026-06-03 IDA 证据驱动的 PID 全数据类型提取
- [x] 制定 Phase 27 计划：
      `docs/plans/2026-06-03-phase27-ida-driven-pid-data-type-extraction-plan-cn.md`
- [x] 连接并选择 `radsrvitem.dll` IDA instance (`127.0.0.1:13338`)
- [x] survey `radsrvitem.dll`，确认 32-bit / 5374 functions / 1739 strings /
      exports `GetServerItemTransceiver` 与 `GetServerItemVersion`
- [x] Phase 27-A 初步恢复 `sub_56448F70` type-code mapper：
  - [x] 导出 27 个 switch case
  - [x] 补齐 `igDimension` / `igBalloon` / `igLeader` 三个 if/else return
  - [x] 确认 `0x00CE = igSymbol2d`
- [x] 新增数据类型矩阵初版：
      `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`
- [x] 建立 `igTextBox` 样板 reader 候选：`sub_564468B0`
- [ ] Phase 27-B：对 P0 五类 typed decoder 逐类追 `type name -> xref -> reader`
  - [x] `igTextBox` 样板：确认 `sub_56445F40 -> sub_564468B0` dispatch / reader path
  - [x] `igTextBox` 样板：完成首版 `IDA reader ↔ decode_igtextbox_at` 字段对照
  - [x] 追 `sub_564462F0` 默认路径，确认该路径只写 type name / `RELEATIONS`，不读取普通几何字段
  - [x] 搜索 P0/P1 type name xrefs，确认 `igPoint2d` / `igLineString2d` / `igSymbol2d` / `igCircle2d` 在当前 `radsrvitem.dll` 仅命中 mapper
  - [x] 追 `sub_56445F40` 中 `v10` record pointer 来源：确认它来自 `this+0x3c` / `ImpIJPersistManager` 的 vtable `+0xA4` lookup
  - [x] 定位 `ImpIJPersistManager::vtable+0xA4 = sub_56468DB0` 与 `SerialCluster::vtable+0x70 = sub_56493F50`，确认 runtime record pointer = loaded cluster base + descriptor offset
  - [x] 确认 `SerialCluster` 层 offset 公式：`stream_offset = runtime_record_ptr - serial_cluster_base = record_descriptor[0]`
  - [x] 确认 `ImpIPersistStorage::Load` 从 CFB `IStorage` 打开 `PSMclustertable` / `PSMroots` / `PSMspacemap` / `PSMcluster0`，当前链路绑定到 PSM 持久化 streams，而非直接 `Sheet*` raw stream
  - [x] 复查现有 `core.dll` IDA instance：`ASHEET` / `DSHEET` 为数据库属性初始化，`CMPTSZ` 为 sheet token 坐标调试/命令输出，未发现 PID `Sheet*` raw record reader
  - [ ] 继续追 PSM runtime record 到 `Sheet*` raw geometry 的投影关系，或打开/选择更多相关 IDA DLL（优先 `J2DSrv.dll` / `style.dll` / `sppid.dll` / `XCeedRAD.dll`）寻找普通几何 reader
- [ ] Phase 27-C：做 `IDA layout ↔ parser DTO` 字段级对照
- [ ] Phase 27-D：形成后续 parser 修复 / 补齐 backlog
- [ ] Phase 27-E：重启 `0x0010` / `GraphicGroup` / `PSMspacemap` 深水区分析
- **Status:** in_progress
- **Plan:** `docs/plans/2026-06-03-phase27-ida-driven-pid-data-type-extraction-plan-cn.md`
- **Matrix:** `docs/analysis/2026-06-03-phase27-pid-data-type-matrix-cn.md`

### Phase 28：2026-06-08 Spec Kit 风格 PID 文件全格式规格包
- [x] 创建 Spec Kit 风格规格包目录：
      `docs/specs/2026-06-08-pid-file-format-spec-kit/`
- [x] 新增 `spec.md`：目标、用户故事、证据等级、功能需求、guardrails、
      验收标准
- [x] 新增 `plan.md`：Phase 28-A..E 执行切片、IDA 续查入口、风险矩阵
- [x] 新增 `research.md`：parser 事实、Phase 27 IDA 事实、fixture 限制、
      `0x0010` / JStyle 阻塞
- [x] 新增 `data-model.md`：container / metadata / registry / PSM / Sheet
      type-code / derived geometry / writer-publish 边界的 evidence-graded
      inventory
- [x] 新增 `tasks.md`：后续 IDA availability、evidence refresh、fixture
      snapshot 与 backlog 分类任务
- [x] 新增 `quickstart.md`：parser inspection、coverage / byte-audit、test
      gates 与 IDA 复查流程
- [x] Phase 28-E：确认当前本地有 6 个 `.pid` fixture，并为全部 6 个
      fixture 生成 coverage / byte-audit JSON 快照；`data-model.md` 已记录
      snapshot matrix
- [x] Phase 28-F：在 `data-model.md` 中新增 completion classification，将各
      entry group 标为 `Complete` / `NeedsFixture` / `NeedsIDA` /
      `NeedsParser` / `Blocked`
- [x] Phase 28-G：基于 6-fixture byte-audit 快照生成
      `snapshot-priority-backlog.md`，聚合 highest-leftover families /
      individual paths / common unregistered paths
- [x] Phase 28-H：新增 `phase29-candidate-slices.md`，将 priority backlog
      转成 Phase 29-A..F 候选 implementation slices
- [ ] Phase 28-C：在 `ida-pro-mcp` tool descriptor 可用且相关 IDB 打开后，
      复查 `style.dll` / `J2DSrv.dll` / `sppid.dll` / `XCeedRAD.dll`
      证据
- **Status:** spec package + 6-fixture snapshot matrix + priority backlog
  + Phase 29 candidate slices complete；live IDA refresh blocked until
  relevant IDA modules / tool descriptors are available
- **Spec:** `docs/specs/2026-06-08-pid-file-format-spec-kit/spec.md`

### Phase 29：2026-06-08 Sheet leftover unknown record priority
- [x] Phase 29-A：基于 6-fixture byte-audit snapshot 生成 Sheet leftover
      priority report：
      `docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md`
- [x] 报告确认 `/Sheet6` 是主导 registered Sheet hotspot：
      total 129,506 / consumed 8,818 / leftover 120,688 / ratio 0.06808951
- [x] 报告识别两个 `Sheet6615` 小型 registered Sheet-like stream：
      total 632 / consumed 144 / leftover 488
- [x] 报告识别 `publish-a01` 的嵌套 `/JSite204/Sheet*` unregistered streams，
      需先决定 byte-audit registration / ownership 语义
- [x] Phase 29-A follow-up：新增 read-only probe
      `examples/probe_phase29_sheet_leftover_windows.rs`，生成
      `docs/analysis/2026-06-08-phase29-sheet-leftover-windows.md`
- [x] bounded windows report 已按 conservative local byte shape 分组，并输出
      sample ranges / header offset / hex prefix；仍保持 investigation-only
- [x] Phase 29-A review：新增
      `docs/analysis/2026-06-08-phase29-sheet-leftover-review.md`，确认当前
      Sheet byte-audit 未接入 typed/audit-only decoders，不能把 leftover
      直接解释成新格式
- [x] Phase 29-B：将现有 Sheet typed/audit-only decoders 接入 byte-audit
      trace，降低已知记录造成的 leftover 噪声
- [x] 修复 `ParserTrace::consumed_bytes()` mixed-confidence overlap 双计数：
      改为 `total_bytes - leftover_bytes`
- [x] 新增 focused tests：
      `consumed_bytes_counts_union_across_confidence_overlap` 与
      `sheet_typed_decoders_claim_known_record_ranges`
- [x] 重跑 6-fixture snapshot matrix 与
      `phase29-sheet-leftover-windows-after-trace.md`
- [x] 新增结果说明：
      `docs/analysis/2026-06-08-phase29-sheet-byte-audit-trace-integration.md`
- [x] Phase 29-C：新增
      `docs/analysis/2026-06-08-phase29-sheet-after-trace-review.md`，分类
      after-trace 剩余 groups
- [x] Phase 29-C 结论：优先执行 Phase 29-C1 Symbol Reject Probe；nested
      `/JSite204/Sheet*` 另走 ownership / registration review
- [x] Phase 29-C1：新增 `examples/probe_phase29_igsymbol_rejects.rs`，
      生成 `docs/analysis/2026-06-08-phase29-igsymbol-reject-probe.md`
- [x] Phase 29-C1 结论：剩余 `0x00CE` candidates 均为
      `out_of_domain_double`，保留 `decode_igsymbols` validation 不变
- [x] Phase 29-C2：新增
      `docs/analysis/2026-06-08-phase29-nested-sheet-ownership-review.md`，
      确认 nested `JSite*/Sheet*` 不与 top-level `/Sheet*` 混同
- [x] Phase 29-C2 结论：nested JSite streams 更像 symbol-local / embedded
      PSM package，需先建 ownership inventory
- [x] Phase 29-D：新增
      `docs/analysis/2026-06-08-phase29-nested-jsite-package-inventory.md`，
      生成 6-fixture nested `JSite*` package inventory
- [x] Phase 29-D 分类：`NeedsOwnership` / `CanTraceHeaderOnly` /
      `IgnoreUntilConsumerNeeds`
- [x] Phase 29-D follow-up：为 `CanTraceHeaderOnly` nested JSite
      cluster-family child streams 添加 header-only byte-audit trace
- [x] 新增 tests：
      `nested_jsite_cluster_header_gets_header_only_trace` 与
      `nested_jsite_jproperties_still_uses_jproperties_parser`
- [x] 重跑 6-fixture snapshots 和 nested inventory，更新 `data-model.md` /
      `snapshot-priority-backlog.md`
- [x] Phase 29-E：验证与收口：
      `cargo test --lib byte_audit -- --nocapture` 44 passed；
      `cargo fmt --all -- --check` 通过；ReadLints 无错误
- [x] Phase 28-C：IDA 可达性核查（2026-06-10）：仅 `core.dll`（无关）与
      `radsrvitem.dll` 可达，目标 IDB 未打开，Slice 29-F 维持 blocked
- [x] 文档漂移修正：`AGENTS.md` 0x0030 GArc2d → JStyleOverride；
      `tasks.md` 28-F / Validation 选框回写
- [x] Phase 29-F：PSMcluster0 body 三角化（candidate Slice 29-B）：
      新增 `examples/probe_phase29_psmcluster0_body_triage.rs`，
      证明 body = 单条连续 PSM envelope record 链（6/6 fixture，
      覆盖 99.8%+，`chain_records == record_count - 2`），
      产出 `docs/analysis/2026-06-08-phase29-psmcluster0-leftover-triage.md`
- [x] Phase 29-F follow-up：实现 audit-only PSMcluster0 walker
      （共享核心 `decode_cluster_body_records`）：full-coverage gate +
      trace 集成（prologue Probed / envelope Decoded / payload Probed），
      8 单测 + 3 panic-safety 入口 + cross-fixture ratchet
      （6/6 fixture record_count-2 成立、consumed ratio = 1.0），
      12 个快照 JSON 重生成，`/PSMcluster0` leftover 193,173 → 0，
      五项门禁全过
- [x] Phase 29-G：StyleCluster body 三角化 + walker：
      泛化探针证明单条 end-anchored 链（record_count 仅 2/6 匹配 →
      整条 Probed），`decode_style_cluster_body_records` +
      `parse_style_cluster_with_trace` 落地，6 单测 + ratchet 6/6 过，
      `/StyleCluster` leftover 83,468 → 12,300（仅剩 GUID-table 形
      prefix），五项门禁全过
- [x] Phase 29-H：StyleCluster prefix 特征化（documentation-only
      closeout）：12B opener + 532B 跨 fixture 常量 boilerplate +
      42B style slot 纹理；GUID 区非均匀 stride → 不上 parser，
      prefix 保持 leftover，IDA target 扩展
- [x] Phase 29-I：Unclustered DA body 三角化（candidate Slice 29-C）：
      先核查 IDA（2026-06-11 仍仅 core.dll / radsrvitem.dll，29-F 维持
      blocked）；新增 `examples/probe_phase29_da_body_triage.rs`，证明
      DA body = 8 字节 magic+counter prologue + 单条 end-anchored
      `0x0089` envelope 链（6/6 fixture，覆盖 0.9978–0.9998，
      417/417 trailer offset == chain head，"31 字节 trailer" 实为下一
      record 的 envelope head）；产出
      `docs/analysis/2026-06-08-phase29-dynamic-attributes-body-backlog.md`
      （walker backlog + 属性 census + IDA target request）
- [x] Phase 29-I follow-up：实现 audit-only DA body-chain walker：
      `decode_unclustered_da_body_records` +
      `parse_unclustered_da_with_trace`（全 `Probed` claims，
      end-anchored full-coverage gate，counter 不作硬 invariant）；
      aggregate DA branch 合并 walker + landmark scanner
      （parser name `parse_unclustered_da`）；7 单测 + 1 aggregate 测试 +
      panic-safety 入口 + ratchet
      `da_body_chain_is_end_anchored_across_fixtures`
      （6/6，records 47/69/231/169/22/169，leftover=0）；
      12 个 snapshot JSON 重生成，`/Unclustered Dynamic Attributes`
      leftover 111,120 → 0，全文件 ratio 升至 0.273–0.670；
      五项门禁全过
- [x] Phase 29-J：nested JSite cluster body dispatch（Slice B/C
      follow-up）：IDA 复查仍 blocked 后，新增
      `examples/probe_phase29_nested_cluster_bodies.rs` 证明 23/23 个
      一层 nested `JSite*` `PSMcluster0`(11) / `StyleCluster`(11) /
      `Unclustered Dynamic Attributes`(1) 流用未改动的 top-level
      walker 即可 end-anchored 走通（nested PSMcluster0 11/11 保持
      `record_count - 2`，链起点统一 145）；aggregate nested branch
      按 child 分派到完整 walker（Sheet* / DA Metadata 维持
      header-only）；新增 dispatch 单测 + 23-stream ratchet；
      快照重生成：`JSite*` family leftover 325,843 → 74,559
      （ratio 0.7817），全文件 ratio 0.630–0.880；五项门禁全过
- [x] Phase 29-K：per-record DA attribute scoping（Slice C named
      benefit #1）：抽出 `parse_section_body` 共享核心（提取逻辑
      byte-for-byte 不变）；新增
      `parse_attribute_records_chain_scoped`（chain gate 通过时按
      record 精确边界解析 attribute section，否则回退 legacy 扫描）；
      `streams/dynamic_attrs.rs` 管线切换，`streams/cluster.rs` 保持
      legacy；4 单测 + panic-safety + ratchet
      `da_chain_scoped_attribute_extraction_matches_or_beats_legacy_scan`
      （6/6：nonascii 68 → 69 找回 flagged-head `Symbol` record，
      其余 fixture 不变，全部既有 baseline 零回归）；快照不受影响
      （提取不改 byte-audit claims）；五项门禁全过
- [x] Phase 29-L：nested JSite registry dispatch：探针
      `examples/probe_phase29_nested_registry_streams.rs` 证明 68 个
      nested registry 流复用 top-level 格式（98.4% consumed，
      DocVersion2/3 / PSMclustertable / PSMsegmenttable 全量解析，
      PSMroots 同款 4 字节尾，4 字节 AppObject stub 干净 gate-out，
      summary pair 部分解析）；byte-audit nested branch 增加
      registry child → top-level parser 分派（JSitesList 无 parser
      维持 unregistered）；dispatch 单测 + ratchet；快照重生成：
      JSite family leftover 74,559 → 66,778，unregistered paths
      降至 12–19/fixture，全文件 ratio 0.664–0.888；五项门禁全过
- [x] Phase 29-M：JSitesList / Revision 清尾：探针
      `examples/probe_phase29_unregistered_tails.rs` 证明
      `/JSitesList` = "OLEM" magic + u32 count + u32 slot 表（逻辑
      entries 与 `JSite<id>` storage id 6/6 全量对应；dwg0202 族有
      3 个 stale 尾 slot 重复逻辑值），`/TaggedTxtData/Revision` =
      0 字节占位（5/5）；新增 `src/parsers/jsites_list.rs`（header
      Decoded / 逻辑表 Probed / stale 尾留 leftover）+ 顶层与 nested
      注册 + Revision 占位注册；8 单测 + panic-safety + ratchet
      （counts {9,10,20,13,5,13}，trailing {0,0,0,3,0,3}，
      storage matches == count 6/6）；快照重生成：unregistered
      51 → 38 distinct（9–14/fixture）；五项门禁严格全绿
      （首版 exact-size gate 被 dwg0202 stale tail 证伪后修正，
      clippy is_multiple_of 告警已修）
- **Status:** Phase 29-A..M complete（byte-accounting + 提取质量 +
  注册清尾全部收口）；剩余 multi-fixture unregistered 全部
  IDA / demand gated；deep semantics gated by IDA / controlled
  fixtures / downstream requirements
- **Analysis:** `docs/analysis/2026-06-08-phase29-sheet-leftover-priority.md`

### Phase 30：radsrvitem.dll IDA 续查（JSite / 0x0089 / gated 语义）
- [x] Phase 30-A：JSite storage naming refresh：在可达
      `radsrvitem.dll` IDB 中确认 `sub_56448A10` /
      `sub_56448A70` 均格式化 `JSite<id>`；`sub_5646FF60`
      接收整数 id，构造 `JSite<id>` 并调用 storage open path。
      `sub_5645FF00` / `sub_56460330` 调用链从 record/runtime
      context 取 id 后打开对应 JSite storage，强化 Phase 29-M
      `/JSitesList.entries` 与 `JSite<id>` storage id 的证据链。
- [x] Phase 30-B：确认当前 `radsrvitem.dll` 不含 `"OLEM"` /
      `JSitesList` 字符串；因此不能证明 `/JSitesList` writer /
      reader，也不能解释 dwg0202 stale tail。`JSitesListDecoded`
      字段继续保持 `entries` / `trailing_slots`，不升级命名为
      `jsite_ids`。
- [x] Phase 30-C：`0x0089` RAD export 边界：`sub_5644B640`
      确认 runtime record first word 会按 `*record == 137`
      过滤并收集；`sub_56445F40` 对 `0x0089` 走 default
      `sub_564462F0`；`sub_56448F70` type-name 表不包含
      `0x0089`，因此该路径只写 `RAD_OBJECT_TYPE = "137"`，
      不解 DA/PSMcluster0 head 字段或 ASCII class payload。
- [x] Phase 30-D：PSMspacemap segment handle 证据：`sub_5648C370`
      是 `ClusterTable::GetSpaceMapSegment()`，按 cluster entry
      的 segment-id array 复用/分配 segment；`sub_56479040`
      证明 handle = `(segment_id << 13) | entry_index`，entry
      范围 `0..0x1FFF`，segment capacity = `0x2000`；但尚未
      直接证明 raw `/PSMspacemap` page byte layout，因此不升级
      byte-audit claim。
- [x] Phase 30-E：Style/JStyle low-cost negative pass：可用 IDB 仍
      只有 `core.dll` / `radsrvitem.dll`；`StyleCluster` 0 hits、
      `JStyleOverride` 0 hits；`JStyleBase::IJPersistImp` /
      `IJManageStyle2Imp` 等命中仅为 generic interface thunk / RTTI，
      未暴露 `0x0030` persistence body 或 StyleCluster prefix layout。
- [x] Phase 30-F：IDA-gated next actions handoff：汇总当前
      `radsrvitem.dll` 已确认结论、不可升级边界，以及下一次打开
      `style.dll` / `J2DSrv.dll` / `sppid.dll` 等 IDB 后应优先执行的
      精确搜索清单；当前建议停止在现有 IDB 中盲搜，转为打开新 IDB
      或提交/评审当前工作树。
- [x] Phase 30-G：worktree readiness check：只读核对当前变更范围；
      当前为 Phase 29/30 累积大变更（tracked diff 16 files，
      4552 insertions / 109 deletions，另有 spec-kit / probe /
      analysis / `jsites_list.rs` untracked）；IDE lint 对关键 Rust
      文件无诊断；提交前仍建议跑五项 pre-commit gate，并注意若干
      Rust 文件 LF→CRLF warning。
- [x] Phase 30-H：final pre-commit gate run：`cargo fmt --all -- --check`、
      `cargo build --locked --workspace --all-targets`、
      `cargo test --locked --workspace --all-targets`、
      `cargo clippy --locked --workspace --all-targets -- -D warnings`
      全过；本地 Windows bash 运行 missing-docs 脚本仍失败
      （乱码 / `REGDB_E_CLASSNOTREG`），按项目既有 fallback
      `cargo rustdoc --lib --locked -- -W missing-docs` 验证通过。
- [x] Phase 30-I：commit/review plan：在未获明确 commit 授权前不提交；
      新增提交/评审方案，给出 single milestone commit 与三段拆分
      review units（Phase 29 parser/byte-audit、Phase 29 probes/spec/docs、
      Phase 30 IDA evidence/handoff），并记录建议 commit message 与
      residual risks。
- [x] Phase 30-J：focused self-review：自审关键 Rust 改动
     （byte-audit aggregate、cluster header walker、DA chain-scoped
      extraction、JSitesList parser、real-fixture / panic-safety tests）；
      `git diff --check` 无 whitespace errors，ReadLints 无诊断，
      未发现 blocking code issue；剩余风险为大 diff、LF→CRLF warning
      和 IDA-gated semantic questions。
- [x] Phase 30-K：`style.dll` JStyleOverride persistence refresh：
      打开 `E:\reverse\pid\style.dll` 为 IDA MCP instance
      (`127.0.0.1:13339`)；确认 `JStyleOverride` vtable、
      `stru_10066B64 = 47fcc338-2d0f-11d0-a1ff-080036a1cf02`；
      `sub_1000F030` 是当前 persistence body，13 次
      `IOContext::DoIO` 合计 64 字节，直接支持现有 `0x0030 =
      JStyleOverride` decoder；`sub_1000F210` 是 versioned path；
      `sub_10010640` clone 复制更宽 runtime slot 区并清 transient
      pointers。`style.dll` 未命中 `StyleCluster` / `JSitesList` /
      `OLEM` / `PSMspacemap` / `GraphicGroup`，这些仍需其它 IDB。
- [x] Phase 30-L：secondary IDB sweep：打开并扫 `J2DSrv.dll`
      (`13340`)、`XceedRAD.dll` (`13341`)、`jengine.dll` (`13342`)。
      `J2DSrv.dll` 更像 2D geometry/render/style helper；
      `XceedRAD.dll` 是 Xceed compression support；二者对
      `JSitesList` / `OLEM` / `PSMspacemap` / `StyleCluster` /
      `JStyleOverride` / `GraphicGroup` 均 0 hits。`jengine.dll` 含
      `IJPersist` / `IOContext` / `DoIO` / `PersistCluster` /
      PSM/segment diagnostic text，是通用 persistence engine，但也不含
      业务 storage 名。
- [x] Phase 30-M：`OLESITE.dll` JSitesList refresh：打开
      `OLESITE.dll` (`13344`)；确认它导出 `JSite` / `JSiteManager` /
      `JOLEMembassy` 相关接口，`off_1005BBC8 -> "JSitesList"`，
      `off_1005BBD0 -> "JSite"`；`sub_1001DFC0` 是 versioned
      `JOLEMembassy` persistence dispatcher，version 1/2 分别走
      `sub_1001D2C0` / `sub_1001D7F0`，二者打开 `JSitesList`、
      读写 count 并迭代 `JSite` entries。`entries` 可解释为
      IDA-backed JSite ids / entries；`trailing_slots` stale semantics
      仍保守。
- [x] Phase 30-N：spec-kit JSitesList terminology sync：同步
      `docs/specs/2026-06-08-pid-file-format-spec-kit/data-model.md`，
      将 `JSitesList` 证据从 fixture match / unnamed 提升为
      `OLESITE.dll` 直接证据；保留 JSON 字段名 `entries`，继续将
      `trailing_slots` 作为未证明 stale/delete 语义的保守 leftover。
- [x] Phase 30-O：local OLE follow-up closeout：尝试打开
      `OLECRT.dll`，IDA 进程存在（`ida.exe E:\reverse\pid\OLECRT.dll`），
      但未注册为 IDA MCP instance，无法做 survey/search；结合
      `Linkole.dll` / `OLESITE.dll` 结果，本地 SmartSketch/RAD runtime
      低成本 broad search 收敛。
- [x] Phase 30-P：`smartplantpid.exe` launcher sweep：打开用户提供的
      `D:\work\plant-code\cad\pid-parse\dlls\smartplantpid.exe` 为
      IDA MCP instance `13345`；survey 显示 32-bit VB6 application
      / launcher（`MSVBVM60` imports，120 functions，161 strings），
      strings 包含 `SmartPlantPID` / `Smart Plant P&ID` / `sppid` /
      `Registry` / `ErrorLogging`，但 Phase 30 checklist
      `JSitesList` / `OLEM` / `JSite` / `PSMspacemap` /
      `StyleCluster` / `JStyleOverride` / `GraphicGroup` /
      `P&IDAttributes` / `IJPersist` / `IOContext` / `DoIO` 均 0 hits。
      结论：该 EXE 是前端/启动壳，不是 `.pid` storage body reader。
- **Status:** partial / gated；当前可达 IDB 的 JSite / 0x0089 /
  PSMspacemap handle 与 `0x0030` JStyleOverride persistence 证据已
  收口，`JSitesList` storage/name/entry 语义也已由 `OLESITE.dll`
  直接确认。`smartplantpid.exe` 已确认只是 VB6 launcher；下一步若
  继续 IDA，应优先真实 SmartPlant P&ID install 的 lower-level backend
  DLL / COM module（例如 `sppid.dll` 或含 storage reader 的产品 DLL）。
- **Analysis:** `docs/analysis/2026-06-12-phase30-radsrvitem-jsite-ida-refresh.md`
- **Analysis:** `docs/analysis/2026-06-12-phase30-radsrvitem-record-spacemap-ida.md`
- **Analysis:** `docs/analysis/2026-06-12-phase30-radsrvitem-style-jstyle-negative.md`
- **Analysis:** `docs/analysis/2026-06-12-phase30-style-dll-jstyleoverride-ida.md`
- **Analysis:** `docs/analysis/2026-06-12-phase30-secondary-idb-sweep.md`
- **Analysis:** `docs/analysis/2026-06-12-phase30-olesite-jsiteslist-ida.md`
- **Analysis:** `docs/analysis/2026-06-12-phase30-smartplantpid-exe-ida.md`
- **Analysis:** `docs/analysis/2026-06-12-phase30-ida-gated-next-actions.md`
- **Spec:** `docs/specs/2026-06-08-pid-file-format-spec-kit/data-model.md`
- **Analysis:** `docs/analysis/2026-06-12-phase29-30-worktree-readiness.md`
- **Analysis:** `docs/analysis/2026-06-12-phase29-30-commit-review-plan.md`
- **Analysis:** `docs/analysis/2026-06-12-phase29-30-self-review.md`

## 决策
| 决策 | 理由 |
|---|---|
| 先补 PSM / Sheet，再做统一语义图 | PSM/Sheet 是当前解析深水区，也是 canonical graph 的事实来源 |
| 保持 Probe/Decode 双层模型 | 避免把启发式识别误交付为稳定语义 |
| 开发任务必须绑定 byte-audit / coverage / fixture gate | 当前项目优势是可证明覆盖率，不能退回主观判断 |
| Publish XML 与 `.pid` 深层解析分线推进 | MDF publish 已接近交付，`.pid` Sheet/PSM 仍处逆向阶段，节奏不同 |
| H7CAD 只渲染已证明的 inferred points | object-coordinate mapping 尚未 source-proven，endpoint/topology 不能伪装 CAD 几何 |
| GraphicIdentityNearby 独立为 PR5 | 身份证据路线有价值但噪声高，应与 PR4 field-x 基础调查分开 review |
| Text placement 先作为 PR6 investigation | 当前 `/Sheet6` 文本多像二进制误识别，不能直接升级为 `Text + Inferred` |
| Phase 8 先做多 fixture 与 Sheet record grammar | 当前 promotion 缺的是 source-proven record 证据，不是 H7CAD UI 能力 |
| Phase 9 先补 fixture baseline 再扩大 promotion | 当前 5 fixture 横向扫描已有 `object_geometry_hint_count=5`，但 Text/Symbol 仍 `text_over_threshold=0`，下一步应先硬化 registry 与 gate |
| Phase 10 优先利用 f64 pair 突破 endpoint line 零线困局 | Phase 9A fixture 扩容被外部样本供给阻塞；Phase 9C 诊断链已发现 repeated f64 pair 坐标候选，可在现有 5 fixture 上闭环 endpoint line |
| Phase 13+ 把详细计划迁移到 `goals/phaseNN-…/` | 单个 `task_plan.md` 文件超过 200 行会失焦；goal package 五件套对 Codex `/goal` 与 Plannotator 更友好 |
| Phase 16 跨 5 IDA instance 反向 0x0030 → JStyleOverride | Phase 14 `decode_primitive_arcs` 的 `axis_a.y ≈ 0` 约束误拒 50 条真 record；必须 IDA-confirmed 修正 |
| Phase 17 移除 legacy `PrimitiveArc` 而非保留 dual-surface | Phase 16 已证明 0x0030 不是 IGDS GArc2d，保留 dual surface 会让下游消费者继续误读 |
| Phase 18 audit-only 而非 typed sub-record DTO | 0x0010 是 polymorphic family，未 IDA-confirmed 前命名 sub-kind 字段 = Phase 14 GArc2d 重蹈覆辙 |
| Phase 19 加 `leading_word` 而非 `sub_kind` | probe 证明 `+0..+1` 只覆盖 ~36% records；size 31/70/13/16/43 在 +0 异质，单一固定 discriminator 不存在 |
| Phase 19 RAD sibling sweep 被证伪后改走 leading-word | 不浪费已采集的 probe 数据；leading-word 是 Phase 18 audit collection 上最便宜的可命名维度 |
| Phase 20 选 IDA-first 而非 byte-pattern-only | Phase 19 probe 已证明纯 byte 看不出 size 31 bucket discriminator；IDA 是唯一可获权威证据的路径 |
| Phase 20 拒绝在单 session 内执行 | 5374 个 function（4867 unnamed）的反向工作量与 Phase 16 量级相当，单 session 必然 lost context；7 个 Slice + 跨 session checkpoint 是必须的 |

## 错误与限制
| 问题 | 处理 |
|---|---|
| `ace-tool.search_context` 当前不可用，语义搜索返回空 | 已降级为文件结构、精确搜索和关键文件读取 |
| `rsvg-convert` / `magick` 不在 PATH | 先生成 SVG 并用 XML parser 验证；PNG 导出待安装转换工具后补 |
| `/Sheet6` same-object identity 未与 feature scoring 相交 | 记录为 guardrail：identity report 有信号，但 scoring 仍 `over_threshold=0` |
| `/Sheet6` text runs 存在二进制误识别风险 | Text-quality filter 已拒绝 Hangul 等误识别特征，当前 `text_quality_passed=0` |
| 多 fixture inventory 仍无 promotion 候选 | 记录为 Phase 8 基线：`identity_supported=0`、`identity_over_threshold=0`、`text_over_threshold=0` |
| 读取 `progress.md` offset 220 超出文件长度 | 已确认文件只有 189 行，改用已读取内容作为当前进度依据 |
