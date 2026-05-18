# 任务计划：PID 解析能力开发方案

## 目标
基于当前 `pid-parse` 能力现状，制定下一阶段中文开发方案：优先补齐高价值解析缺口，保持 Probe/Decode 分层、byte-audit 可验证、writer passthrough 安全边界。

## 当前阶段
Phase 24 - CoordinatePageMetadata decoder 候选筛选方案已制定；下一步先从
Phase 23 `top_evidence` 生成 candidate evidence table，不直接 promotion
`PidPageTransform::Available`。

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
