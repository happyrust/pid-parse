# H7CAD PID 完整解析下一阶段开发方案

> 日期：2026-05-02  
> 目标：从当前 `ProbeOnly / Inferred Point` 能力推进到可证明的 `Line / Text / Symbol` 显示。  
> 当前原则：没有 source-proven `object -> coordinate` 映射前，不把 endpoint/topology 渲染成 CAD 线。

## 0. 当前基线

已完成：

- H7CAD 可显示 PID `Inferred Point`。
- `SheetGeometry` / normalized geometry contract 已落地。
- `SheetObjectGeometryHint` 保持空基线，避免误 promotion。
- Field-X、GraphicIdentityNearby、Text placement 三条证据路线均有 focused regression。
- 多 fixture evidence inventory 已覆盖当前可用 5 个 PID fixture，包含非 ASCII 文件名 fixture 与 publish fixture。
- 第一版 Sheet record shape classifier 已落地，可输出 chunk-relative field/coordinate shape classes。

最新横向报告：

```text
fixtures=5
sheets=3
windows=6337
record_shape_classes=328
identities=437
same_object=17
wrong_object=420
identity_supported=0
max_identity_score=45
identity_over_threshold=0
text_candidates=578
text_over_threshold=0
```

结论：样本覆盖增加后仍没有可 promotion 的 Line/Text/Symbol 证据。

已新增 per-fixture / per-sheet 明细输出；其中非 ASCII 文件名 fixture
`工艺管道及仪表流程-1.pid` 可被扫描，但当前 `/Sheet6` 无 endpoint
`field_x`，只贡献 text candidate 证据。

`/Sheet6` 当前 top record shape classes 为 `(14,38)` 与 `(46,70)`，support
均为 2；这提供了后续 grammar 复查入口，但当前 `promotable=0`。

多 fixture 汇总后，top aggregate record shapes 为 `(12,-18)`、`(14,38)`、
`(68,5)`，累计 support 最高为 4；仍缺少 same-object identity 与 promotion
score 相交证据。

## 1. 离“完整解析”的主要缺口

| 缺口 | 当前状态 | 完成判定 |
|---|---|---|
| 多 fixture 覆盖 | 5 个 fixture 已入 inventory，per-fixture / per-sheet 明细已输出 | 覆盖 8-12 个真实 PID，包含不同图纸模板、设备密度、文字标注风格 |
| Sheet record grammar | 第一版 classifier 已能识别重复 chunk-relative shape，但字段边界仍未 source-proven | 至少识别 2-3 类稳定 record shape，并能解释坐标字段边界 |
| object-coordinate mapping | 未 source-proven | 同一对象 identity、record shape、coordinate delta 三类证据相交 |
| Text extraction | 当前存在二进制误识别 | 真实工程标签通过质量过滤，并能与坐标稳定配对 |
| Symbol placement | 仅有 JSite / symbol path 线索 | symbol instance 能和 Sheet 位置、对象 identity 建立稳定映射 |
| H7CAD rendering | 只渲染 inferred points | 分层显示 Line/Text/Symbol，并保留 evidence/provenance UI |

## 2. 推荐执行阶段

### Phase 8A：Fixture 扩容与 inventory hardening

目标：让“没有 promotion”的结论不再只依赖 `/Sheet6`。

任务：

- 收集并登记更多 `.pid` fixture，优先同一 plant 不同图、不同模板、不同语言标注。
- 扩展 `available_pid_fixtures_geometry_evidence_inventory_stays_probe_only`：
  - 支持非 ASCII 文件名 fixture 的显式登记。
  - 输出每个 fixture/sheet 的 top scores，而不仅是总计。
  - 区分缺少 cross-reference、缺少 dynamic attributes、缺少 raw Sheet stream 三类 skip。
- 把 inventory 输出沉淀到 `findings.md`，保留 promotion gate 关键值。

完成判定：

- 至少 8 个 PID fixture 可被 inventory 扫描。
- 任意 fixture 出现 `identity_over_threshold > 0` 或 `text_over_threshold > 0` 时，必须新增独立 regression，不直接 promotion。

### Phase 8B：Sheet record grammar 反推

目标：从 byte-window 证据推进到 record-shape 证据。

任务：

- 对 top identity/text candidates 生成 compact record dump：
  - chunk range
  - nearby field_x
  - nearby coordinates
  - identity record id / DrawingID
  - repeated byte markers
- 为稳定 record shape 建立 classifier：
  - record header / body / trailer 边界
  - coordinate pair 字段位置
  - field_x / object identity 字段位置
- 只在 classifier 可解释字段边界后，新增 `DecodedCandidate` 风格 DTO；否则继续留在 probe。

完成判定：

- 至少 2 个 fixture 上同类 record shape 重复出现。
- 字段边界能被 byte-audit 或 focused fixture test 锁定。

### Phase 8C：Object-coordinate promotion gate

目标：定义 Line/Symbol promotion 的硬门槛。

推荐 gate：

```text
same-object identity evidence
+ stable record shape
+ stable coordinate field offsets
+ coordinate quality passed
+ repeated support across fixture/sheet
=> populate SheetObjectGeometryHint
```

任务：

- 新增 `SheetObjectGeometryHint` population 的红测，但先用 fixture 证明当前仍为空。
- 找到满足 gate 的 fixture 后，只 promotion 最小对象集合。
- H7CAD 先把 promoted hint 渲染到独立 evidence layer，不混入 topology preview。

完成判定：

- `object_geometry_hints > 0` 只在 source-proven fixture 上出现。
- 现有 `/Sheet6` guardrail 仍保持 `object_geometry_hints=0`。

### Phase 8D：Text 与 Symbol 渲染升级

目标：在 Line 之前或并行推进低风险可视元素。

任务：

- Text：
  - 改进 text extraction，降低 CJK/Hangul 二进制误识别。
  - 真实标签必须通过质量过滤与坐标配对。
  - 通过后再生成 `PidGraphicKind::Text`。
- Symbol：
  - 把 JSite symbol path、object identity、Sheet position 三类证据相交。
  - 先渲染 symbol anchor，不直接复原完整 block geometry。

完成判定：

- H7CAD 中 Text/Symbol 层可单独开关。
- 每个渲染实体都能追溯到 stream path、offset、confidence、reason。

## 3. 风险与禁止项

- 禁止降低 promotion threshold 来“看起来有线”。
- 禁止把 relationship endpoint 当作 CAD line。
- 禁止把二进制误识别文本渲染为工程标注。
- 禁止为了 PR 拆分重写已推送 `main` 历史；如果需要 review 形态，应从旧 base 重建分支并明确告知。

## 4. 近期任务清单

1. [x] 给多 fixture inventory 增加 per-fixture / per-sheet 明细输出。
2. [x] 增加非 ASCII 文件名 fixture 的显式登记与 skip 说明。
3. [x] 为 top identity/text candidates 生成 record dump helper。
4. [x] 建立第一版 Sheet record shape classifier。
5. [ ] 只有 gate 达标后，再推进 `SheetObjectGeometryHint` population 和 H7CAD Line/Text/Symbol layer。

