# pid-parse 下一阶段开发计划

> 日期：2026-05-06
> 范围：SmartPlant / Smart P&ID `.pid` 深层解析、H7CAD 几何显示、canonical graph、publish XML 守门。
> 原则：只 promotion 有 source provenance 的结果；probe 结果继续服务逆向，不作为稳定交付承诺。

## 0. 当前基线

`pid-parse` 已经具备成熟的 `.pid` 容器读取、元数据解析、对象关系图、cross-reference、layout-first 视图、byte-audit、writer passthrough，以及 MDF-first publish XML 管线。

当前主要瓶颈集中在 Sheet 深层几何：

- 已能输出 `SheetGeometry` DTO、text run、coordinate hint、endpoint record。
- 已能把 coordinate hint 作为 `Inferred Point` 暴露给 H7CAD。
- 已建立多 fixture evidence inventory、top candidate dump helper、Sheet record shape classifier。
- 当前 5 个 PID fixture 的横向扫描已经出现最小 object-coordinate promotion，但 Text/Symbol 仍无 promotion 证据：
  - `identity_supported=44`
  - `identity_over_threshold=28`
  - `promotable=5`
  - `object_geometry_hint_count=5`
  - `text_over_threshold=0`

## 1. 目标

近期目标不是宣称完整解析 SmartPlant 私有几何，而是把“哪些内容可交付、哪些内容仍是 probe”变成可证明、可回归、可审查的工程事实。

交付目标：

1. 真实 fixture baseline 可稳定运行。
2. Sheet record grammar 有最小 source-proven 字段边界。
3. object-coordinate promotion gate 明确且有回归测试，promotable count 必须与 `object_geometry_hints` 对齐。
4. H7CAD 只渲染通过 gate 的实体，并保留 provenance UI；Text/Symbol 继续等待独立 gate。
5. canonical graph 只吸纳 decoded / source-proven inferred 事实，不固化 probe 噪声。
6. A01 publish 继续保持 green，DWG publish 阻塞项收敛到明确 fixture/enrichment 缺口。

## 2. 开发阶段

### Phase 9A：Fixture Baseline Hardening

目标：让当前最小 promotion 与 probe-only 结论都不依赖单个 `/Sheet6` 样本。

任务：

- 将当前 5 个 PID fixture 扩展到 8-12 个。
- 覆盖不同模板、设备密度、语言标注、Sheet 数量、DWG/A01 风格。
- 为每个 fixture 输出 per-fixture / per-sheet evidence summary。
- 区分三类 skip：
  - 缺 raw Sheet stream。
  - 缺 dynamic attributes。
  - 缺 cross-reference endpoint links。
- 建立可提交的脱敏 baseline；私有 fixture 走 local-only 或 CI artifact。

验收：

- `available_pid_fixtures_geometry_evidence_inventory_stays_probe_only` 至少覆盖 8 个 fixture。
- 任意 fixture 出现 `identity_over_threshold > 0` 或 `text_over_threshold > 0` 时，必须新增 focused regression。
- baseline 下降时 CI 或 local gate 能明确指出 fixture、sheet、metric。

### Phase 9B：Sheet Record Grammar Reverse Engineering

目标：从 byte-window 证据推进到可解释 record shape。

任务：

- 对 top aggregate shapes `(12,-18)`、`(14,38)`、`(68,5)` 做人工 grammar 复查。
- 为高频 shape 输出 compact dump：
  - chunk range。
  - nearby `field_x`。
  - nearby coordinates。
  - identity record id / DrawingID。
  - repeated marker。
  - bounded hex window。
- 提炼第一批 candidate record field layout。
- 在 byte-audit 中区分 decoded field、probed prefix、leftover trailer。

验收：

- 至少 2 类 record shape 在 2 个以上 fixture 中重复出现。
- 每类 shape 至少有一个 focused unit test 和一个真实 fixture soft-skip/hard gate。
- 未证明语义的字段不得标为 `Decoded`。

### Phase 9C：Object-Coordinate Promotion Gate Hardening

目标：把当前最小 `SheetObjectGeometryHint` promotion 固化为可审查 gate，而不是隐式启发式。

promotion gate：

```text
same-object identity evidence
+ stable record shape
+ stable coordinate field offsets
+ coordinate quality passed
+ repeated support across fixture/sheet
=> SheetObjectGeometryHint
```

任务：

- 保持 `object_geometry_hint_count == promotable` 回归测试。
- 只 promotion 满足 gate 的最小对象集合。
- 每个 hint 必须带：
  - `stream_path`
  - byte range / record id
  - `field_x`
  - confidence
  - reason
- `build_normalized_geometry()` 必须把 promoted hint 的 source note 投影到 `PidGraphicProvenance.note`，让 H7CAD / renderer 能读取 promotion gate 摘要。
- H7CAD 将 promoted hint 放在独立 evidence layer，不混入 topology preview。

验收：

- 当前 DWG fixture 的 promoted hint 数量必须与 gate output 对齐。
- promoted hint 进入 normalized geometry 后仍保留 `score`、identity evidence 与 stable shape evidence。
- 新增 fixture 若达标，只让达标对象出现 hint。
- endpoint/topology 不能被用于生成 CAD line。

### Phase 9D：Text / Symbol Source-Proven Rendering

目标：在 line 之前或并行推进低风险可视元素。

任务：

- Text：
  - 改进 text extraction，降低 CJK/Hangul 二进制误识别。
  - 将真实标签与 coordinate hint 稳定配对。
  - 通过质量过滤后才生成 `PidGraphicKind::Text`。
- Symbol：
  - 交叉验证 JSite symbol path、object identity、Sheet position。
  - 先渲染 symbol anchor，不复原完整 block geometry。
- H7CAD：
  - Line/Text/Symbol layer 独立开关。
  - 悬浮信息展示 source stream、offset、confidence、reason。

验收：

- Text/Symbol 每个实体都有 provenance。
- ProbeOnly text 仍默认不渲染。
- 错误文本不得通过 quality gate。

### Phase 9E：Canonical Graph Integration

目标：统一下游消费面，避免 UI 同时拼 `object_graph`、`cross_reference`、`layout`、probe 输出。

任务：

- 新增 additive normalized graph model：
  - `NormalizedObject`
  - `NormalizedRelationship`
  - `NormalizedEndpoint`
  - `NormalizedSymbolRef`
  - `NormalizedGeometryRef`
  - `Provenance`
- `PidImportView` 先消费 normalized graph 的稳定子集。
- 旧 `PidDocument` 字段保留兼容窗口。
- JSON schema 与 fixture snapshot 同步。

验收：

- 下游只消费 normalized graph 即可重建主要 object/relationship/endpoint/symbol 信息。
- Probe-only evidence 不进入 stable normalized graph，除非明确标记 confidence。

### Phase 9F：Publish XML Gate Closure

目标：保持 A01 高置信交付，同时收敛 DWG 侧剩余阻塞。

任务：

- A01：
  - 保持 `_Data.xml` semantic diff、interface parity、attr parity、Rel UID soundness、`_Meta.xml` parity。
  - 继续只对白名单 synthetic slots 做窄归一化。
- DWG：
  - 固化 compare-only MDF fixture 状态。
  - 闭环 branch-point parity。
  - 补 loader-side canonical field enrichment：
    - EqType / ProcessEqCompType。
    - ConnectionFlowDirection。
    - insulation / slope fields。
  - soft-skip 信息必须可操作。

验收：

- A01 gates 不回退。
- DWG fixture 存在时 hard gate；不存在时 soft-skip 说明阻塞项。
- GPL-3.0 vendored MDF reader 分发边界写入发布说明。

## 3. 禁止项

- 禁止降低 promotion threshold 来制造线。
- 禁止把 relationship endpoint 渲染成 CAD line。
- 禁止把二进制误识别文本当工程标注。
- 禁止把 candidate owner / field layout 直接升级为 decoded semantic meaning。
- 禁止将私有 fixture 原始内容提交到公共仓库。

## 4. 推荐执行顺序

1. Phase 9A fixture baseline hardening。
2. Phase 9B Sheet record grammar。
3. Phase 9C 最小 object-coordinate promotion。
4. Phase 9D Text/Symbol rendering。
5. Phase 9E canonical graph。
6. Phase 9F publish XML gate closure。

## 5. 首批任务清单

1. [x] 为当前 fixture 建立 registry 与 availability summary；继续扩展到 8-12 个 fixture。
2. 将 top aggregate record shapes 输出为稳定审查报告。
3. [x] 为 `SheetObjectGeometryHint` 写 promotion-count 与 provenance guardrail 测试。
4. 设计 `NormalizedGraph` additive schema 草案。
5. 梳理 DWG publish soft-skip 阻塞清单。
