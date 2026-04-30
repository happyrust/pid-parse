# 任务计划：PID 解析能力开发方案

## 目标
基于当前 `pid-parse` 能力现状，制定下一阶段中文开发方案：优先补齐高价值解析缺口，保持 Probe/Decode 分层、byte-audit 可验证、writer passthrough 安全边界。

## 当前阶段
Phase 2

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
- [ ] 将现有 Sheet text run / endpoint record / coordinate hint 归一化
- [ ] 定义稳定的 `SheetGeometry` / `SheetText` / `SheetEndpoint` DTO
- [ ] 保持未命名字节走 probe，不伪装为 decoded
- [ ] 用真实 fixture 或 synthetic fixture 建立回归样例
- **Status:** pending

### Phase 4：规范化语义图层
- [ ] 将 object、relationship、endpoint、symbol、cluster、sheet provenance 统一为 canonical graph
- [ ] 明确 `PidDocument` 原始事实层与 `ImportView` 消费视图边界
- [ ] 为 H7CAD / 下游 CAD 导入提供稳定 JSON contract
- **Status:** pending

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

## 决策
| 决策 | 理由 |
|---|---|
| 先补 PSM / Sheet，再做统一语义图 | PSM/Sheet 是当前解析深水区，也是 canonical graph 的事实来源 |
| 保持 Probe/Decode 双层模型 | 避免把启发式识别误交付为稳定语义 |
| 开发任务必须绑定 byte-audit / coverage / fixture gate | 当前项目优势是可证明覆盖率，不能退回主观判断 |
| Publish XML 与 `.pid` 深层解析分线推进 | MDF publish 已接近交付，`.pid` Sheet/PSM 仍处逆向阶段，节奏不同 |

## 错误与限制
| 问题 | 处理 |
|---|---|
| `ace-tool.search_context` 当前不可用，语义搜索返回空 | 已降级为文件结构、精确搜索和关键文件读取 |
| `rsvg-convert` / `magick` 不在 PATH | 先生成 SVG 并用 XML parser 验证；PNG 导出待安装转换工具后补 |
