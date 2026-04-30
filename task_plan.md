# 任务计划：PID 解析能力开发方案

## 目标
基于当前 `pid-parse` 能力现状，制定下一阶段中文开发方案：优先补齐高价值解析缺口，保持 Probe/Decode 分层、byte-audit 可验证、writer passthrough 安全边界。

## 当前阶段
Phase 7

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

## 错误与限制
| 问题 | 处理 |
|---|---|
| `ace-tool.search_context` 当前不可用，语义搜索返回空 | 已降级为文件结构、精确搜索和关键文件读取 |
| `rsvg-convert` / `magick` 不在 PATH | 先生成 SVG 并用 XML parser 验证；PNG 导出待安装转换工具后补 |
| `/Sheet6` same-object identity 未与 feature scoring 相交 | 记录为 guardrail：identity report 有信号，但 scoring 仍 `over_threshold=0` |
| `/Sheet6` text runs 存在二进制误识别风险 | Text-quality filter 已拒绝 Hangul 等误识别特征，当前 `text_quality_passed=0` |
